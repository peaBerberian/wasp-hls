use std::cmp::Ordering;

use self::bandwidth_estimator::BandwithEstimator;
use crate::parser::VariantStream;

mod bandwidth_estimator;
mod ewma;

/// Produces Bandwith estimates allowing a more educated guess for the current variant stream
/// selected.
pub(crate) struct AdaptiveQualitySelector {
    bandwidth_estimator: BandwithEstimator,
}

const ADAPTIVE_FACTOR: f64 = 0.8;
const BOLA_MIN_LOW_BUFFER: f64 = 3.0;
const BOLA_MAX_LOW_BUFFER: f64 = 10.0;

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
    pub(crate) fn get_estimate(&self) -> f64 {
        self.bandwidth_estimator.get_estimate() * ADAPTIVE_FACTOR
    }

    /// Select the best variant by composing the throughput estimate with a BOLA-style
    /// buffer-occupancy rule.
    pub(crate) fn select_variant(
        &self,
        variants: &[&VariantStream],
        current_variant_id: Option<u32>,
        bandwidth: f64,
        buffer_level: f64,
        buffer_goal: f64,
        segment_duration: f64,
    ) -> Option<u32> {
        if variants.is_empty() {
            return None;
        }
        let throughput_id = best_variant_id(variants.iter().copied(), bandwidth)
            .or_else(|| fallback_variant_id(variants.iter().copied()))?;
        if !segment_duration.is_finite() || segment_duration <= 0. {
            return Some(throughput_id);
        }

        if variants.len() == 1 {
            return variants.first().map(|v| v.id());
        }

        let qmax = buffer_goal.max(segment_duration);
        let qlow = (segment_duration * 2.)
            .clamp(BOLA_MIN_LOW_BUFFER, BOLA_MAX_LOW_BUFFER)
            .min((qmax - 0.1).max(segment_duration));
        let normalized_buffer = buffer_level.max(0.).min(qmax);
        if normalized_buffer < qlow {
            return Some(throughput_id);
        }

        let bola_id = compute_bola_variant_id(variants, normalized_buffer, qlow, qmax)?;
        let bola_index = variants.iter().position(|v| v.id() == bola_id)?;
        let throughput_index = variants.iter().position(|v| v.id() == throughput_id)?;

        // BOLA-O avoids oscillation by preventing an up-switch beyond both the current
        // representation and what the throughput estimate can sustain. Buffer-driven
        // down-switches are still applied immediately.
        if let Some(current_index) =
            current_variant_id.and_then(|id| variants.iter().position(|variant| variant.id() == id))
        {
            if bola_index > current_index && bola_index > throughput_index {
                return variants
                    .get(current_index.max(throughput_index))
                    .map(|variant| variant.id());
            }
        }

        Some(bola_id)
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
    use super::AdaptiveQualitySelector;
    use crate::{parser::TopLevelPlaylist, utils::url::Url};

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
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected =
            selector.select_variant(&variants, Some(variants[0].id()), 2_500_000., 2., 30., 4.);

        assert_eq!(selected, Some(variants[1].id()));
    }

    #[test]
    fn falls_back_to_throughput_with_an_invalid_segment_duration() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected =
            selector.select_variant(&variants, Some(variants[0].id()), 2_500_000., 30., 30., 0.);

        assert_eq!(selected, Some(variants[1].id()));
    }

    #[test]
    fn buffer_pressure_can_switch_below_the_throughput_choice() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected =
            selector.select_variant(&variants, Some(variants[2].id()), 5_000_000., 8., 30., 4.);

        assert_eq!(selected, Some(variants[0].id()));
    }

    #[test]
    fn bola_o_caps_an_unsustainable_quality_increase() {
        let selector = AdaptiveQualitySelector::new(5_000_000.);
        let playlist = parsed_playlist();
        let variants = variants(&playlist);

        let selected =
            selector.select_variant(&variants, Some(variants[0].id()), 2_500_000., 30., 30., 4.);

        assert_eq!(selected, Some(variants[1].id()));
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

        let selected =
            selector.select_variant(&variants, Some(variants[1].id()), 5_000_000., 30., 30., 4.);

        assert_eq!(selected, Some(variants[2].id()));
    }
}
