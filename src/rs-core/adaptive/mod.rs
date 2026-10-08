use std::cmp::Ordering;

use self::bandwidth_estimator::BandwithEstimator;
use crate::parser::VariantStream;

mod bandwidth_estimator;
mod ewma;

/// Chooses variants from throughput estimates and buffer occupancy.
pub(crate) struct AdaptiveQualitySelector {
    bandwidth_estimator: BandwithEstimator,
}

/// Estimate struct produced by the `AdaptiveQualitySelector`.
/// Indicate which variant should be selected.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdaptiveVariantSelection {
    /// Variant ID that can be selected to fill future buffer positions.
    /// Equal or of a higher quality as the variant indicated through
    /// `safe_variant_id`.
    ///
    /// Unlike `safe_variant_id` this is the recommentation **ONLY** for buffer
    /// addition, not buffer replacement.
    /// Switching may incidentally overlap the buffer's tail when segment boundaries differ.
    pub(crate) best_variant_id: u32,
    /// Variant ID that should be selected if *replacing* buffered positions
    /// (e.g. you might want to do that to visually raise in quality more
    /// quickly). "Safe" because we're unlikely to enter a rebuffering period
    /// under that variant.
    pub(crate) safe_variant_id: u32,
}

/// Playback information used to choose an adaptive variant.
pub(crate) struct PlaybackConditions {
    /// Minimum known duration buffered ahead of the wanted position across audio/video
    /// SourceBuffers, in media seconds. `None` means no buffer information is available.
    ///
    /// TODO/NOTE: Explore separate audio/video BOLA decisions using each media type's buffer
    /// and a shared throughput estimate. Scheduling would need to reconcile rendition
    /// choices and account for their combined network cost.
    pub(crate) buffer_level: Option<f64>,
    /// Amount of buffered data in front of the current position in seconds at
    /// which data will stop being buffered.
    pub(crate) buffer_goal: f64,
    /// Current **wanted** playback rate. `2` indicates x2 playback, `1` is
    /// normal playback etc.
    pub(crate) playback_speed: f64,
    /// Content-wide maximum observed target duration, retained while playlists load.
    /// Used to calibrate BOLA thresholds and roughly budget downloads above throughput.
    pub(crate) abr_reference_segment_duration: Option<f64>,
}

/// Factor with which we multiply bandwidth estimates to ensure a safe variant
/// recommendation is given in terms of variant.
const BANDWIDTH_ESTIMATE_FACTOR: f64 = 0.8;

/// Minimum size for the `qlow` parameter of BOLA: the maximum size of buffer in seconds for which
/// the lowest quality will be given (unless `qmax` is too low for it).
const BOLA_MIN_LOW_BUFFER: f64 = 3.0;
/// Maximum size for the `qlow` parameter of BOLA.
const BOLA_MAX_LOW_BUFFER: f64 = 10.0;

const BOLA_UP_SWITCH_HYSTERESIS: f64 = 0.25;
/// Spend at most this fraction of buffered playback time when raising quality above throughput.
const BOLA_DOWNLOAD_BUFFER_FRACTION: f64 = 1.;
/// Minimum playback lead after allowing for an overlapping segment at a rendition switch.
const BOLA_MIN_SWITCH_LEAD_SECONDS: f64 = 5.;

impl AdaptiveQualitySelector {
    /// Creates new `AdaptiveQualitySelector`.
    pub(crate) fn new(initial_bandwidth: f64) -> Self {
        Self {
            bandwidth_estimator: BandwithEstimator::new(initial_bandwidth),
        }
    }

    /// Adds metric allowing the `AdaptiveQualitySelector` to provide more educated guesses.
    /// Here, `duration_ms` should correspond to the time taken to make a request and `size_bytes`
    /// should be the corresponding size of loaded data.
    pub(crate) fn add_metric(&mut self, duration_ms: f64, size_bytes: u32) {
        self.bandwidth_estimator.add_sample(duration_ms, size_bytes);
    }

