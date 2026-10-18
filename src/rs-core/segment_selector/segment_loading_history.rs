use crate::{
    parser::{MediaPlaylistPermanentId, SegmentIdentity},
    utils::time_ranges::TimeRanges,
};

/// When choosing which segment to load next, there's a need to understand which segments we already
/// loaded.
///
/// We could use the decoding back-end's buffer-related API, but they aren't reliable: they may not
/// show a segment that has been pushed, might do GC at some point etc.
///
/// So this history here separately keeps around the list of segments that have been considered as
/// loaded+pushed. It is intended as more of a history of operations than an actual representation
/// of what's in the buffer.
#[derive(Default)]
pub(super) struct SegmentLoadingHistory {
    entries: Vec<SegmentHistoryEntry>,
}

struct SegmentHistoryEntry {
    /// Identifier for the associated `media_playlist`.
    playlist_id: MediaPlaylistPermanentId,
    /// Identifies unambiguously the segment.
    identity: SegmentIdentity,
    /// Playlist positions used for buffer checks and eviction.
    start: f64,
    end: f64,
    /// Parts of this segment previously reported as buffered. Used to detect removed data. Cleared
    /// when the segment is supplied again.
    ///
    /// Only remember data inside this segment's time bounds. For example, if this segment covers 8
    /// to 12 seconds:
    ///
    /// ```text
    /// Time:              8       10      12      14
    /// This segment:      [---------------]
    /// Reported buffer:           [---------------]
    /// Part we remember:          [-------]
    /// ```
    ///
    /// Each observation adds its ranges to this memory. Missing data does not shrink it. Gaps
    /// remain unless a later observation reports data there:
    ///
    /// ```text
    /// Reported now       Remembered after this report
    /// [8--9]             [8--9]
    ///         [10--12]   [8--9]  [10--12]
    /// [8----------12]    [8----------12]
    /// [8------10]        [8----------12]
    /// ```
    ///
    /// In the last row, we remember data up to 12, but only see data up to 10. Comparing the two
    /// lets us detect removal, including several small removals over time. The check ignores data
    /// before the wanted position and tolerates up to 0.5 seconds missing in each gap or at a
    /// boundary. If all remembered data disappears, that tolerance does not apply.
    ///
    /// When the segment is supplied again, clear this memory and start over. Otherwise the same
    /// missing data could trigger reloads forever. These ranges tell us where data was seen, not
    /// which segment supplied it: another quality covering the same times also counts as buffered
    /// data.
    buffered: TimeRanges,
    /// If `true`, this entry has been detected as garbage collected since it
    /// was first pushed, it can be retried.
    needs_reload: bool,
}

impl SegmentLoadingHistory {
    /// Check whether previously reported data is now missing after the wanted position.
    /// Both the ranges and the position are in playlist time.
    pub(super) fn synchronize(&mut self, buffered: &[(f64, f64)], wanted_position: f64) {
        for entry in &mut self.entries {
            let mut current = TimeRanges::new();
            for &(start, end) in buffered {
                // Only keep the part of this range inside the segment.
                let start = start.max(entry.start);
                let end = end.min(entry.end);
                if end > start {
                    current.add(start, end);
                }
            }
            entry.needs_reload = has_lost_buffer(&entry.buffered, &current, wanted_position);
            for range in &current {
                entry.buffered.add(range.start(), range.end());
            }
        }
    }

