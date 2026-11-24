use std::{iter::Map, ops::Index, slice::Chunks};

use crate::utils::logger::*;

pub struct JsTimeRanges {
    buffered: Vec<f64>,
}

impl JsTimeRanges {
    pub fn new(buffered: Vec<f64>) -> Self {
        if !buffered.len().is_multiple_of(2) {
            log_error!("Incorrect JsTimeRanges object");
            Self { buffered: vec![] }
        } else {
            Self { buffered }
        }
    }

    pub fn len(&self) -> usize {
        self.buffered.len() / 2
    }

    pub fn start(&self, idx: usize) -> Option<f64> {
        self.buffered.get(idx * 2).copied()
    }

    pub unsafe fn start_unchecked(&self, idx: usize) -> f64 {
        self.buffered[idx * 2]
    }

    pub fn end(&self, idx: usize) -> Option<f64> {
        self.buffered.get((idx * 2) + 1).copied()
    }

    pub unsafe fn end_unchecked(&self, idx: usize) -> f64 {
        self.buffered[idx * 2 + 1]
    }
}

impl Index<usize> for JsTimeRanges {
    type Output = [f64; 2];
    fn index(&self, index: usize) -> &Self::Output {
        &self.buffered.as_slice().as_chunks::<2>().0[index]
    }
}

#[cfg(test)]
mod tests {
    use super::JsTimeRanges;

    #[test]
    fn indexes_start_end_pairs() {
        let ranges = JsTimeRanges::new(vec![1.0, 2.0, 3.0, 4.0]);

        assert_eq!(ranges[0], [1.0, 2.0]);
        assert_eq!(ranges[1], [3.0, 4.0]);
    }
}

impl JsTimeRanges {
    pub(crate) fn range(&self, idx: usize) -> Option<(f64, f64)> {
        self.buffered
            .get(idx * 2)
            .and_then(|s| self.buffered.get((idx * 2) + 1).map(|e| (*s, *e)))
    }

    pub(crate) fn range_unchecked(&self, idx: usize) -> (f64, f64) {
        (self.buffered[idx * 2], self.buffered[(idx * 2) + 1])
    }

    pub(crate) fn range_for(&self, pos: f64) -> Option<(f64, f64)> {
        for range in self.into_iter() {
            if pos < range.1 {
                return if pos >= range.0 { Some(range) } else { None };
            }
        }
        None
    }

    pub(crate) fn buffer_gap(&self, pos: f64) -> Option<f64> {
        self.range_for(pos).map(|r| r.1 - pos)
    }
}

impl<'a> IntoIterator for &'a JsTimeRanges {
    type Item = (f64, f64);

    // Yep, not easy to look at. Maybe future Rust feature can simplify that mess
    type IntoIter = Map<Chunks<'a, f64>, fn(&'a [f64]) -> (f64, f64)>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffered.chunks(2).map(|vals| (vals[0], vals[1]))
    }
}