    /// Estimate which variant to choose from what's available, playback conditions, and
    /// collected bandwidth estimates.
    ///
    /// # Arguments
    ///
    /// * `variants` - The pool of variants this method should choose from. **MUST**
    ///   be in ascending quality order.
    ///
    /// * `current_variant_id` - The id for the optional current variant selected.
    ///
    /// *`playback` - Various metadata about playback.
    ///
    /// # Returns
    ///
    /// - The produced variant estimate from those conditions and internal data.
    pub(crate) fn select_variant(
        &self,
        variants: &[&VariantStream],
        current_variant_id: Option<u32>,
        playback: &PlaybackConditions,
    ) -> Option<AdaptiveVariantSelection> {
        match variants {
            [] => return None,
            [variant] => {
                return Some(AdaptiveVariantSelection {
                    best_variant_id: variant.id(),
                    safe_variant_id: variant.id(),
                });
            }
            _ => {}
        }
        let estimate = self.get_estimate();
        let bandwidth = if playback.playback_speed.is_finite() && playback.playback_speed > 0. {
            estimate / playback.playback_speed
        } else {
            estimate
        };

        // Variant id just by looking at the throughput estimate.
        let throughput_id = best_variant_id(variants.iter().copied(), bandwidth)
            .or_else(|| fallback_variant_id(variants.iter().copied()))?;

        let Some(buffer_level) = playback.buffer_level else {
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        };
        let segment_duration = playback.abr_reference_segment_duration.unwrap_or(0.);
        if !segment_duration.is_finite() || segment_duration <= 0. {
            // Missing enough information for the buffer-based estimate, exiting with throughput
            // choice
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        }

        let qmax = playback.buffer_goal.max(segment_duration);
        let qlow = (segment_duration * 2.)
            .clamp(BOLA_MIN_LOW_BUFFER, BOLA_MAX_LOW_BUFFER)
            .min((qmax - 0.1).max(segment_duration));
        let clamped_buffer = buffer_level.max(0.).min(qmax);
        if clamped_buffer < qlow {
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        }

        let Some(bola) = BolaModel::new(variants, qlow, qmax) else {
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        };
        let mut bola_id = bola.variant_id(clamped_buffer)?;
        if let Some(current_position) =
            current_variant_id.and_then(|id| variants.iter().position(|variant| variant.id() == id))
        {
            // Rely on hysteresis to limit oscillation
            let bola_position = variants
                .iter()
                .position(|variant| variant.id() == bola_id)?;
            if bola_position > current_position {
                let conservative_buffer =
                    (clamped_buffer - (segment_duration * BOLA_UP_SWITCH_HYSTERESIS)).max(qlow);
                let conservative_id = bola.variant_id(conservative_buffer)?;
                let conservative_position = variants
                    .iter()
                    .position(|variant| variant.id() == conservative_id)?;
                if conservative_position <= current_position {
                    bola_id = variants[current_position].id();
                }
            }
        }

        // Estimate a variant bitrate limit from half the actual buffer and current throughput.
        // `bandwidth` already accounts for playback speed.
        let max_variant_bitrate =
            bandwidth * (buffer_level.max(0.) / segment_duration) * BOLA_DOWNLOAD_BUFFER_FRACTION;
        // A segment from another rendition may begin one target duration before the buffer
        // ends. Gate upgrades above throughput on that rough overlap plus a playback lead.
        let has_switch_lead = clamped_buffer > segment_duration + BOLA_MIN_SWITCH_LEAD_SECONDS;
        let bola_position = variants
            .iter()
            .position(|variant| variant.id() == bola_id)?;
        // TODO: Have safe/best-aware segment selection budget new requests without cancelling
        // in-flight downloads solely because playback drained buffer during the download.
        let budgeted_bola_id = variants[..=bola_position]
            .iter()
            .rev()
            .find(|variant| has_switch_lead && (variant.bandwidth() as f64) <= max_variant_bitrate)
            .map(|variant| variant.id());
        let best_id = variants
            .iter()
            .rev()
            .find(|variant| {
                Some(variant.id()) == budgeted_bola_id || variant.id() == throughput_id
            })?
            .id();

        Some(AdaptiveVariantSelection {
            best_variant_id: best_id,
            safe_variant_id: throughput_id,
        })
    }

