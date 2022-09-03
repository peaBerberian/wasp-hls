pub struct Ewma {
    alpha: f64,
    last_estimate: f64,
    total_weight: f64,
}

impl Ewma {
    pub fn new(half_life: u32) -> Self {
        Self {
            alpha: f64::exp(0.5f64.ln() / half_life as f64),
            last_estimate: 0.,
            total_weight: 0.,
        }
    }

    pub fn add_sample(&mut self, weight: f64, val: f64) {
        let adj_alpha = self.alpha.powf(weight);
        let new_estimate = val * (1. - adj_alpha) +
            adj_alpha * self.last_estimate;
        self.last_estimate = new_estimate;
        self.total_weight += weight;
    }

    pub fn get_estimate(&self) -> f64 {
        if self.total_weight == 0. {
            0.
        } else {
            let zero_factor = 1. - self.alpha.powf(self.total_weight);
            self.last_estimate / zero_factor
        }
    }
}
