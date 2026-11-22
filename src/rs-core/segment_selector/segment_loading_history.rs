use super::MISSING_DATA_TOLERANCE;
use crate::{
    parser::{MediaPlaylistPermanentId, PlaylistSequenceInfo},
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
    /// Latest reported ranges for this media type, in playlist seconds.
    /// None means no observation has been received; Some(empty) means the buffer is empty.
    latest_buffered: Option<TimeRanges>,
}

struct SegmentHistoryEntry {
    /// Identifier for the associated `media_playlist`.
    playlist_id: MediaPlaylistPermanentId,
    /// Identifies unambiguously the segment.
    sequence_info: PlaylistSequenceInfo,
    /// Playlist positions used for buffer checks.
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
    /// A restart has occurred since the last supply. Also consider parts of the segment
    /// that have never been observed buffered when deciding whether to retry.
    reconsider_initial_holes: bool,
}

impl SegmentHistoryEntry {
    fn needs_loading(&self, latest_buffered: Option<&TimeRanges>, needed_from: f64) -> bool {
        let Some(latest_buffered) = latest_buffered else {
            // An explicit restart can retry an unobserved attempt before the first report.
            return self.reconsider_initial_holes && needed_from < self.end;
        };
        // Other segments may still be buffered when this segment has completely disappeared.
        let current = latest_buffered.into_iter().filter_map(|range| {
            let start = range.start().max(self.start);
            let end = range.end().min(self.end);
            (start < end).then_some((start, end))
        });
        let boundary = if self.reconsider_initial_holes {
            missing_data_boundary(std::iter::once((self.start, self.end)), current)
        } else {
            missing_data_boundary(
                self.buffered
                    .into_iter()
                    .map(|range| (range.start(), range.end())),
                current,
            )
        };
        boundary.is_some_and(|boundary| needed_from < boundary)
    }
}

impl SegmentLoadingHistory {
    /// Return the earliest position to reconsider after data loss, a restart, or forgetting
    /// entries with no reported data starting after `forget_after`. All positions are in
    /// playlist time.
    ///
    /// `forget_after` only limits memory. Forgetting an entry also loses its protection against
    /// reload loops.
    // TODO: Limit memory by count instead: keep at most N entries with no reported data,
    // forgetting the ones furthest ahead of `needed_from` first. This would remove the last
    // position limit given by the caller.
    pub(super) fn synchronize(
        &mut self,
        buffered: &[(f64, f64)],
        needed_from: f64,
        forget_after: f64,
    ) -> Option<f64> {
        let latest_buffered = self.latest_buffered.get_or_insert_default();
        latest_buffered.clear();
        for &(start, end) in buffered {
            if start < end {
                latest_buffered.add(start, end);
            }
        }
        let mut first_position: Option<f64> = None;
        self.entries.retain_mut(|entry| {
            if entry.needs_loading(Some(latest_buffered), needed_from) {
                first_position = Some(first_position.map_or(entry.start, |p| p.min(entry.start)));
            }
            let mut has_current_coverage = false;
            for range in &*latest_buffered {
                // Only keep the part of this range inside the segment.
                let start = range.start().max(entry.start);
                let end = range.end().min(entry.end);
                if end > start {
                    has_current_coverage = true;
                    entry.buffered.add(start, end);
                }
            }
            if !has_current_coverage {
                if entry.start > forget_after {
                    first_position =
                        Some(first_position.map_or(entry.start, |p| p.min(entry.start)));
                    return false;
                }
                return entry.end > needed_from;
            }
            true
        });
        first_position
    }