    /// Reset internal state kept by the `AdaptiveQualitySelector`.
    pub(crate) fn reset(&mut self) {
        self.bandwidth_estimator.reset();
    }

    /// Returns the throughput estimate produced by the `AdaptiveQualitySelector`.
    fn get_estimate(&self) -> f64 {
        self.bandwidth_estimator.get_estimate() * BANDWIDTH_ESTIMATE_FACTOR
    }
}

fn best_variant_id<'a>(
    variants: impl Iterator<Item = &'a VariantStream>,
    bandwidth: f64,
) -> Option<u32> {
    variants
        .filter(|variant| (variant.bandwidth() as f64) <= bandwidth)
        .last()
        .map(|v| v.id())
}

fn fallback_variant_id<'a>(variants: impl Iterator<Item = &'a VariantStream>) -> Option<u32> {
    variants
        .reduce(|best, variant| {
            // Variants are in ascending quality order, so prefer the later one on a tie.
            if variant.bandwidth() <= best.bandwidth() {
                variant
            } else {
                best
            }
        })
        .map(|variant| variant.id())
}

struct BolaCandidate {
    variant_id: u32,
    bandwidth: f64,
    utility: f64,
}

/// Candidates and calibration shared by the buffer-level evaluations of one selection.
enum BolaModel {
    FixedVariant(u32),
    BufferBased {
        candidates: Vec<BolaCandidate>,
        v: f64,
        gamma_p: f64,
    },
}

impl BolaModel {
    fn new(variants: &[&VariantStream], qlow: f64, qmax: f64) -> Option<Self> {
        let all_have_scores = variants.iter().all(|variant| variant.score().is_some());
        if !all_have_scores && variants.iter().any(|variant| variant.score().is_some()) {
            // Missing scores leave BOLA without a consistent utility scale.
            return None;
        }

        let mut scored_candidates = Vec::new();
        let variants = if all_have_scores {
            // BOLA calibration needs increasing cost as quality increases. Keep this
            // filtering local: it must not change track choices or manual variant locks.
            let mut cheapest_higher_quality = None;
            for &variant in variants.iter().rev() {
                if cheapest_higher_quality.is_none_or(|bandwidth| variant.bandwidth() < bandwidth) {
                    scored_candidates.push(variant);
                    cheapest_higher_quality = Some(variant.bandwidth());
                }
            }
            scored_candidates.reverse();
            scored_candidates.as_slice()
        } else {
            variants
        };

        let min_bandwidth = variants.first()?.bandwidth() as f64;
        if min_bandwidth <= 0. || qmax <= qlow {
            return variants
                .first()
                .map(|variant| Self::FixedVariant(variant.id()));
        }

        let candidates: Vec<BolaCandidate> = variants
            .iter()
            .map(|variant| BolaCandidate {
                variant_id: variant.id(),
                bandwidth: variant.bandwidth() as f64,
                utility: if all_have_scores {
                    variant.score().unwrap_or(0.)
                } else {
                    ((variant.bandwidth() as f64) / min_bandwidth).ln()
                },
            })
            .collect();
        let s1 = min_bandwidth;
        let u1 = candidates.first()?.utility;
        let Some(second) = candidates
            .iter()
            .skip(1)
            .find(|candidate| candidate.bandwidth > s1)
        else {
            return candidates
                .last()
                .map(|candidate| Self::FixedVariant(candidate.variant_id));
        };
        let s2 = second.bandwidth;
        let u2 = second.utility;
        let alpha = ((s2 * u1) - (s1 * u2)) / (s2 - s1);
        let u_max = candidates.last()?.utility;
        let denominator = u_max - alpha;
        if denominator <= 0. {
            return candidates
                .last()
                .map(|candidate| Self::FixedVariant(candidate.variant_id));
        }

        let v = (qmax - qlow) / denominator;
        let gamma_p = ((u_max * qlow) - (alpha * qmax)) / (qmax - qlow);

        Some(Self::BufferBased {
            candidates,
            v,
            gamma_p,
        })
    }

