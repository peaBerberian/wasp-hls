use self::bandwidth_estimator::BandwithEstimator;

mod bandwidth_estimator;
mod ewma;

pub struct AdaptiveQualitySelector {
    bandwidth_estimator: BandwithEstimator,
}

const ADAPTIVE_FACTOR : f64 = 0.8;

impl AdaptiveQualitySelector {
    pub fn new() -> Self {
        Self {
            bandwidth_estimator: BandwithEstimator::new(),
        }
    }

    pub fn add_metric(&mut self, duration_ms: f64, size_bytes: u32) {
        self.bandwidth_estimator.add_sample(duration_ms, size_bytes);
    }

    pub fn get_estimate(&self) -> Option<f64> {
        self.bandwidth_estimator.get_estimate().map(|x| {
            x * ADAPTIVE_FACTOR
        })
    }
}
