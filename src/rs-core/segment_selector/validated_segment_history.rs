use crate::parser::SegmentIdentity;

/// Accepted media, retained independently of what the decoding back-end keeps buffered.
/// Selection and playlist inspection never create entries; only caller validation does.
#[derive(Default)]
pub(super) struct ValidatedSegmentHistory {
    entries: Vec<ValidatedSegment>,
}

struct ValidatedSegment {
    media_id: u32,
    identity: SegmentIdentity,
    /// Playlist end time used only for eviction, not for identity comparisons.
    end: f64,
}

impl ValidatedSegmentHistory {
    pub(super) fn contains(&self, media_id: u32, identity: SegmentIdentity) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.media_id == media_id && entry.identity == identity)
    }

    pub(super) fn insert(&mut self, media_id: u32, identity: SegmentIdentity, end: f64) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.media_id == media_id && entry.identity == identity)
        {
            entry.end = entry.end.max(end);
        } else {
            self.entries.push(ValidatedSegment {
                media_id,
                identity,
                end,
            });
        }
    }

    /// Remove entries ending at or before playback. Times must remain on the same playlist
    /// timeline while history is retained; a timeline restart must clear the history.
    pub(super) fn evict_before(&mut self, position: f64) {
        self.entries.retain(|entry| entry.end > position);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::ValidatedSegmentHistory;
    use crate::parser::SegmentIdentity;

    #[test]
    fn identity_ignores_end_time_and_distinguishes_media_and_discontinuities() {
        let mut history = ValidatedSegmentHistory::default();
        let identity = SegmentIdentity::new(10, 2);
        history.insert(1, identity, 12.);
        history.insert(1, identity, 12.001);

        assert_eq!(history.entries.len(), 1);
        assert!(history.contains(1, identity));
        assert!(!history.contains(2, identity));
        assert!(!history.contains(1, SegmentIdentity::new(10, 3)));
        assert!(!history.contains(1, SegmentIdentity::new(11, 2)));
        history.evict_before(12.);
        assert!(history.contains(1, identity));
        history.evict_before(12.001);
        assert!(!history.contains(1, identity));
    }

    #[test]
    fn eviction_keeps_future_entries_regardless_of_insertion_order() {
        let mut history = ValidatedSegmentHistory::default();
        let earlier = SegmentIdentity::new(1, 0);
        let later = SegmentIdentity::new(2, 0);
        history.insert(1, later, 20.);
        history.insert(1, earlier, 12.);

        history.evict_before(12.);
        assert!(!history.contains(1, earlier));
        assert!(history.contains(1, later));
        history.clear();
        assert!(!history.contains(1, later));
    }
}