    fn variant_id(&self, buffer_level: f64) -> Option<u32> {
        match self {
            Self::FixedVariant(id) => Some(*id),
            Self::BufferBased {
                candidates,
                v,
                gamma_p,
            } => candidates
                .iter()
                .max_by(|a, b| {
                    let objective_a = ((v * (a.utility + gamma_p)) - buffer_level) / a.bandwidth;
                    let objective_b = ((v * (b.utility + gamma_p)) - buffer_level) / b.bandwidth;
                    objective_a
                        .partial_cmp(&objective_b)
                        .unwrap_or(Ordering::Equal)
                })
                .map(|candidate| candidate.variant_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AdaptiveQualitySelector, BolaModel, PlaybackConditions, VariantStream};
    use crate::{parser::TopLevelPlaylist, utils::url::Url};

    fn compute_bola_variant_id(
        variants: &[&VariantStream],
        buffer_level: f64,
        qlow: f64,
        qmax: f64,
    ) -> Option<u32> {
        BolaModel::new(variants, qlow, qmax)?.variant_id(buffer_level)
    }

    fn playback_conditions(
        buffer_level: f64,
        abr_reference_segment_duration: Option<f64>,
    ) -> PlaybackConditions {
        PlaybackConditions {
            buffer_level: Some(buffer_level),
            buffer_goal: 30.,
            playback_speed: 1.,
            abr_reference_segment_duration,
        }
    }

    fn parsed_playlist() -> TopLevelPlaylist {
        TopLevelPlaylist::parse(
            b"#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
low.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=2000000\n\
medium.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=4000000\n\
high.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap()
    }

    fn variants(playlist: &TopLevelPlaylist) -> Vec<&crate::parser::VariantStream> {
        match playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.all_variants().iter().collect(),
            TopLevelPlaylist::DirectMedia(_) => panic!("expected a multivariant playlist"),
        }
    }

    #[test]
    fn bola_model_can_be_reused_for_different_buffer_levels() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let bola = BolaModel::new(&variants, 8., 30.).unwrap();
        for (buffer_level, expected_position) in [(8., 0), (30., 2), (8., 0), (30., 2)] {
            assert_eq!(
                bola.variant_id(buffer_level),
                Some(variants[expected_position].id())
            );
        }
    }

    #[test]
    fn bola_model_preserves_fixed_variant_fallbacks() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        for bola in [
            BolaModel::new(&variants, 8., 8.).unwrap(),
            BolaModel::new(&variants[..1], 8., 30.).unwrap(),
        ] {
            for buffer_level in [0., 8., 20., 30.] {
                assert_eq!(bola.variant_id(buffer_level), Some(variants[0].id()));
            }
        }
    }

