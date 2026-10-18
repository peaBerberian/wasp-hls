/// Represent a range of time, from a start to an end, generally in seconds
#[derive(Clone, Debug)]
pub struct TimeRange {
    start: f64,
    end: f64,
}

impl TimeRange {
    /// Returns the start time of the range
    pub(crate) fn start(&self) -> f64 {
        self.start
    }
    /// Returns the end time of the range
    pub(crate) fn end(&self) -> f64 {
        self.end
    }
}

/// Abstracts non-contiguous chronological ranges of time, generally expressed in seconds.
#[derive(Clone, Debug, Default)]
pub(crate) struct TimeRanges {
    ranges: Vec<TimeRange>,
}

impl TimeRanges {
    /// Create a new empty `TimeRanges` object
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Add a range of time to that `TimeRanges` object, merging it with the ranges that are
    /// already there.
    pub(crate) fn add(&mut self, mut start: f64, mut end: f64) {
        let first = self
            .ranges
            .iter()
            .position(|range| range.end >= start)
            .unwrap_or(self.ranges.len());
        let mut after_last = first;
        while let Some(range) = self.ranges.get(after_last) {
            if range.start > end {
                break;
            }
            start = start.min(range.start);
            end = end.max(range.end);
            after_last += 1;
        }
        // Replace only the ranges that overlap or touch the new range.
        self.ranges
            .splice(first..after_last, [TimeRange { start, end }]);
    }

    /// Remove time range at a particular index
    pub(crate) fn remove(&mut self, idx: usize) -> TimeRange {
        self.ranges.remove(idx)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Returns the number of non-contiguous ranges in this `TimeRanges` object
    pub(crate) fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Returns the starting time of the range whose index is given in argument.
    ///
    /// Returns `None` if the given index is superior or equal to the number of actual ranges.
    pub(crate) fn start(&self, idx: usize) -> Option<f64> {
        self.ranges.get(idx).map(|r| r.start)
    }

    pub(crate) unsafe fn start_unchecked(&self, idx: usize) -> f64 {
        self.ranges[idx].start
    }

    /// Returns the ending time of the range whose index is given in argument.
    ///
    /// Returns `None` if the given index is superior or equal to the number of actual ranges.
    pub(crate) fn end(&self, idx: usize) -> Option<f64> {
        self.ranges.get(idx).map(|r| r.end)
    }

    pub(crate) unsafe fn end_unchecked(&self, idx: usize) -> f64 {
        self.ranges[idx].end
    }

    /// Returns the range containing the given position.
    ///
    /// Returns `None` if no range in this `TimeRanges` object contains it.
    pub(crate) fn range_for(&self, pos: f64) -> Option<&TimeRange> {
        for range in self.into_iter() {
            if pos < range.end {
                return if pos >= range.start {
                    Some(range)
                } else {
                    None
                };
            }
        }
        None
    }

    pub(crate) fn clear(&mut self) {
        self.ranges.clear()
    }

    pub(crate) fn drain(&mut self, itv: Range<usize>) {
        self.ranges.drain(itv);
    }
}

use std::{
    ops::{Index, Range},
    slice::Iter,
};

impl Index<usize> for TimeRanges {
    type Output = TimeRange;
    fn index(&self, index: usize) -> &Self::Output {
        &self.ranges[index]
    }
}

impl<'a> IntoIterator for &'a TimeRanges {
    type Item = &'a TimeRange;

    // Yep, not easy to look at. Maybe future Rust feature can simplify that mess
    type IntoIter = Iter<'a, TimeRange>;

    fn into_iter(self) -> Self::IntoIter {
        self.ranges.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::TimeRanges;

    #[test]
    fn merging_matches_covered_intervals_in_every_insertion_order() {
        let inputs: Vec<_> = (0..5)
            .flat_map(|start| (start + 1..=5).map(move |end| (start, end)))
            .collect();
        for a in &inputs {
            for b in &inputs {
                for c in &inputs {
                    let mut ranges = TimeRanges::new();
                    let mut covered = [false; 5];
                    for &(start, end) in [a, b, c] {
                        ranges.add(start as f64, end as f64);
                        covered[start..end].fill(true);
                    }
                    // Build the expected ranges from individual covered intervals,
                    // independently of the range-merging algorithm.
                    let mut expected = Vec::new();
                    let mut index = 0;
                    while index < covered.len() {
                        if !covered[index] {
                            index += 1;
                            continue;
                        }
                        let start = index;
                        while index < covered.len() && covered[index] {
                            index += 1;
                        }
                        expected.push((start as f64, index as f64));
                    }
                    let actual: Vec<_> = (&ranges)
                        .into_iter()
                        .map(|range| (range.start(), range.end()))
                        .collect();
                    assert_eq!(actual, expected, "inputs: {a:?}, {b:?}, {c:?}");
                }
            }
        }
    }

    #[test]
    fn adding_ranges_merges_overlap_and_touching_boundaries_but_keeps_gaps() {
        for (input, expected) in [
            (vec![(2., 4.), (3., 6.)], vec![(2., 6.)]),
            (vec![(2., 4.), (0., 2.), (4., 6.)], vec![(0., 6.)]),
            (
                vec![(0., 2.), (4., 6.), (8., 10.), (1., 9.)],
                vec![(0., 10.)],
            ),
            (
                vec![(0., 2.), (4., 10.), (11., 12.), (1., 5.)],
                vec![(0., 10.), (11., 12.)],
            ),
            (vec![(0., 10.), (2., 3.)], vec![(0., 10.)]),
            (
                vec![(8., 10.), (0., 2.), (4., 6.)],
                vec![(0., 2.), (4., 6.), (8., 10.)],
            ),
            (vec![(0., 2.), (2.01, 4.)], vec![(0., 2.), (2.01, 4.)]),
        ] {
            let mut ranges = TimeRanges::new();
            for &(start, end) in &input {
                ranges.add(start, end);
            }
            let actual: Vec<_> = (&ranges)
                .into_iter()
                .map(|range| (range.start(), range.end()))
                .collect();
            assert_eq!(actual, expected, "input: {input:?}");
        }
    }
}
