/// Exponentially-weighted moving average.
///
/// Average considering a "weight" for each estimates and prioritizing the last samples added.
/// This is useful in media streaming where you want to calculate a continuous bandwidth
/// average, while putting more importance to the last loaded data.
pub struct Ewma {
    alpha: f64,
    last_estimate: f64,
    total_weight: f64,
}

impl Ewma {
    /// Creates a new Ewma with the given "half life", in seconds.
    pub fn new(half_life: u32) -> Self {
        Self {
            alpha: f64::exp(0.5f64.ln() / f64::from(half_life)),
            last_estimate: 0.,
            total_weight: 0.,
        }
    }

    /// Adds new sample to the `Ewma` where `val` is the value to add and `weight` is its...
    /// weight.
    pub fn add_sample(&mut self, weight: f64, val: f64) {
        let adj_alpha = self.alpha.powf(weight);
        let new_estimate = val * (1. - adj_alpha) + adj_alpha * self.last_estimate;
        self.last_estimate = new_estimate;
        self.total_weight += weight;
    }

    /// Get the current estimate produced by the `Ewma`.
    ///
    /// Returns `0.` if it cannot produce an estimate yet.
    pub fn get_estimate(&self) -> f64 {
        if self.total_weight == 0. {
            0.
        } else {
            let zero_factor = 1. - self.alpha.powf(self.total_weight);
            self.last_estimate / zero_factor
        }
    }
}