    #[test]
    fn empty_variant_pool_has_no_selection() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        assert!(selector
            .select_variant(&[], None, &playback_conditions(30., Some(4.)))
            .is_none());
    }

    #[test]
    fn single_variant_needs_no_throughput_or_playback_information() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(f64::NAN);
        let playback = PlaybackConditions {
            buffer_level: None,
            buffer_goal: f64::NAN,
            playback_speed: f64::NAN,
            abr_reference_segment_duration: None,
        };
        let selected = selector
            .select_variant(&variants[1..2], None, &playback)
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[1].id());
        assert_eq!(selected.safe_variant_id, variants[1].id());
    }

    #[test]
    fn throughput_prefers_the_highest_score_variant() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=6000000,SCORE=3\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        for (initial_bandwidth, expected_position) in [
            (500_000., 1),
            (2_500_000., 1),
            (5_000_000., 1),
            (7_500_000., 2),
        ] {
            let selector = AdaptiveQualitySelector::new(initial_bandwidth);
            for duration in [None, Some(4.)] {
                let selected = selector
                    .select_variant(&variants, None, &playback_conditions(0., duration))
                    .unwrap();
                assert_eq!(selected.best_variant_id, variants[expected_position].id());
                assert_eq!(selected.safe_variant_id, variants[expected_position].id());
            }
        }
    }

    #[test]
    fn throughput_prefers_highest_score_at_the_lowest_bandwidth() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=3\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(500_000.);
        for duration in [None, Some(4.)] {
            let selected = selector
                .select_variant(&variants, None, &playback_conditions(0., duration))
                .unwrap();
            assert_eq!(selected.best_variant_id, variants[2].id());
            assert_eq!(selected.safe_variant_id, variants[2].id());
        }
    }

    #[test]
    fn mixed_scoredness_use_only_throughput_even_with_a_full_buffer() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nunknown.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=2\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        assert_eq!(compute_bola_variant_id(&variants, 30., 8., 30.), None);
        for (initial_bandwidth, expected_position) in [(500_000., 0), (2_500_000., 2)] {
            let selector = AdaptiveQualitySelector::new(initial_bandwidth);
            for current in &variants {
                for buffer_level in [0., 8., 20., 30.] {
                    let selected = selector
                        .select_variant(
                            &variants,
                            Some(current.id()),
                            &playback_conditions(buffer_level, Some(4.)),
                        )
                        .unwrap();
                    assert_eq!(selected.best_variant_id, variants[expected_position].id());
                    assert_eq!(selected.safe_variant_id, variants[expected_position].id());
                }
            }
        }
    }

    #[test]
    fn bola_excludes_more_expensive_lower_quality_candidates() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=3000000,SCORE=3\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(1_250_000.);
        for (buffer_level, expected_position) in [(7., 1), (30., 2)] {
            assert_eq!(
                compute_bola_variant_id(&variants, buffer_level, 8., 30.),
                Some(variants[expected_position].id()),
            );
            let selected = selector
                .select_variant(
                    &variants,
                    None,
                    &playback_conditions(buffer_level, Some(4.)),
                )
                .unwrap();
            assert_eq!(selected.best_variant_id, variants[expected_position].id());
            assert_eq!(selected.safe_variant_id, variants[1].id());
        }
        // BOLA's exclusion does not remove the original candidates.
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].bandwidth(), 4_000_000);
    }

    #[test]
    fn bola_keeps_only_the_highest_score_at_equal_bandwidth() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=3\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        for buffer_level in [8., 20., 30.] {
            assert_eq!(
                compute_bola_variant_id(&variants, buffer_level, 8., 30.),
                Some(variants[2].id()),
            );
        }
    }

    #[test]
    fn bola_ignores_variants_with_higher_bandwidth_and_lower_score_than_another() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=5000000,SCORE=3\nhigh.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=3000000,SCORE=4\nhigher.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=8000000,SCORE=5\nhighest.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let remaining = [variants[1], variants[3], variants[4]];
        for step in 0..=300 {
            let buffer_level = step as f64 / 10.;
            let selected = compute_bola_variant_id(&variants, buffer_level, 8., 30.).unwrap();
            assert_eq!(
                Some(selected),
                compute_bola_variant_id(&remaining, buffer_level, 8., 30.),
            );
            assert!(remaining.iter().any(|variant| variant.id() == selected));
        }
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn upgrade_hysteresis_uses_quality_order_even_when_bandwidth_decreases() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=10000000,SCORE=3\nhigh.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=8000000,SCORE=4\nhighest.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let current = variants[2];
        let buffer_level = (81..=300)
            .map(|step| step as f64 / 10.)
            .find(|buffer_level| {
                let raw = compute_bola_variant_id(&variants, *buffer_level, 8., 30.).unwrap();
                let conservative =
                    compute_bola_variant_id(&variants, (buffer_level - 1.).max(8.), 8., 30.)
                        .unwrap();
                raw == variants[3].id() && conservative == variants[1].id()
            })
            .expect("expected a BOLA transition from medium to highest quality");
        // Fund the current variant so this checks hysteresis rather than the download budget.
        let selector = AdaptiveQualitySelector::new(6_500_000.);
        let selected = selector
            .select_variant(
                &variants,
                Some(current.id()),
                &playback_conditions(buffer_level, Some(4.)),
            )
            .unwrap();
        assert_eq!(selected.best_variant_id, current.id());
        assert_eq!(selected.safe_variant_id, variants[1].id());
    }

    #[test]
    fn uses_throughput_below_the_low_buffer_threshold() {
        let selector = AdaptiveQualitySelector::new(3_125_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected = selector.select_variant(
            &variants,
            Some(variants[0].id()),
            &playback_conditions(2., Some(4.)),
        );

        assert_eq!(selected.unwrap().best_variant_id, variants[1].id());
    }

    #[test]
    fn missing_buffer_information_uses_throughput() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let playback = PlaybackConditions {
            buffer_level: None,
            ..playback_conditions(30., Some(4.))
        };
        for (initial_bandwidth, expected_position) in
            [(500_000., 0), (3_125_000., 1), (5_000_000., 2)]
        {
            let selector = AdaptiveQualitySelector::new(initial_bandwidth);
            for current_id in [None, Some(variants[2].id())] {
                let selected = selector
                    .select_variant(&variants, current_id, &playback)
                    .unwrap();
                assert_eq!(selected.best_variant_id, variants[expected_position].id());
                assert_eq!(selected.safe_variant_id, variants[expected_position].id());
            }
        }
    }

    #[test]
    fn falls_back_to_throughput_when_0_segment_duration() {
        let selector = AdaptiveQualitySelector::new(3_125_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected = selector.select_variant(
            &variants,
            Some(variants[0].id()),
            &playback_conditions(30., Some(0.)),
        );

        assert_eq!(selected.unwrap().best_variant_id, variants[1].id());
    }

    #[test]
    fn bola_is_ignored_when_lower_than_throughput_choice() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        assert_eq!(
            compute_bola_variant_id(&variants, 8., 8., 30.),
            Some(variants[0].id())
        );

        let selected = selector.select_variant(
            &variants,
            Some(variants[2].id()),
            &playback_conditions(8., Some(4.)),
        );

        let selected = selected.unwrap();
        assert_eq!(selected.best_variant_id, variants[2].id());
        assert_eq!(selected.safe_variant_id, variants[2].id());
    }

    #[test]
    fn bola_can_propose_unsustainable_quality() {
        let selector = AdaptiveQualitySelector::new(3_125_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected = selector.select_variant(
            &variants,
            Some(variants[0].id()),
            &playback_conditions(30., Some(4.)),
        );

        let selected = selected.unwrap();
        assert_eq!(selected.best_variant_id, variants[2].id());
        assert_eq!(selected.safe_variant_id, variants[1].id());
    }

    #[test]
    fn bola_does_not_raise_quality_when_high_chance_of_buffer_exhaustion() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=12000000\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(1_250_000.);
        let selected = selector
            .select_variant(
                &variants,
                Some(variants[0].id()),
                &playback_conditions(29., Some(4.)),
            )
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[0].id());
        assert_eq!(selected.safe_variant_id, variants[0].id());
    }

    #[test]
    fn best_variant_above_safe_requires_buffer_margin_that_grows_with_segment_duration() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(1_875_000.);
        for (segment_duration, buffer_level, expected_position) in [
            (1., 5.5, 0),
            (1., 6., 0),
            (1., 6.1, 2),
            (2., 6.5, 0),
            (2., 7., 0),
            (2., 7.1, 2),
        ] {
            let playback = PlaybackConditions {
                buffer_goal: buffer_level,
                ..playback_conditions(buffer_level, Some(segment_duration))
            };
            let selected = selector
                .select_variant(&variants, Some(variants[0].id()), &playback)
                .unwrap();
            assert_eq!(selected.best_variant_id, variants[expected_position].id());
            assert_eq!(selected.safe_variant_id, variants[0].id());
        }
    }

    #[test]
    fn buffer_margin_also_applies_to_keeping_the_current_variant() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let playback = PlaybackConditions {
            buffer_goal: 6.1,
            ..playback_conditions(6., Some(1.))
        };
        let selector = AdaptiveQualitySelector::new(1_875_000.);
        let selected = selector
            .select_variant(&variants, Some(variants[2].id()), &playback)
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[0].id());
        assert_eq!(selected.safe_variant_id, variants[0].id());

        let selector = AdaptiveQualitySelector::new(10_000_000.);
        let selected = selector
            .select_variant(&variants, Some(variants[0].id()), &playback)
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[2].id());
        assert_eq!(selected.safe_variant_id, variants[2].id());
    }

    #[test]
    fn best_variant_above_safe_picks_highest_score_that_fits_in_buffer_and_depends_on_playback_speed(
    ) {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=8000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=5000000,SCORE=3\nhigh.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=12000000,SCORE=4\nhighest.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(1_250_000.);
        for (speed, expected_position) in [(1., 2), (2., 0), (0., 2), (-1., 2)] {
            let playback = PlaybackConditions {
                playback_speed: speed,
                ..playback_conditions(29., Some(4.))
            };
            let selected = selector
                .select_variant(&variants, Some(variants[0].id()), &playback)
                .unwrap();
            assert_eq!(selected.best_variant_id, variants[expected_position].id());
            assert_eq!(selected.safe_variant_id, variants[0].id());
        }
    }

    #[test]
    fn best_variant_above_safe_requires_buffer_to_cover_its_download() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=3000000\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=12000000\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(1_250_000.);
        for (buffer_level, current_position, expected_position) in [
            (29., 0, 1),
            (12.1, 1, 1),
            (11.9, 1, 0),
            (10., 1, 0),
            (9., 1, 0),
            (2., 1, 0),
        ] {
            let selected = selector
                .select_variant(
                    &variants,
                    Some(variants[current_position].id()),
                    &playback_conditions(buffer_level, Some(4.)),
                )
                .unwrap();
            assert_eq!(selected.best_variant_id, variants[expected_position].id());
            assert_eq!(selected.safe_variant_id, variants[0].id());
        }
    }

    #[test]
    fn buffer_margin_only_holds_back_variants_above_throughput_choice() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let current = variants[1];
        let buffer_level = (81..=300)
            .map(|step| step as f64 / 10.)
            .find(|buffer_level| {
                let raw = compute_bola_variant_id(&variants, *buffer_level, 8., 30.).unwrap();
                let conservative =
                    compute_bola_variant_id(&variants, (buffer_level - 1.).max(8.), 8., 30.)
                        .unwrap();
                raw == variants[2].id() && conservative == current.id()
            })
            .expect("expected a BOLA transition between medium and high");

        for (initial_bandwidth, expected) in
            [(3_125_000., current.id()), (5_000_000., variants[2].id())]
        {
            let selector = AdaptiveQualitySelector::new(initial_bandwidth);
            let selected = selector
                .select_variant(
                    &variants,
                    Some(current.id()),
                    &playback_conditions(buffer_level, Some(4.)),
                )
                .unwrap();

            assert_eq!(selected.best_variant_id, expected);
            assert_eq!(selected.safe_variant_id, expected);
        }
    }

    #[test]
    fn duplicate_bandwidths_do_not_disable_bola() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
