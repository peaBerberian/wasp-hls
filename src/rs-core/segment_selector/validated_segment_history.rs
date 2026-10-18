use crate::{
    media_element::BufferedChunk,
    parser::{MediaPlaylistPermanentId, SegmentIdentity},
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
pub(super) struct ValidatedSegmentHistory {
    entries: Vec<ValidatedSegment>,
}

// XXX TODO: To rename?
struct ValidatedSegment {
    /// Identifier for the associated `media_playlist`.
    playlist_id: MediaPlaylistPermanentId,
    /// Identifies unambiguously the segment.
    identity: SegmentIdentity,
    /// Playlist positions used for buffer checks and eviction.
    start: f64,
    end: f64,
    /// Ranges seen since the last attempt, keeping any gaps between them.
    buffered: Vec<(f64, f64)>,
    /// If `true`, this entry has been detected as garbage collected since it
    /// was first pushed, it can be retried.
    needs_reload: bool,
}

impl ValidatedSegmentHistory {
    /// XXX TODO: Rewrite comment. Use simple english
    /// Only data lost after an observation can permit another attempt. The
    /// initial coverage of each attempt is its baseline, even if partial.
    /// XXX TODO: What is "position"?
    pub(super) fn synchronize(&mut self, inventory: &[BufferedChunk], position: f64) {
        for entry in &mut self.entries {
            // A queued replacement may already have replaced the old inventory
            // entry. Wait for its append result before treating that as removal.
            if inventory.iter().any(|chunk| {
                chunk.is_validated()
                    && chunk.playlist_start() < entry.end
                    && chunk.playlist_end() > entry.start
            }) {
                continue;
            }
            let buffered: Vec<_> = inventory
                .iter()
                .filter(|chunk| {
                    !chunk.is_validated()
                        && chunk.playlist_start() < entry.end
                        && chunk.playlist_end() > entry.start
                })
                .filter_map(|chunk| {
                    // XXX TODO: Why are we doing the max with last buffered here?
                    let start = chunk.last_buffered_start().max(entry.start);
                    let end = chunk.last_buffered_end().min(entry.end);
                    (end > start).then_some((start, end))
                })
                .collect();
            let buffered = merge_overlapping_ranges(buffered);
            entry.needs_reload = has_lost_buffer(&entry.buffered, &buffered, position);
            entry.buffered.extend(buffered);
            entry.buffered = merge_overlapping_ranges(std::mem::take(&mut entry.buffered));
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

    // XXX TODO: rename? And comment
    pub(super) fn next_retry_position(&self) -> Option<f64> {
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

    /// Adds the segment to the `ValidatedSegmentHistory`.
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
            // XXX TODO: why clearing all buffered? Isn't it risky?
            entry.buffered.clear();
            entry.needs_reload = false;
        } else {
            self.entries.push(ValidatedSegment {
                playlist_id,
                identity,
                start,
                end,
                buffered: vec![],
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

/// Merge overlapping or touching ranges, but keep gaps between them.
/// XXX TODO: shouldn't it me in range utils? As TimeRange?
fn merge_overlapping_ranges(mut ranges: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    ranges.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some(previous) = merged.last_mut() {
            if start <= previous.1 {
                previous.1 = previous.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

/// Look for newly missing data ahead of playback, including inside a range.
fn has_lost_buffer(previous: &[(f64, f64)], current: &[(f64, f64)], position: f64) -> bool {
    if current.is_empty() {
        // XXX TODO: Why only end here? The only call site check for a single entry
        return previous.iter().any(|&(_, end)| end > position);
    }
    for &(start, end) in previous {
        let mut covered_until = start.max(position);
        for &(current_start, current_end) in current {
            if current_end <= covered_until {
                continue;
            }
            if current_start >= end {
                break;
            }
            // Ignore small changes, as for partial removal in the inventory.
            if current_start - covered_until > 0.5 {
                return true;
            }
            covered_until = covered_until.max(current_end);
            if covered_until >= end {
                break;
            }
        }
        if end - covered_until > 0.5 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::ValidatedSegmentHistory;
    use crate::{
        media_element::{BufferedChunk, SegmentQualityContext},
        parser::{MediaPlaylistPermanentId, SegmentIdentity},
    };

    #[test]
    fn losing_data_in_the_middle_requests_a_retry_until_the_hole_is_filled() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk = |start, end| {
            BufferedChunk::new_for_test(start, end, SegmentQualityContext::new(1., playlist_id))
        };
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[chunk(0., 12.)], 0.);
        history.synchronize(&[chunk(0., 4.), chunk(8., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        assert_eq!(history.next_retry_position(), Some(0.));
        history.synchronize(&[chunk(0., 4.), chunk(8., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(0., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn existing_holes_are_preserved_between_observations_and_after_a_retry() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk = |start, end| {
            BufferedChunk::new_for_test(start, end, SegmentQualityContext::new(1., playlist_id))
        };
        history.insert(playlist_id, identity, 0., 12.);
        // Unordered and overlapping entries must merge without filling the hole.
        let inventory = [chunk(8., 12.), chunk(0., 3.), chunk(2., 4.)];
        for _ in 0..2 {
            history.synchronize(&inventory, 0.);
            assert_eq!(history.entries[0].buffered, vec![(0., 4.), (8., 12.)]);
            assert!(!history.needs_loading(playlist_id, identity));
        }
        // Newly seen data may fill part of the hole, but not all of it.
        history.synchronize(&[chunk(0., 5.), chunk(7., 12.)], 0.);
        assert_eq!(history.entries[0].buffered, vec![(0., 5.), (7., 12.)]);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(0., 3.), chunk(7., 12.)], 0.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[chunk(0., 3.), chunk(7., 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn middle_loss_behind_playback_is_ignored_and_small_holes_are_tolerated() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk = |start, end| {
            BufferedChunk::new_for_test(start, end, SegmentQualityContext::new(1., playlist_id))
        };
        history.insert(playlist_id, identity, 0., 12.);
        history.synchronize(&[chunk(0., 12.)], 0.);
        history.synchronize(&[chunk(0., 4.), chunk(4.2, 12.)], 0.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(0., 4.), chunk(8., 12.)], 9.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(0., 4.), chunk(8., 12.)], 6.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn missing_data_without_previous_coverage_does_not_trigger_reloads() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity));
            assert_eq!(history.next_retry_position(), None);
        }
    }

    #[test]
    fn removed_data_can_be_retried_but_repeated_missing_data_cannot_loop() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk =
            || BufferedChunk::new_for_test(8., 12., SegmentQualityContext::new(1., playlist_id));
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[chunk()], 4.);
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        assert_eq!(history.next_retry_position(), Some(8.));
        // Merely proposing a retry must not consume it: an existing request may
        // still be in progress, or selection may choose a different segment.
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[], 4.);
            assert!(!history.needs_loading(playlist_id, identity));
            assert_eq!(history.next_retry_position(), None);
        }
        history.clear();
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn repeated_partial_loss_is_suppressed_but_further_loss_can_be_retried() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk =
            |end| BufferedChunk::new_for_test(8., end, SegmentQualityContext::new(1., playlist_id));
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[chunk(12.)], 4.);
        history.synchronize(&[chunk(10.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[chunk(10.)], 4.);
        history.synchronize(&[chunk(10.)], 4.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(9.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn a_quality_replacement_is_not_mistaken_for_removed_data() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        history.insert(playlist_id, identity, 8., 12.);
        for quality in [1, 2] {
            history.synchronize(
                &[BufferedChunk::new_for_test(
                    8.,
                    12.,
                    SegmentQualityContext::new(
                        quality as f64,
                        MediaPlaylistPermanentId::new_for_test(quality),
                    ),
                )],
                4.,
            );
            assert!(!history.needs_loading(playlist_id, identity));
        }
        history.synchronize(&[], 4.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn restored_data_can_be_reloaded_after_a_separate_removal() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk =
            || BufferedChunk::new_for_test(8., 12., SegmentQualityContext::new(1., playlist_id));
        history.insert(playlist_id, identity, 8., 12.);
        for _ in 0..3 {
            history.synchronize(&[chunk()], 4.);
            history.synchronize(&[], 4.);
            assert!(history.needs_loading(playlist_id, identity));
            history.insert(playlist_id, identity, 8., 12.);
        }
    }

    #[test]
    fn partial_loss_is_ignored_behind_playback_and_small_boundary_changes_are_tolerated() {
        let mut history = ValidatedSegmentHistory::default();
        let playlist_id = MediaPlaylistPermanentId::new_for_test(1);
        let identity = SegmentIdentity::new(1, 0);
        let chunk = |start, end| {
            BufferedChunk::new_for_test(start, end, SegmentQualityContext::new(1., playlist_id))
        };
        history.insert(playlist_id, identity, 8., 12.);
        history.synchronize(&[chunk(8., 12.)], 4.);
        history.synchronize(&[chunk(8., 11.8)], 4.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(9., 12.)], 10.);
        assert!(!history.needs_loading(playlist_id, identity));
        history.synchronize(&[chunk(9., 12.)], 4.);
        assert!(history.needs_loading(playlist_id, identity));
    }

    #[test]
    fn identity_ignores_end_time_and_distinguishes_media_and_discontinuities() {
        let mut history = ValidatedSegmentHistory::default();
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
        let mut history = ValidatedSegmentHistory::default();
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