    /// Whether a loading attempt is allowed from the given position.
    pub(super) fn needs_loading(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        sequence_info: PlaylistSequenceInfo,
        needed_from: f64,
    ) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.playlist_id == playlist_id && entry.sequence_info == sequence_info)
            .is_none_or(|entry| entry.needs_loading(self.latest_buffered.as_ref(), needed_from))
    }

    /// Earliest start of a segment allowed to reload from the given position.
    pub(super) fn first_position_to_reload(&self, needed_from: f64) -> Option<f64> {
        self.entries
            .iter()
            .filter(|entry| entry.needs_loading(self.latest_buffered.as_ref(), needed_from))
            .map(|entry| entry.start)
            .min_by(f64::total_cmp)
    }

    /// Remove the matching entry and return its start position.
    pub(super) fn remove(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        sequence_info: PlaylistSequenceInfo,
    ) -> Option<f64> {
        let index = self.entries.iter().position(|entry| {
            entry.playlist_id == playlist_id && entry.sequence_info == sequence_info
        })?;
        Some(self.entries.remove(index).start)
    }

    /// Returns `true` if current history contains the given segment.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - Identifier of the segment's media playlist.
    /// * `sequence_info` - Identity of the segment within that playlist.
    pub(super) fn contains(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        sequence_info: PlaylistSequenceInfo,
    ) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.playlist_id == playlist_id && entry.sequence_info == sequence_info)
    }

    /// Adds the segment to the `SegmentLoadingHistory`.
    ///
    /// # Arguments
    ///
    /// * `playlist_id` - Identifier of the segment's media playlist.
    /// * `sequence_info` - Identity of the segment within that playlist.
    /// * `start`, `end` - Playlist positions used for buffer checks.
    pub(super) fn insert(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        sequence_info: PlaylistSequenceInfo,
        start: f64,
        end: f64,
    ) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.playlist_id == playlist_id && entry.sequence_info == sequence_info)
        {
            entry.end = entry.end.max(end);
            // Judge a new attempt from its own observations, not the coverage
            // of an earlier attempt. Repeated missing or partial results then
            // do not look like fresh removal on every observation.
            entry.buffered.clear();
            entry.reconsider_initial_holes = false;
        } else {
            self.entries.push(SegmentHistoryEntry {
                playlist_id,
                sequence_info,
                start,
                end,
                buffered: TimeRanges::new(),
                reconsider_initial_holes: false,
            });
        }
    }

    /// Reconsider initial missing data without forgetting what was supplied or observed.
    pub(super) fn reconsider(&mut self) {
        for entry in &mut self.entries {
            entry.reconsider_initial_holes = true;
        }
    }
}

/// Compare `expected` ranges with `available` ranges, in playlist seconds.
///
/// Look for parts of `expected` absent from `available`. Ignore missing parts no longer than
/// MISSING_DATA_TOLERANCE. For each longer missing part, take its end minus that tolerance.
/// Return the latest of those positions, or None if there is no such missing part.
///
/// For example, with a tolerance of 0.5s and `expected` containing 8-12s:
///
/// ```text
/// available               result       missing parts
/// 8-12                    None         nothing
/// 8.25-12                 None         only 0.25s
/// 10-12                   Some(9.5)    8-10
/// 8-10                    Some(11.5)   10-12
/// 8-9 and 10-11           Some(11.5)   9-10 and 11-12; keep the later boundary
/// empty                   Some(12)     everything; see the rule below
/// ```
///
/// Both inputs are sorted ranges. If `available` is empty, return the latest end in `expected`
/// without subtracting the tolerance. An empty `expected` always returns None.
fn missing_data_boundary(
    expected: impl Iterator<Item = (f64, f64)>,
    available: impl Iterator<Item = (f64, f64)> + Clone,
) -> Option<f64> {
    if available.clone().next().is_none() {
        // If all expected coverage is absent, bypass the 0.5s tolerance.
        // An empty expected range returns None.
        return expected.map(|(_, end)| end).max_by(f64::total_cmp);
    }
    let mut latest_boundary: Option<f64> = None;
    for (start, end) in expected {
        let mut covered_until = start;
        for (available_start, available_end) in available.clone() {
            if available_end <= covered_until {
                continue;
            }
            if available_start >= end {
                break;
            }
            if available_start - covered_until > MISSING_DATA_TOLERANCE {
                let boundary = available_start - MISSING_DATA_TOLERANCE;
                latest_boundary = Some(latest_boundary.map_or(boundary, |p| p.max(boundary)));
            }
            covered_until = covered_until.max(available_end);
            if covered_until >= end {
                break;
            }
        }
        if end - covered_until > MISSING_DATA_TOLERANCE {
            let boundary = end - MISSING_DATA_TOLERANCE;
            latest_boundary = Some(latest_boundary.map_or(boundary, |p| p.max(boundary)));
        }
    }
    latest_boundary
}

