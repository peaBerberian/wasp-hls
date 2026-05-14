use std::cmp::Ordering;

use self::bandwidth_estimator::BandwithEstimator;
use crate::{media_element::SegmentQualityContext, parser::VariantStream};

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
        segment_duration: Option<f64>,
    ) -> Option<u32> {
        if variants.is_empty() {
            return None;
        }
        let throughput_id = best_variant_id(variants.iter().copied(), bandwidth)
            .or_else(|| fallback_variant_id(variants.iter().copied()))?;
        let Some(segment_duration) = segment_duration.filter(|d| d.is_finite() && *d > 0.) else {
            return Some(throughput_id);
        };

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
        let bola_bandwidth = variants
            .iter()
            .find(|v| v.id() == bola_id)
            .map(|v| v.bandwidth())?;
        let throughput_bandwidth = variants
            .iter()
            .find(|v| v.id() == throughput_id)
            .map(|v| v.bandwidth())?;

        let mid_buffer = qlow + ((qmax - qlow) / 2.);
        if bola_bandwidth > throughput_bandwidth && normalized_buffer < mid_buffer {
            return Some(throughput_id);
        }

        if let Some(curr_id) = current_variant_id {
            let current_bandwidth = variants
                .iter()
                .find(|v| v.id() == curr_id)
                .map(|v| v.bandwidth());
            if current_bandwidth == Some(bola_bandwidth)
                || current_bandwidth == Some(throughput_bandwidth)
            {
                return Some(curr_id);
            }
        }

        Some(bola_id)
    }

    /// Decide whether an in-flight higher-quality segment should be abandoned in favor of the
    /// currently desired lower-quality one.
    pub(crate) fn should_abandon_media_request(
        &self,
        pending_quality: &SegmentQualityContext,
        desired_quality: &SegmentQualityContext,
        pending_variant_bandwidth: u64,
        desired_variant_bandwidth: u64,
        pending_segment_duration: f64,
        buffer_level: f64,
    ) -> bool {
        if !pending_quality.is_better_than(desired_quality)
            || desired_variant_bandwidth >= pending_variant_bandwidth
            || pending_segment_duration <= 0.
        {
            return false;
        }

        let bandwidth = self.get_estimate();
        if bandwidth <= 0. {
            return false;
        }

        let pending_download_time =
            (pending_segment_duration * pending_variant_bandwidth as f64) / bandwidth;
        let replacement_download_time =
            (pending_segment_duration * desired_variant_bandwidth as f64) / bandwidth;

        pending_download_time > buffer_level
            && replacement_download_time + (pending_segment_duration * 0.25) < pending_download_time
    }

    pub(crate) fn reset(&mut self) {
        self.bandwidth_estimator.reset();
    }
}

fn best_variant_id<'a>(
    variants: impl DoubleEndedIterator<Item = &'a VariantStream>,
    bandwidth: f64,
) -> Option<u32> {
    variants
        .rev()
        .find(|x| (x.bandwidth() as f64) <= bandwidth)
        .map(|v| v.id())
}

fn fallback_variant_id<'a>(variants: impl Iterator<Item = &'a VariantStream>) -> Option<u32> {
    variants
        .fold(None, |acc, v| {
            if let Some((bandwidth, _)) = acc {
                if v.bandwidth() <= bandwidth {
                    Some((v.bandwidth(), v.id()))
                } else {
                    acc
                }
            } else {
                Some((v.bandwidth(), v.id()))
            }
        })
        .map(|r| r.1)
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

    let utilities: Vec<f64> = variants
        .iter()
        .map(|variant| {
            variant
                .score()
                .unwrap_or_else(|| ((variant.bandwidth() as f64) / min_bandwidth).ln())
                .max(0.)
        })
        .collect();
    let s1 = min_bandwidth;
    let s2 = variants.get(1).map(|v| v.bandwidth() as f64)?;
    if s2 <= s1 {
        return variants.first().map(|v| v.id());
    }

    let u1 = utilities[0];
    let u2 = utilities[1];
    let alpha = ((s1 * u2) - (s2 * u1)) / (s2 - s1);
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
