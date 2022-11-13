use self::bandwidth_estimator::BandwithEstimator;

mod bandwidth_estimator;
mod ewma;

/// Produces Bandwith estimates allowing a more educated guess for the current variant stream
/// selected.
pub struct AdaptiveQualitySelector {
    bandwidth_estimator: BandwithEstimator,
}

const ADAPTIVE_FACTOR : f64 = 0.8;

impl AdaptiveQualitySelector {
    /// Creates new `AdaptiveQualitySelector`.
    pub fn new() -> Self {
        Self {
            bandwidth_estimator: BandwithEstimator::new(),
        }
    }

    /// Adds metric allowing the `AdaptiveQualitySelector` to provide more educated guesses.
    /// Here, `duration_ms` should correspond to the time taken to make a request and `size_bytes`
    /// should be the corresponding size of loaded data.
    pub fn add_metric(&mut self, duration_ms: f64, size_bytes: u32) {
        self.bandwidth_estimator.add_sample(duration_ms, size_bytes);
    }

    /// Returns the current estimate produced by the `AdaptiveQualitySelector`.
    ///
    /// Returns `None` if it does not have enough data to produce an estimate yet.
    pub fn get_estimate(&self) -> Option<f64> {
        self.bandwidth_estimator.get_estimate().map(|x| {
            x * ADAPTIVE_FACTOR
        })
    }
}
