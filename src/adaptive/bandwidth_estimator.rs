use super::ewma::Ewma;

const FAST_EWMA_HALF_LIFE : u32 = 2;
const SLOW_EWMA_HALF_LIFE : u32 = 10;
const MINIMUM_CHUNK_SIZE : u32 = 16_000;
const MINIMUM_TOTAL_BYTES : u64 = 150_000;

pub struct BandwithEstimator {
    fast_ewma: Ewma,
    slow_ewma: Ewma,
    bytes_sampled: u64,
}

impl BandwithEstimator {
    pub fn new() -> Self {
        Self {
            fast_ewma: Ewma::new(FAST_EWMA_HALF_LIFE),
            slow_ewma: Ewma::new(SLOW_EWMA_HALF_LIFE),
            bytes_sampled: 0,
        }
    }

    pub fn add_sample(&mut self, duration_ms: f64, size_bytes: u32) {
        if size_bytes < MINIMUM_CHUNK_SIZE {
            return;
        }
        let bandwidth = (size_bytes as f64) * 8000. / duration_ms;
        let weight = duration_ms / 1000.;
        self.bytes_sampled += size_bytes as u64;
        self.fast_ewma.add_sample(weight, bandwidth);
        self.slow_ewma.add_sample(weight, bandwidth);
    }

    pub fn get_estimate(&self) -> Option<f64> {
        if self.bytes_sampled < MINIMUM_TOTAL_BYTES {
            None
        } else {
            Some(self.fast_ewma.get_estimate().min(self.slow_ewma.get_estimate()))
        }
    }

    pub fn reset(&mut self) {
        self.fast_ewma = Ewma::new(FAST_EWMA_HALF_LIFE);
        self.slow_ewma = Ewma::new(SLOW_EWMA_HALF_LIFE);
        self.bytes_sampled = 0;
    }
}