#[cfg(test)]
mod tests {
    use super::SegmentLoadingHistory;
    use crate::parser::{MediaPlaylistPermanentId, PlaylistSequenceInfo};

    #[test]
    fn losing_data_in_the_middle_requests_a_retry_until_the_hole_is_filled() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 0., 12.);
        history.synchronize(&[(0., 12.)], 0., 30.);
        history.synchronize(&[(0., 4.), (8., 12.)], 0., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        assert_eq!(history.first_position_to_reload(0.), Some(0.));
        history.synchronize(&[(0., 4.), (8., 12.)], 0., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        history.synchronize(&[(0., 12.)], 0., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn existing_holes_are_preserved_between_observations_and_after_a_retry() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 0., 12.);
        // Unordered and overlapping ranges must merge without filling the hole.
        let ranges = [(8., 12.), (0., 3.), (2., 4.)];
        for _ in 0..2 {
            history.synchronize(&ranges, 0., 30.);
            assert_eq!(
                (&history.entries[0].buffered)
                    .into_iter()
                    .map(|range| (range.start(), range.end()))
                    .collect::<Vec<_>>(),
                vec![(0., 4.), (8., 12.)]
            );
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        }
        // Newly seen data may fill part of the hole, but not all of it.
        history.synchronize(&[(0., 5.), (7., 12.)], 0., 30.);
        assert_eq!(
            (&history.entries[0].buffered)
                .into_iter()
                .map(|range| (range.start(), range.end()))
                .collect::<Vec<_>>(),
            vec![(0., 5.), (7., 12.)]
        );
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        history.synchronize(&[(0., 3.), (7., 12.)], 0., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        history.insert(playlist_id, sequence_info, 0., 12.);
        history.synchronize(&[(0., 3.), (7., 12.)], 0., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn middle_loss_behind_playback_is_ignored_and_small_holes_are_tolerated() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 0., 12.);
        history.synchronize(&[(0., 12.)], 0., 30.);
        history.synchronize(&[(0., 4.), (4.2, 12.)], 0., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        history.synchronize(&[(0., 4.), (8., 12.)], 9., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 9.));
        history.synchronize(&[(0., 4.), (8., 12.)], 6., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn reload_queries_use_their_position_without_another_synchronization() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        for (current, boundary) in [
            (vec![(5., 12.)], 4.5),
            (vec![(0., 2.), (4., 6.), (10., 12.)], 9.5),
            (vec![(0., 8.)], 11.5),
            // A later, smaller hole must not extend the reload boundary.
            (vec![(0., 2.), (4., 10.), (10.25, 12.)], 3.5),
        ] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist_id, sequence_info, 0., 12.);
            history.synchronize(&[(0., 12.)], 0., 30.);
            assert_eq!(history.synchronize(&current, 20., 30.), None);
            assert!(!history.needs_loading(playlist_id, sequence_info, boundary));
            assert_eq!(history.first_position_to_reload(boundary), None);
            assert!(history.needs_loading(playlist_id, sequence_info, boundary - 0.1));
            assert_eq!(history.first_position_to_reload(boundary - 0.1), Some(0.));
            assert!(!history.needs_loading(playlist_id, sequence_info, 20.));
        }
    }

    #[test]
    fn missing_data_without_previous_coverage_does_not_trigger_reloads() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4., 30.);
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
            assert_eq!(history.first_position_to_reload(0.), None);
        }
        assert_eq!(history.synchronize(&[], 11.9, 30.), None);
        assert!(history.contains(playlist_id, sequence_info));
        assert_eq!(history.synchronize(&[], 12., 30.), None);
        assert!(!history.contains(playlist_id, sequence_info));
    }

    #[test]
    fn removed_data_can_be_retried_but_repeated_missing_data_cannot_loop() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(8., 12.)], 4., 30.);
        assert_eq!(history.synchronize(&[], 4., 30.), Some(8.));
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        assert!(history.contains(playlist_id, sequence_info));
        // Merely proposing a retry must not consume it: an existing request may
        // still be in progress, or selection may choose a different segment.
        history.synchronize(&[], 4., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        history.insert(playlist_id, sequence_info, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4., 30.);
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
            assert_eq!(history.first_position_to_reload(0.), None);
        }
        history = SegmentLoadingHistory::default();
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));

        // Complete disappearance bypasses the 0.5s tolerance, including across the wanted position.
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(8., 8.25)], 4., 30.);
        assert_eq!(history.synchronize(&[], 8.1, 30.), Some(8.));
    }

    #[test]
    fn repeated_partial_loss_is_suppressed_but_further_loss_can_be_retried() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(8., 12.)], 4., 30.);
        history.synchronize(&[(8., 10.)], 4., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(8., 10.)], 4., 30.);
        history.synchronize(&[(8., 10.)], 4., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        history.synchronize(&[(8., 9.)], 4., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn reported_ranges_are_limited_to_the_segment_being_checked() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(0., 20.)], 4., 30.);
        assert_eq!(
            (&history.entries[0].buffered)
                .into_iter()
                .map(|range| (range.start(), range.end()))
                .collect::<Vec<_>>(),
            vec![(8., 12.)]
        );
        // Removing data outside this segment does not require reloading it.
        history.synchronize(&[(8., 12.)], 4., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn restored_data_can_be_reloaded_after_a_separate_removal() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[(8., 12.)], 4., 30.);
            history.synchronize(&[], 4., 30.);
            assert!(history.needs_loading(playlist_id, sequence_info, 0.));
            history.insert(playlist_id, sequence_info, 8., 12.);
        }
    }

    #[test]
    fn partial_loss_is_ignored_behind_playback_and_small_boundary_changes_are_tolerated() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        history.insert(playlist_id, sequence_info, 8., 12.);
        history.synchronize(&[(8., 12.)], 4., 30.);
        history.synchronize(&[(8., 11.8)], 4., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        history.synchronize(&[(9., 12.)], 10., 30.);
        assert!(!history.needs_loading(playlist_id, sequence_info, 10.));
        history.synchronize(&[(9., 12.)], 4., 30.);
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn identity_ignores_end_time_and_distinguishes_media_and_discontinuities() {
        let mut history = SegmentLoadingHistory::default();
        let sequence_info = PlaylistSequenceInfo::new(10, 2);
        history.insert(
            MediaPlaylistPermanentId::new_for_test(1),
            sequence_info,
            0.,
            12.,
        );
        history.insert(
            MediaPlaylistPermanentId::new_for_test(1),
            sequence_info,
            0.,
            12.001,
        );

        assert_eq!(history.entries.len(), 1);
        assert!(history.contains(MediaPlaylistPermanentId::new_for_test(1), sequence_info));
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(2), sequence_info));
        assert!(!history.contains(
            MediaPlaylistPermanentId::new_for_test(1),
            PlaylistSequenceInfo::new(10, 3)
        ));
        assert!(!history.contains(
            MediaPlaylistPermanentId::new_for_test(1),
            PlaylistSequenceInfo::new(11, 2)
        ));
    }

    #[test]
    fn entries_remain_while_buffered_or_needed_for_reload() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let earlier = PlaylistSequenceInfo::new(1, 0);
        let later = PlaylistSequenceInfo::new(2, 0);
        history.insert(playlist_id, earlier, 0., 4.);
        history.insert(playlist_id, later, 4., 8.);
        history.synchronize(&[(0., 8.)], 0., 30.);

        // Advancing playback does not discard buffered segments, even when partial.
        assert_eq!(history.synchronize(&[(2., 8.)], 20., 30.), None);
        assert!(history.contains(playlist_id, earlier));
        assert!(history.contains(playlist_id, later));
        assert_eq!(history.synchronize(&[(4., 8.)], 20., 30.), None);
        assert!(!history.contains(playlist_id, earlier));
        assert!(history.contains(playlist_id, later));

        // A seek back makes loss after the new wanted position relevant again.
        assert_eq!(history.synchronize(&[(6., 8.)], 4., 30.), Some(4.));
        assert!(history.contains(playlist_id, later));
        // Empty entries outside the window are forgotten, even ahead of playback.
        assert_eq!(history.synchronize(&[], 0., 3.), Some(4.));
        assert!(!history.contains(playlist_id, later));
    }

    #[test]
    fn empty_entry_retention_includes_segments_overlapping_the_needed_window() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        for (start, end, retained) in [
            (0., 4., false),
            (4., 8., false),
            (6., 10., true),
            (8., 12., true),
            (12., 16., true),
            (12.25, 16., false),
        ] {
            for previously_buffered in [false, true] {
                let mut history = SegmentLoadingHistory::default();
                history.insert(playlist_id, sequence_info, start, end);
                if previously_buffered {
                    history.synchronize(&[(start, end)], 0., 30.);
                }
                let reconsider = history.synchronize(&[], 8., 12.);
                assert_eq!(history.contains(playlist_id, sequence_info), retained);
                if start > 12. {
                    // Even an entry never observed as buffered must rewind a cursor past it.
                    assert_eq!(reconsider, Some(start));
                }
            }
        }
    }

    #[test]
    fn buffered_entries_outside_the_needed_window_survive_until_empty() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let earlier = PlaylistSequenceInfo::new(1, 0);
        let later = PlaylistSequenceInfo::new(2, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist_id, earlier, 0., 4.);
        history.insert(playlist_id, later, 16., 20.);
        history.synchronize(&[(0., 4.), (16., 20.)], 8., 12.);
        history.synchronize(&[(1., 4.), (16., 19.)], 8., 12.);
        assert!(history.contains(playlist_id, earlier));
        assert!(history.contains(playlist_id, later));
        assert_eq!(history.synchronize(&[], 8., 12.), Some(16.));
        assert!(history.entries.is_empty());
    }

    #[test]
    fn repeated_position_changes_do_not_accumulate_empty_entries() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let mut history = SegmentLoadingHistory::default();
        for iteration in 0..40 {
            let needed_from = if iteration % 2 == 0 { 0. } else { 1000. };
            for offset in 0..20 {
                let start = needed_from + f64::from(offset) * 4.;
                history.insert(
                    playlist_id,
                    PlaylistSequenceInfo::new(iteration * 20 + offset, 0),
                    start,
                    start + 4.,
                );
            }
            history.synchronize(&[], needed_from, needed_from + 8.);
            assert_eq!(history.entries.len(), 3);
        }
    }

    #[test]
    fn small_removals_accumulate_but_a_retry_starts_from_its_own_coverage() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist_id, sequence_info, 0., 4.);
        history.synchronize(&[(0., 4.)], 0., 8.);
        for start in [0.125, 0.25, 0.5] {
            assert_eq!(history.synchronize(&[(start, 4.)], 0., 8.), None);
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        }
        assert_eq!(history.synchronize(&[(0.75, 4.)], 0., 8.), Some(0.));
        assert!(history.needs_loading(playlist_id, sequence_info, 0.125));
        assert!(!history.needs_loading(playlist_id, sequence_info, 0.25));

        history.insert(playlist_id, sequence_info, 0., 4.);
        for _ in 0..3 {
            assert_eq!(history.synchronize(&[(0.75, 4.)], 0., 8.), None);
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
        }
        assert_eq!(history.synchronize(&[(1.5, 4.)], 0., 8.), Some(0.));
        assert!(history.needs_loading(playlist_id, sequence_info, 0.));
    }

    #[test]
    fn initially_missing_or_partial_data_can_appear_later_without_a_retry() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        for initial in [vec![], vec![(2., 4.)]] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist_id, sequence_info, 0., 4.);
            for _ in 0..3 {
                assert_eq!(history.synchronize(&initial, 0., 8.), None);
                assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
            }
            assert_eq!(history.synchronize(&[(0., 4.)], 0., 8.), None);
            assert!(!history.needs_loading(playlist_id, sequence_info, 0.));
            assert_eq!(history.synchronize(&[(1., 4.)], 0., 8.), Some(0.));
            assert!(history.needs_loading(playlist_id, sequence_info, 0.));
        }
    }

    #[test]
    fn retrying_one_quality_does_not_reset_another_qualitys_history() {
        let first_playlist = MediaPlaylistPermanentId::new_for_test(1);
        let second_playlist = MediaPlaylistPermanentId::new_for_test(2);
        let sequence_info = PlaylistSequenceInfo::new(1, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(first_playlist, sequence_info, 0., 4.);
        history.insert(second_playlist, sequence_info, 0., 4.);
        history.synchronize(&[(0., 4.)], 0., 8.);
        history.synchronize(&[(2., 4.)], 0., 8.);
        assert!(history.needs_loading(first_playlist, sequence_info, 0.));
        assert!(history.needs_loading(second_playlist, sequence_info, 0.));

        history.insert(second_playlist, sequence_info, 0., 4.);
        history.synchronize(&[(2., 4.)], 0., 8.);
        assert!(history.needs_loading(first_playlist, sequence_info, 0.));
        assert!(!history.needs_loading(second_playlist, sequence_info, 0.));
        history.synchronize(&[(0., 4.)], 0., 8.);
        assert!(!history.needs_loading(first_playlist, sequence_info, 0.));
        assert!(!history.needs_loading(second_playlist, sequence_info, 0.));
    }

    #[test]
    fn reconsidering_initial_holes_keeps_history_and_allows_one_new_supply() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        for ranges in [
            vec![],
            vec![(2., 4.)],
            vec![(0., 2.)],
            vec![(0., 1.), (3., 4.)],
        ] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist, sequence, 0., 4.);
            history.synchronize(&ranges, 0., 8.);
            assert!(!history.needs_loading(playlist, sequence, 0.));
            history.reconsider();
            assert!(history.contains(playlist, sequence));
            for _ in 0..3 {
                assert_eq!(history.synchronize(&ranges, 0., 8.), Some(0.));
                assert!(history.needs_loading(playlist, sequence, 0.));
                assert_eq!(history.first_position_to_reload(0.), Some(0.));
            }
            history.insert(playlist, sequence, 0., 4.);
            for _ in 0..3 {
                assert_eq!(history.synchronize(&ranges, 0., 8.), None);
                assert!(!history.needs_loading(playlist, sequence, 0.));
            }
            history.reconsider();
            assert!(history.needs_loading(playlist, sequence, 0.));
        }
    }

    #[test]
    fn reconsideration_tolerates_small_initial_holes() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        for ranges in [
            vec![(0., 4.)],
            vec![(0.25, 4.)],
            vec![(0., 3.75)],
            vec![(0., 1.), (1.25, 4.)],
        ] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist, sequence, 0., 4.);
            history.synchronize(&ranges, 0., 8.);
            history.reconsider();
            assert!(!history.needs_loading(playlist, sequence, 0.));
        }
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist, sequence, 0., 4.);
        history.synchronize(&[(2., 4.)], 0., 8.);
        history.synchronize(&[(0., 4.)], 0., 8.);
        history.reconsider();
        assert!(!history.needs_loading(playlist, sequence, 0.));
    }

    #[test]
    fn reconsidered_prefix_does_not_require_loading_when_already_behind() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist, sequence, 0., 4.);
        history.synchronize(&[(2., 4.)], 3., 8.);
        history.reconsider();
        assert!(!history.needs_loading(playlist, sequence, 3.));
        assert!(!history.needs_loading(playlist, sequence, 1.5));
        assert_eq!(history.first_position_to_reload(3.), None);
        assert!(history.needs_loading(playlist, sequence, 1.));
    }

    #[test]
    fn supplying_one_reconsidered_segment_keeps_other_permissions_and_loss_detection() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let first = PlaylistSequenceInfo::new(1, 0);
        let second = PlaylistSequenceInfo::new(2, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist, first, 0., 4.);
        history.insert(playlist, second, 4., 8.);
        history.synchronize(&[(2., 6.)], 0., 12.);
        history.reconsider();
        history.insert(playlist, first, 0., 4.);
        history.synchronize(&[(2., 6.)], 0., 12.);
        assert!(!history.needs_loading(playlist, first, 0.));
        assert!(history.needs_loading(playlist, second, 0.));
        history.synchronize(&[(0., 6.)], 0., 12.);
        history.synchronize(&[(2., 6.)], 0., 12.);
        assert!(history.needs_loading(playlist, first, 0.));
    }

    #[test]
    fn restored_coverage_withdraws_a_reconsidered_retry_without_another_supply() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist, sequence, 8., 12.);
        history.synchronize(&[(10., 12.)], 0., 20.);
        history.reconsider();
        assert!(history.needs_loading(playlist, sequence, 8.));

        // Any quality can fill the hole before the retry is supplied.
        assert_eq!(history.synchronize(&[(8., 12.)], 0., 20.), None);
        assert!(!history.needs_loading(playlist, sequence, 8.));
        assert_eq!(history.first_position_to_reload(8.), None);
        assert!(history.contains(playlist, sequence));

        // A separate removal can still justify a retry.
        assert_eq!(history.synchronize(&[(10., 12.)], 0., 20.), Some(8.));
        assert!(history.needs_loading(playlist, sequence, 8.));
    }

    #[test]
    fn restart_compares_the_total_hole_including_initial_missing_data_and_later_loss() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        for (initial, current) in [
            (vec![(0.4, 4.)], vec![(0.8, 4.)]),
            (vec![(0., 3.6)], vec![(0., 3.2)]),
            (vec![(0., 1.), (1.4, 4.)], vec![(0., 1.), (1.8, 4.)]),
        ] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist, sequence, 0., 4.);
            history.synchronize(&initial, 0., 8.);
            history.synchronize(&current, 0., 8.);
            // The later loss alone is within tolerance.
            assert!(!history.needs_loading(playlist, sequence, 0.));
            history.reconsider();
            assert!(history.needs_loading(playlist, sequence, 0.));
            assert_eq!(history.first_position_to_reload(0.), Some(0.));

            history.insert(playlist, sequence, 0., 4.);
            for _ in 0..3 {
                assert_eq!(history.synchronize(&current, 0., 8.), None);
                assert!(!history.needs_loading(playlist, sequence, 0.));
            }
        }
    }

    #[test]
    fn restart_can_retry_an_absent_segment_until_its_playlist_end() {
        let playlist = MediaPlaylistPermanentId::new_for_test(1);
        let sequence = PlaylistSequenceInfo::new(1, 0);
        let mut history = SegmentLoadingHistory::default();
        history.insert(playlist, sequence, 8., 12.);
        history.synchronize(&[(8., 11.8)], 0., 20.);
        // A different segment remains buffered. This one has completely disappeared.
        history.synchronize(&[(16., 20.)], 0., 20.);
        assert!(!history.needs_loading(playlist, sequence, 11.9));
        history.reconsider();
        assert!(history.needs_loading(playlist, sequence, 11.9));
        assert_eq!(history.first_position_to_reload(11.9), Some(8.));
        assert!(!history.needs_loading(playlist, sequence, 12.));

        history.insert(playlist, sequence, 8., 12.);
        assert!(!history.needs_loading(playlist, sequence, 11.9));
    }
}