low-a.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=1000000\n\
low-b.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=4000000\n\
high.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let variants = variants(&playlist);

        let selected = selector.select_variant(
            &variants,
            Some(variants[1].id()),
            &playback_conditions(30., Some(4.)),
        );

        assert_eq!(selected.unwrap().best_variant_id, variants[2].id());
    }

    #[test]
    fn startup_selection_uses_throughput_without_segment_timing() {
        let selector = AdaptiveQualitySelector::new(3_125_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        let selected = selector
            .select_variant(&variants, None, &playback_conditions(30., None))
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[1].id());
        assert_eq!(selected.safe_variant_id, variants[1].id());
    }

    #[test]
    fn bola_lower_choice_is_ignored_when_variants_have_scores() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=3\nhigh.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=4\nhighest.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        assert_eq!(
            compute_bola_variant_id(&variants, 8.5, 8., 30.),
            Some(variants[2].id())
        );
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let selected = selector
            .select_variant(
                &variants,
                Some(variants[3].id()),
                &playback_conditions(8.5, Some(4.)),
            )
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[3].id());
        assert_eq!(selected.safe_variant_id, variants[3].id());
    }

    #[test]
    fn best_uses_score_not_bandwidth_or_variant_id() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=3\nhigh.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,SCORE=2\nmedium.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let selected = selector
            .select_variant(&variants, None, &playback_conditions(30., Some(4.)))
            .unwrap();
        assert!(variants[2].bandwidth() < variants[1].bandwidth());
        assert!(variants[2].id() < variants[1].id());
        assert_eq!(selected.best_variant_id, variants[2].id());
        assert_eq!(selected.safe_variant_id, variants[2].id());
    }

    #[test]
    fn best_quality_never_falls_below_the_safe_throughput_choice() {
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        for (initial_bandwidth, expected_safe_position) in
            [(500_000., 0), (3_125_000., 1), (5_000_000., 2)]
        {
            let selector = AdaptiveQualitySelector::new(initial_bandwidth);
            for current_id in [
                None,
                Some(variants[0].id()),
                Some(variants[1].id()),
                Some(variants[2].id()),
            ] {
                for buffer_level in [0., 2., 8., 12., 20., 30.] {
                    for duration in [None, Some(0.), Some(4.)] {
                        let selected = selector
                            .select_variant(
                                &variants,
                                current_id,
                                &playback_conditions(buffer_level, duration),
                            )
                            .unwrap();
                        assert_eq!(selected.safe_variant_id, variants[expected_safe_position].id(), "safe choice changed at bandwidth {initial_bandwidth}, current {current_id:?}, buffer {buffer_level}, duration {duration:?}");
                        let best_position = variants
                            .iter()
                            .position(|variant| variant.id() == selected.best_variant_id)
                            .unwrap();
                        let safe_position = variants
                            .iter()
                            .position(|variant| variant.id() == selected.safe_variant_id)
                            .unwrap();
                        assert!(safe_position <= best_position, "replacement quality exceeds additive quality at bandwidth {initial_bandwidth}, current {current_id:?}, buffer {buffer_level}, duration {duration:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn throughput_selection_scales_with_playback_speed() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        for (speed, expected) in [
            (1., 2),
            (2., 1),
            (4., 0),
            (0., 2),
            (-1., 2),
            (f64::NAN, 2),
            (f64::INFINITY, 2),
        ] {
            let playback = PlaybackConditions {
                playback_speed: speed,
                ..playback_conditions(0., None)
            };
            let selected = selector.select_variant(&variants, None, &playback).unwrap();
            assert_eq!(selected.best_variant_id, variants[expected].id());
            assert_eq!(selected.safe_variant_id, variants[expected].id());
        }
    }

    #[test]
    fn throughput_selection_uses_new_download_metrics() {
        let mut selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);
        assert_eq!(
            selector
                .select_variant(&variants, None, &playback_conditions(0., None))
                .unwrap()
                .best_variant_id,
            variants[2].id()
        );
        selector.add_metric(1_000., 200_000);
        assert_eq!(
            selector
                .select_variant(&variants, None, &playback_conditions(0., None))
                .unwrap()
                .best_variant_id,
            variants[0].id()
        );
    }
}
