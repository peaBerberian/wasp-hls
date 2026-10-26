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
    /// Reload is allowed when the wanted position is before this boundary.
    /// None means no observed loss warrants a reload.
    reload_before: Option<f64>,
}

impl SegmentLoadingHistory {
    /// Return the earliest segment with lost data after the wanted position, in playlist time.
    /// Forget segments whose previously observed coverage has completely disappeared.
    pub(super) fn synchronize(
        &mut self,
        buffered: &[(f64, f64)],
        wanted_position: f64,
    ) -> Option<f64> {
        let mut first_position: Option<f64> = None;
        self.entries.retain_mut(|entry| {
            let mut current = TimeRanges::new();
            for &(start, end) in buffered {
                // Only keep the part of this range inside the segment.
                let start = start.max(entry.start);
                let end = end.min(entry.end);
                if end > start {
                    current.add(start, end);
                }
            }
            entry.reload_before = reload_before(&entry.buffered, &current);
            if entry
                .reload_before
                .is_some_and(|boundary| wanted_position < boundary)
            {
                first_position = Some(first_position.map_or(entry.start, |p| p.min(entry.start)));
            }
            if current.is_empty() {
                // Report the loss before discarding the entry, so the selector can rewind its cursor.
                if !entry.buffered.is_empty() {
                    return false;
                }
                // Keep never-observed appends to prevent repeated downloads, but discard them
                // once behind the wanted position so they cannot stay in history forever.
                return entry.end > wanted_position;
            }
            for range in &current {
                entry.buffered.add(range.start(), range.end());
            }
            true
        });
        first_position
    }

    /// Whether a loading attempt is allowed at the given wanted position.
    pub(super) fn needs_loading(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
        wanted_position: f64,
    ) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.playlist_id == playlist_id && entry.identity == identity)
            .is_none_or(|entry| {
                entry
                    .reload_before
                    .is_some_and(|boundary| wanted_position < boundary)
            })
    }

    /// Earliest start of a segment with lost data still needed at the given wanted position.
    pub(super) fn first_position_to_reload(&self, wanted_position: f64) -> Option<f64> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .reload_before
                    .is_some_and(|boundary| wanted_position < boundary)
            })
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
    /// * `start`, `end` - Playlist positions used for buffer checks.
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
            entry.reload_before = None;
        } else {
            self.entries.push(SegmentHistoryEntry {
                playlist_id,
                identity,
                start,
                end,
                buffered: TimeRanges::new(),
                reload_before: None,
            });
        }
    }
}

