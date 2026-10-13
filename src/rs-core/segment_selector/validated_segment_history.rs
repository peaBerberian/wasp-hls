use crate::parser::{MediaPlaylistPermanentId, SegmentIdentity};

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

struct ValidatedSegment {
    /// Identifier for the associated `media_playlist`.
    playlist_id: MediaPlaylistPermanentId,
    /// Identifies unambiguously the segment.
    identity: SegmentIdentity,
    /// Playlist end time used only for eviction, not for identity comparisons.
    end: f64,
}

impl ValidatedSegmentHistory {
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
    /// * `end` - Segment end in playlist time. Used for eviction rather than identity.
    pub(super) fn insert(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
        end: f64,
    ) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.playlist_id == playlist_id && entry.identity == identity)
        {
            entry.end = entry.end.max(end);
        } else {
            self.entries.push(ValidatedSegment {
                playlist_id,
                identity,
                end,
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

#[cfg(test)]
mod tests {
    use super::ValidatedSegmentHistory;
    use crate::parser::{MediaPlaylistPermanentId, SegmentIdentity};

    #[test]
    fn identity_ignores_end_time_and_distinguishes_media_and_discontinuities() {
        let mut history = ValidatedSegmentHistory::default();
        let identity = SegmentIdentity::new(10, 2);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), identity, 12.);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), identity, 12.001);

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
        history.insert(MediaPlaylistPermanentId::new_for_test(1), later, 20.);
        history.insert(MediaPlaylistPermanentId::new_for_test(1), earlier, 12.);

        history.evict_before(12.);
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(1), earlier));
        assert!(history.contains(MediaPlaylistPermanentId::new_for_test(1), later));
        history.clear();
        assert!(!history.contains(MediaPlaylistPermanentId::new_for_test(1), later));
    }
}
