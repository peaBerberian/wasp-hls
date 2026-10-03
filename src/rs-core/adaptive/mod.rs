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
    pub(crate) best_variant_id: u32,
    /// Variant ID that should be selected if *replacing* buffered positions
    /// (e.g. you might want to do that to visually raise in quality more
    /// quickly). "Safe" because we're unlikely to enter a rebuffering period
    /// under that variant.
    pub(crate) safe_variant_id: u32,
}

/// Playback information used to choose an adaptive variant.
pub(crate) struct PlaybackConditions {
    /// Current amount of buffered data in front of the current position
    pub(crate) buffer_level: f64,
    pub(crate) buffer_goal: f64,
    pub(crate) playback_speed: f64,
    /// Maximum target segment duration across the currently selected media playlists.
    pub(crate) max_target_segment_duration: Option<f64>,
}

const ADAPTIVE_FACTOR: f64 = 0.8;
const BOLA_MIN_LOW_BUFFER: f64 = 3.0;
const BOLA_MAX_LOW_BUFFER: f64 = 10.0;
const BOLA_UP_SWITCH_HYSTERESIS: f64 = 0.25;

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

    /// Returns the throughput estimate produced by the `AdaptiveQualitySelector`.
    fn get_estimate(&self) -> f64 {
        self.bandwidth_estimator.get_estimate() * ADAPTIVE_FACTOR
    }

    /// Select the best variant by composing the throughput estimate with a BOLA-style
    /// buffer-occupancy rule.
    /// `compatible_variants` must preserve the playlist's ascending quality order.
    pub(crate) fn select_variant(
        &self,
        compatible_variants: &[&VariantStream],
        current_variant_id: Option<u32>,
        playback: &PlaybackConditions,
    ) -> Option<AdaptiveVariantSelection> {
        let variants = compatible_variants;
        if variants.is_empty() {
            return None;
        }
        let estimate = self.get_estimate();
        let bandwidth = if playback.playback_speed.is_finite() && playback.playback_speed > 0. {
            estimate / playback.playback_speed
        } else {
            estimate
        };
        let throughput_id = best_variant_id(variants.iter().copied(), bandwidth)
            .or_else(|| fallback_variant_id(variants.iter().copied()))?;
        let segment_duration = playback.max_target_segment_duration.unwrap_or(0.);
        if !segment_duration.is_finite() || segment_duration <= 0. {
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        }

        if variants.len() == 1 {
            return variants.first().map(|variant| AdaptiveVariantSelection {
                best_variant_id: variant.id(),
                safe_variant_id: throughput_id,
            });
        }

        let qmax = playback.buffer_goal.max(segment_duration);
        let qlow = (segment_duration * 2.)
            .clamp(BOLA_MIN_LOW_BUFFER, BOLA_MAX_LOW_BUFFER)
            .min((qmax - 0.1).max(segment_duration));
        let normalized_buffer = playback.buffer_level.max(0.).min(qmax);
        if normalized_buffer < qlow {
            return Some(AdaptiveVariantSelection {
                best_variant_id: throughput_id,
                safe_variant_id: throughput_id,
            });
        }

        let mut bola_id = compute_bola_variant_id(variants, normalized_buffer, qlow, qmax)?;
        if let Some(current_variant) =
            current_variant_id.and_then(|id| variants.iter().find(|variant| variant.id() == id))
        {
            let bola_variant = variants.iter().find(|variant| variant.id() == bola_id)?;
            if bola_variant.bandwidth() > current_variant.bandwidth() {
                let conservative_buffer =
                    (normalized_buffer - (segment_duration * BOLA_UP_SWITCH_HYSTERESIS)).max(qlow);
                let conservative_id =
                    compute_bola_variant_id(variants, conservative_buffer, qlow, qmax)?;
                let conservative_variant = variants
                    .iter()
                    .find(|variant| variant.id() == conservative_id)?;
                if conservative_variant.bandwidth() <= current_variant.bandwidth() {
                    bola_id = current_variant.id();
                }
            }
        }

        let best_id = variants
            .iter()
            .rev()
            .find(|variant| variant.id() == bola_id || variant.id() == throughput_id)?
            .id();

        Some(AdaptiveVariantSelection {
            best_variant_id: best_id,
            safe_variant_id: throughput_id,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.bandwidth_estimator.reset();
    }
}

fn best_variant_id<'a>(
    variants: impl Iterator<Item = &'a VariantStream>,
    bandwidth: f64,
) -> Option<u32> {
    variants
        .filter(|variant| (variant.bandwidth() as f64) <= bandwidth)
        .max_by_key(|variant| variant.bandwidth())
        .map(|v| v.id())
}

