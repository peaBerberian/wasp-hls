use super::ewma::Ewma;

const FAST_EWMA_HALF_LIFE : u32 = 2;
const SLOW_EWMA_HALF_LIFE : u32 = 10;
const MINIMUM_CHUNK_SIZE : u32 = 16_000;
const MINIMUM_TOTAL_BYTES : u64 = 150_000;

/// Produce bandwidth estimates based on two EWMA (exponentially-weighted moving average), one
/// evolving slow and the other evolving fast.
///
/// The minimum between both is then taken into consideration to ensure a sudden fall in bandwidth
/// has a lasting impact on estimates and that we only raise that estimate once it raised for
/// enough time.
pub struct BandwithEstimator {
    fast_ewma: Ewma,
    slow_ewma: Ewma,
    bytes_sampled: u64,
}

impl BandwithEstimator {
    /// Creates a new `BandwithEstimator`
    pub fn new() -> Self {
        Self {
            fast_ewma: Ewma::new(FAST_EWMA_HALF_LIFE),
            slow_ewma: Ewma::new(SLOW_EWMA_HALF_LIFE),
            bytes_sampled: 0,
        }
    }

    /// Feed the BandwithEstimator a new bandwidth data sample.
    ///
    /// You may want to call this method after a new resource was loaded, in the
    /// case where you want to consider this request in the whole bandwidth
    /// estimation logic.
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

    /// Get the current estimate made by the `BandwithEstimator`.
    ///
    /// Returns `None` if it does not have enough data to produce a estimate yet.
    pub fn get_estimate(&self) -> Option<f64> {
        if self.bytes_sampled < MINIMUM_TOTAL_BYTES {
            None
        } else {
            Some(self.fast_ewma.get_estimate().min(self.slow_ewma.get_estimate()))
        }
    }

    /// Reset the `BandwithEstimator` as if there was no sample added yet.
    pub fn reset(&mut self) {
        self.fast_ewma = Ewma::new(FAST_EWMA_HALF_LIFE);
        self.slow_ewma = Ewma::new(SLOW_EWMA_HALF_LIFE);
        self.bytes_sampled = 0;
    }
}