    /// returns `true` if the given segment needs to be reloaded
    pub(super) fn needs_loading(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
    ) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.playlist_id == playlist_id && entry.identity == identity)
            .is_none_or(|entry| entry.needs_reload)
    }

    /// Earliest start of a segment that needs loading again.
    pub(super) fn first_position_to_reload(&self) -> Option<f64> {
        self.entries
            .iter()
            .filter(|entry| entry.needs_reload)
            .map(|entry| entry.start)
            .min_by(f64::total_cmp)
    }

    /// Returns `true` if current history contains the given segment.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - Identifier of the segment's media playlist.
    /// * `identity` - Identity of the segment within that playlist.
    pub(super) fn contains(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
    ) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.playlist_id == playlist_id && entry.identity == identity)
    }

    /// Adds the segment to the `SegmentLoadingHistory`.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - Identifier of the segment's media playlist.
    /// * `identity` - Identity of the segment within that playlist.
    /// * `start`, `end` - Playlist positions used for buffer checks and eviction.
    pub(super) fn insert(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
        start: f64,
        end: f64,
    ) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.playlist_id == playlist_id && entry.identity == identity)
        {
            entry.end = entry.end.max(end);
            // Judge a new attempt from its own observations, not the coverage
            // of an earlier attempt. Repeated missing or partial results then
            // do not look like fresh removal on every observation.
            entry.buffered.clear();
            entry.needs_reload = false;
        } else {
            self.entries.push(SegmentHistoryEntry {
                playlist_id,
                identity,
                start,
                end,
                buffered: TimeRanges::new(),
                needs_reload: false,
            });
        }
    }

    /// Removes entries ending before or at the given `position` (playlist time).
    ///
    /// # Arguments
    ///
    /// * `position` - All history entries ending before or at that playlist time will be evicted.
    pub(super) fn evict_before(&mut self, position: f64) {
        self.entries.retain(|entry| entry.end > position);
    }

    // Completely clears the content of the history.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Compare the data seen since the latest loading attempt with the data reported now.
