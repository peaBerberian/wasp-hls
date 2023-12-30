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
    pub(crate) fn add(&mut self, start: f64, end: f64) {
        let first_overlapping_idx = self.ranges.iter().position(|r| r.end > start);

        if let Some(first_overlapping_idx) = first_overlapping_idx {
            let mut has_updated_start = false;
            let mut has_updated_end = false;

            let initial_range = self.ranges.get_mut(first_overlapping_idx).unwrap();
            if end < initial_range.start {
                self.ranges
                    .insert(first_overlapping_idx, TimeRange { start, end });
                return;
            } else {
                if end > initial_range.end {
                    initial_range.end = end;
                    has_updated_end = true;
                }

                if start < initial_range.start {
                    initial_range.start = start;
                    has_updated_start = true;
                }
            }

            // TODO simplify this mess pls

            if has_updated_end {
                let mut to_remove_after = 0;
                let mut prev_end = self.ranges.get_mut(first_overlapping_idx).unwrap().end;
                for next_range in self.ranges.iter().skip(first_overlapping_idx) {
                    if next_range.start <= prev_end {
                        to_remove_after += 1;
                        prev_end = next_range.end;
                    } else {
                        break;
                    }
                }

                if to_remove_after > 0 {
                    let removal_range_start = first_overlapping_idx + 1;
                    let removal_range_end = first_overlapping_idx + 1 + to_remove_after;
                    self.ranges.drain(removal_range_start..removal_range_end);
                }
            }

            if has_updated_start {
                let mut to_remove_before = 0;
                let mut next_start = self.ranges.get_mut(first_overlapping_idx).unwrap().start;
                let iteration_start = self.ranges.len() - first_overlapping_idx;
                for prev_range in self.ranges.iter().rev().skip(iteration_start) {
                    if prev_range.end >= next_start {
                        to_remove_before += 1;
                        next_start = prev_range.start;
                    } else {
                        break;
                    }
                }

                if to_remove_before > 0 {
                    let removal_range_start = first_overlapping_idx - to_remove_before;
                    let removal_range_end = to_remove_before;
                    self.ranges.drain(removal_range_start..removal_range_end);
                }
            }
        } else {
            self.ranges.push(TimeRange { start, end });
        }
    }

    /// Remove time range at a particular index
    pub(crate) fn remove(&mut self, idx: usize) -> TimeRange {
        self.ranges.remove(idx)
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