fn fallback_variant_id<'a>(variants: impl Iterator<Item = &'a VariantStream>) -> Option<u32> {
    variants
        .min_by_key(|variant| variant.bandwidth())
        .map(|variant| variant.id())
}

fn compute_bola_variant_id(
    variants: &[&VariantStream],
    buffer_level: f64,
    qlow: f64,
    qmax: f64,
) -> Option<u32> {
    let min_bandwidth = variants.first()?.bandwidth() as f64;
    if min_bandwidth <= 0. || qmax <= qlow {
        return variants.first().map(|v| v.id());
    }

    let all_have_scores = variants.iter().all(|variant| variant.score().is_some());
    let utilities: Vec<f64> = variants
        .iter()
        .map(|variant| {
            if all_have_scores {
                variant.score().unwrap_or(0.)
            } else {
                ((variant.bandwidth() as f64) / min_bandwidth).ln()
            }
        })
        .collect();
    let s1 = min_bandwidth;
    let u1 = utilities[0];
    let Some((s2, u2)) =
        variants
            .iter()
            .zip(utilities.iter())
            .skip(1)
            .find_map(|(variant, utility)| {
                let bandwidth = variant.bandwidth() as f64;
                (bandwidth > s1).then_some((bandwidth, *utility))
            })
    else {
        return variants.last().map(|variant| variant.id());
    };
    let alpha = ((s2 * u1) - (s1 * u2)) / (s2 - s1);
    let u_max = *utilities.last()?;
    let denominator = u_max - alpha;
    if denominator <= 0. {
        return variants.last().map(|v| v.id());
    }

    let v = (qmax - qlow) / denominator;
    let gamma_p = ((u_max * qlow) - (alpha * qmax)) / (qmax - qlow);

    variants
        .iter()
        .zip(utilities.iter())
        .max_by(|(variant_a, utility_a), (variant_b, utility_b)| {
            let objective_a =
                ((v * (*utility_a + gamma_p)) - buffer_level) / (variant_a.bandwidth() as f64);
            let objective_b =
                ((v * (*utility_b + gamma_p)) - buffer_level) / (variant_b.bandwidth() as f64);
            objective_a
                .partial_cmp(&objective_b)
                .unwrap_or(Ordering::Equal)
        })
        .map(|(variant, _)| variant.id())
}

#[cfg(test)]
mod tests {
    use super::{compute_bola_variant_id, AdaptiveQualitySelector, PlaybackConditions};
    use crate::{parser::TopLevelPlaylist, utils::url::Url};

    fn playback_conditions(
        buffer_level: f64,
        max_target_segment_duration: Option<f64>,
    ) -> PlaybackConditions {
        PlaybackConditions {
            buffer_level,
            buffer_goal: 30.,
            playback_speed: 1.,
            max_target_segment_duration,
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
    fn falls_back_to_throughput_with_an_invalid_segment_duration() {
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
    fn bola_cannot_lower_the_throughput_choice() {
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
    fn buffer_can_fund_an_unsustainable_quality_increase() {
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
    fn buffer_margin_only_delays_upgrades_above_the_throughput_choice() {
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
    fn throughput_choice_is_not_capped_by_bola_with_scored_variants() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,SCORE=2\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,SCORE=3\nhigh.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_string()),
        ).unwrap();
        let variants = variants(&playlist);
        assert_eq!(
            compute_bola_variant_id(&variants, 8., 8., 30.),
            Some(variants[1].id())
        );
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let selected = selector
            .select_variant(
                &variants,
                Some(variants[2].id()),
                &playback_conditions(8., Some(4.)),
            )
            .unwrap();
        assert_eq!(selected.best_variant_id, variants[2].id());
        assert_eq!(selected.safe_variant_id, variants[2].id());
    }

    #[test]
    fn additive_choice_uses_playlist_quality_not_bandwidth_or_variant_id() {
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
        assert_eq!(selected.safe_variant_id, variants[1].id());
    }

    #[test]
    fn additive_quality_never_falls_below_the_safe_throughput_choice() {
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
    fn selection_uses_new_download_metrics() {
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