/// The caller limits both sets of ranges to one segment, which may contain holes.
/// TimeRanges keeps each set sorted and keeps its gaps.
/// `position` is the first playlist position we still need; earlier data can be ignored.
fn has_lost_buffer(previous: &TimeRanges, current: &TimeRanges, position: f64) -> bool {
    if current.is_empty() {
        // All previously seen data disappeared. We need another load if any of
        // it was after `position`. Its start does not matter: a range can start
        // before that position and still contain data we need after it.
        // Complete disappearance triggers a reload even for a range under 0.5s.
        return previous.into_iter().any(|range| range.end() > position);
    }
    for range in previous {
        let end = range.end();
        // Start at the wanted position if the earlier part is already behind us.
        let mut covered_until = range.start().max(position);
        for current_range in current {
            let current_start = current_range.start();
            let current_end = current_range.end();
            if current_end <= covered_until {
                // This current range is entirely before the part being checked.
                continue;
            }
            if current_start >= end {
                // No later current range can cover this old range. The remaining
                // missing part is checked after the loop.
                break;
            }
            // A later start leaves a hole, including in the middle of a segment.
            // Tolerate at most 0.5s of missing data in each hole or at a boundary.
            // This allows small timing changes and small amounts of removal.
            if current_start - covered_until > 0.5 {
                return true;
            }
            covered_until = covered_until.max(current_end);
            if covered_until >= end {
                // All of this old range has been checked. Continue with the next.
                break;
            }
        }
        // The current ranges may have ended before this old range ended.
        if end - covered_until > 0.5 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::SegmentLoadingHistory;
    use crate::parser::{MediaPlaylistPermanentId, SegmentIdentity};

    #[test]
    fn losing_data_in_the_middle_requests_a_retry_until_the_hole_is_filled() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[(0., 12.)], 0.);
        history.synchronize(&[(0., 4.), (8., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        assert_eq!(history.first_position_to_reload(), Some(0.));
        history.synchronize(&[(0., 4.), (8., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        history.synchronize(&[(0., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn existing_holes_are_preserved_between_observations_and_after_a_retry() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 0., 12.);
        // Unordered and overlapping ranges must merge without filling the hole.
        let ranges = [(8., 12.), (0., 3.), (2., 4.)];
        for _ in 0..2 {
            history.synchronize(&ranges, 0.);
            assert_eq!(
                (&history.entries[0].buffered)
                    .into_iter()
                    .map(|range| (range.start(), range.end()))
                    .collect::<Vec<_>>(),
                vec![(0., 4.), (8., 12.)]
            );
            assert!(!history.needs_loading(playlist_id, identity));
        }
        // Newly seen data may fill part of the hole, but not all of it.
        history.synchronize(&[(0., 5.), (7., 12.)], 0.);
        assert_eq!(
            (&history.entries[0].buffered)
                .into_iter()
                .map(|range| (range.start(), range.end()))
                .collect::<Vec<_>>(),
            vec![(0., 5.), (7., 12.)]
        );
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(0., 3.), (7., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[(0., 3.), (7., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn middle_loss_behind_playback_is_ignored_and_small_holes_are_tolerated() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[(0., 12.)], 0.);
        history.synchronize(&[(0., 4.), (4.2, 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(0., 4.), (8., 12.)], 9.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(0., 4.), (8., 12.)], 6.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn missing_data_without_previous_coverage_does_not_trigger_reloads() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity));
            assert_eq!(history.first_position_to_reload(), None);
        }
    }

    #[test]
    fn removed_data_can_be_retried_but_repeated_missing_data_cannot_loop() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 12.)], 4.);
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        assert_eq!(history.first_position_to_reload(), Some(8.));
        // Merely proposing a retry must not consume it: an existing request may
        // still be in progress, or selection may choose a different segment.
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity));
            assert_eq!(history.first_position_to_reload(), None);
        }
        history.clear();
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn repeated_partial_loss_is_suppressed_but_further_loss_can_be_retried() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 12.)], 4.);
        history.synchronize(&[(8., 10.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 10.)], 4.);
        history.synchronize(&[(8., 10.)], 4.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(8., 9.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn reported_ranges_are_limited_to_the_segment_being_checked() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(0., 20.)], 4.);
        assert_eq!(
            (&history.entries[0].buffered)
                .into_iter()
                .map(|range| (range.start(), range.end()))
                .collect::<Vec<_>>(),
            vec![(8., 12.)]
        );
        // Removing data outside this segment does not require reloading it.
        history.synchronize(&[(8., 12.)], 4.);
        assert!(!history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn restored_data_can_be_reloaded_after_a_separate_removal() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[(8., 12.)], 4.);
            history.synchronize(&[], 4.);
            assert!(history.needs_loading(playlist_id, identity));
            history.insert(playlist_id, identity, 8., 12.);
        }
    }

    #[test]
    fn partial_loss_is_ignored_behind_playback_and_small_boundary_changes_are_tolerated() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 12.)], 4.);
        history.synchronize(&[(8., 11.8)], 4.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(9., 12.)], 10.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[(9., 12.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn identity_ignores_end_time_and_distinguishes_media_and_discontinuities() {
        let mut history = SegmentLoadingHistory::default();
        let identity = SegmentIdentity::new(10, 2);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), identity, 0., 12.);
        history.insert(
            MediaPlaylistPermanentId::new_for_test(1),
            identity,
            0.,
            12.001,
        );

        assert_eq!(history.entries.len(), 1);
        assert!(history.contains(MediaPlaylistPermanentId::new_for_test(1), identity));
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(2), identity));
        assert!(!history.contains(
            MediaPlaylistPermanentId::new_for_test(1),
            SegmentIdentity::new(10, 3)
        ));
        assert!(!history.contains(
            MediaPlaylistPermanentId::new_for_test(1),
            SegmentIdentity::new(11, 2)
        ));
        history.evict_before(12.);
        assert!(history.contains(MediaPlaylistPermanentId::new_for_test(1), identity));
        history.evict_before(12.001);
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(1), identity));
    }

    #[test]
    fn eviction_keeps_future_entries_regardless_of_insertion_order() {
        let mut history = SegmentLoadingHistory::default();
        let earlier = SegmentIdentity::new(1, 0);
        let later = SegmentIdentity::new(2, 0);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), later, 0., 20.);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), earlier, 0., 12.);

        history.evict_before(12.);
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(1), earlier));
        assert!(history.contains(MediaPlaylistPermanentId::new_for_test(1), later));
        history.clear();
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(1), later));
    }
}