/// Find the latest wanted position before which observed loss warrants another load.
/// Both sets of ranges are limited to one segment. Initial holes do not count as loss.
fn reload_before(previous: &TimeRanges, current: &TimeRanges) -> Option<f64> {
    if current.is_empty() {
        // Complete disappearance is reported before the entry is discarded, even for a range
        // under 0.5s. Never-observed appends return None and cannot trigger a reload.
        return previous
            .into_iter()
            .map(|range| range.end())
            .max_by(f64::total_cmp);
    }
    let mut reload_before: Option<f64> = None;
    for range in previous {
        let end = range.end();
        let mut covered_until = range.start();
        for current_range in current {
            let current_start = current_range.start();
            let current_end = current_range.end();
            if current_end <= covered_until {
                continue;
            }
            if current_start >= end {
                break;
            }
            // A missing range only requires a reload while more than 0.5s of it remains ahead
            // of the wanted position. The latest such boundary covers all missing ranges.
            if current_start - covered_until > 0.5 {
                let boundary = current_start - 0.5;
                reload_before = Some(reload_before.map_or(boundary, |p| p.max(boundary)));
            }
            covered_until = covered_until.max(current_end);
            if covered_until >= end {
                break;
            }
        }
        if end - covered_until > 0.5 {
            let boundary = end - 0.5;
            reload_before = Some(reload_before.map_or(boundary, |p| p.max(boundary)));
        }
    }
    reload_before
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
        assert!(history.needs_loading(playlist_id, identity, 0.));
        assert_eq!(history.first_position_to_reload(0.), Some(0.));
        history.synchronize(&[(0., 4.), (8., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
        history.synchronize(&[(0., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity, 0.));
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
            assert!(!history.needs_loading(playlist_id, identity, 0.));
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
        assert!(!history.needs_loading(playlist_id, identity, 0.));
        history.synchronize(&[(0., 3.), (7., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[(0., 3.), (7., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity, 0.));
    }

    #[test]
    fn middle_loss_behind_playback_is_ignored_and_small_holes_are_tolerated() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[(0., 12.)], 0.);
        history.synchronize(&[(0., 4.), (4.2, 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity, 0.));
        history.synchronize(&[(0., 4.), (8., 12.)], 9.);
        assert!(!history.needs_loading(playlist_id, identity, 9.));
        history.synchronize(&[(0., 4.), (8., 12.)], 6.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
    }

    #[test]
    fn reload_queries_use_their_position_without_another_synchronization() {
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        for (current, boundary) in [
            (vec![(5., 12.)], 4.5),
            (vec![(0., 2.), (4., 6.), (10., 12.)], 9.5),
            (vec![(0., 8.)], 11.5),
        ] {
            let mut history = SegmentLoadingHistory::default();
            history.insert(playlist_id, identity, 0., 12.);
            history.synchronize(&[(0., 12.)], 0.);
            assert_eq!(history.synchronize(&current, 20.), None);
            assert!(!history.needs_loading(playlist_id, identity, boundary));
            assert_eq!(history.first_position_to_reload(boundary), None);
            assert!(history.needs_loading(playlist_id, identity, boundary - 0.1));
            assert_eq!(history.first_position_to_reload(boundary - 0.1), Some(0.));
            assert!(!history.needs_loading(playlist_id, identity, 20.));
        }
    }

    #[test]
    fn missing_data_without_previous_coverage_does_not_trigger_reloads() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity, 0.));
            assert_eq!(history.first_position_to_reload(0.), None);
        }
        assert_eq!(history.synchronize(&[], 11.9), None);
        assert!(history.contains(playlist_id, identity));
        assert_eq!(history.synchronize(&[], 12.), None);
        assert!(!history.contains(playlist_id, identity));
    }

    #[test]
    fn removed_data_can_be_retried_but_repeated_missing_data_cannot_loop() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 12.)], 4.);
        assert_eq!(history.synchronize(&[], 4.), Some(8.));
        assert!(history.needs_loading(playlist_id, identity, 0.));
        assert!(!history.contains(playlist_id, identity));
        // Merely proposing a retry must not consume it: an existing request may
        // still be in progress, or selection may choose a different segment.
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity, 0.));
            assert_eq!(history.first_position_to_reload(0.), None);
        }
        history = SegmentLoadingHistory::default();
        assert!(history.needs_loading(playlist_id, identity, 0.));

        // Complete disappearance bypasses the 0.5s tolerance, including across the wanted position.
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 8.25)], 4.);
        assert_eq!(history.synchronize(&[], 8.1), Some(8.));
    }

    #[test]
    fn repeated_partial_loss_is_suppressed_but_further_loss_can_be_retried() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 12.)], 4.);
        history.synchronize(&[(8., 10.)], 4.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[(8., 10.)], 4.);
        history.synchronize(&[(8., 10.)], 4.);
        assert!(!history.needs_loading(playlist_id, identity, 0.));
        history.synchronize(&[(8., 9.)], 4.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
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
        assert!(!history.needs_loading(playlist_id, identity, 0.));
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
            assert!(history.needs_loading(playlist_id, identity, 0.));
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
        assert!(!history.needs_loading(playlist_id, identity, 0.));
        history.synchronize(&[(9., 12.)], 10.);
        assert!(!history.needs_loading(playlist_id, identity, 10.));
        history.synchronize(&[(9., 12.)], 4.);
        assert!(history.needs_loading(playlist_id, identity, 0.));
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
    }

    #[test]
    fn entries_remain_behind_playback_until_all_observed_coverage_disappears() {
        let mut history = SegmentLoadingHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let earlier = SegmentIdentity::new(1, 0);
        let later = SegmentIdentity::new(2, 0);
        history.insert(playlist_id, earlier, 0., 4.);
        history.insert(playlist_id, later, 4., 8.);
        history.synchronize(&[(0., 8.)], 0.);

        // Advancing playback does not discard buffered segments, even when partial.
        assert_eq!(history.synchronize(&[(2., 8.)], 20.), None);
        assert!(history.contains(playlist_id, earlier));
        assert!(history.contains(playlist_id, later));
        assert_eq!(history.synchronize(&[(4., 8.)], 20.), None);
        assert!(!history.contains(playlist_id, earlier));
        assert!(history.contains(playlist_id, later));

        // A seek back makes loss after the new wanted position relevant again.
        assert_eq!(history.synchronize(&[(6., 8.)], 4.), Some(4.));
        assert!(history.contains(playlist_id, later));
        assert_eq!(history.synchronize(&[], 4.), Some(4.));
        assert!(!history.contains(playlist_id, later));
    }
}
