use crate::{
    bindings::MediaType,
    media_element::{BufferedChunk, SegmentQualityContext},
    parser::{
        InitSegmentInfo, MediaPlaylistPermanentId, MediaSegmentInfo, SegmentIdentity, SegmentList,
    },
    utils::logger::*,
};

mod segment_loading_history;

use segment_loading_history::SegmentLoadingHistory;

/// Minimum distance from playback, in seconds, before replacing buffered media.
const MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS: f64 = 3.;

/// Indicate the most prioritary segment to load according to the given situation.
///
/// Each media type keeps its own loading history and initialization state.
pub(crate) struct NextSegmentSelectors {
    /// Segment-selection logic for the audio media type
    audio: NextSegmentSelector,
    /// Segment-selection logic for the video media type
    video: NextSegmentSelector,
}

impl NextSegmentSelectors {
    /// Create a new `NextSegmentSelectors`, which will start loading segments from `base_pos` (as a
    /// playlist time in seconds) and will stop loading segments if enough to fill the buffer until
    /// `base_pos + buffer_goal` are already loaded.
    ///
    /// As the current playback position advances, it is then recommended to update the base
    /// position (here indicated by `base_pos`) regularly by calling the `advance_position`
    /// method on the returned instance.
    ///
    /// After a seek or flush, `restart_from_position` also clears loading history.
    ///
    /// Likewise, the buffer goal (here indicated by `buffer_goal`), can be updated by calling the
    /// `update_buffer_goal` method.
    pub(crate) fn new(base_pos: f64, buffer_goal: f64) -> Self {
        Self {
            audio: NextSegmentSelector::new(base_pos, buffer_goal),
            video: NextSegmentSelector::new(base_pos, buffer_goal),
        }
    }

    /// Re-sets to its initial state all underlying `NextSegmentSelector` instances, starting at the given "base position".
    ///
    /// Resetting the `NextSegmentSelector`s this way will lead them to lose much of their internal
    /// state they keep to know which segment should be loaded next. As such, this method should
    /// most likely only be called when stopping the current content or when switching to another
    /// content.
    pub(crate) fn reset_selectors(&mut self, pos: f64) {
        let pos = f64::max(0., pos);
        self.audio.reset(pos);
        self.video.reset(pos);
    }

    /// Advance the wanted position and discard loading history behind it.
    pub(crate) fn advance_position(&mut self, pos: f64) {
        self.audio.advance_position(pos);
        self.video.advance_position(pos);
    }

    /// Change the wanted position and clear loading history after a seek or flush.
    pub(crate) fn restart_from_position(&mut self, pos: f64) {
        let pos = f64::max(0., pos);
        self.audio.restart_from_position(pos);
        self.video.restart_from_position(pos);
    }

    /// Update the "buffer_goal" which is the amount of media data, in seconds of media, ahead of
    /// the current "base position" that should be loaded.
    ///
    /// Once that amount is reached, no further segment will be returned by the
    /// `NextSegmentSelectors` until either the buffer goal is raised again, or most probably until
    /// the "base position" (which generally represents the current position), is updated.
    pub(crate) fn update_buffer_goal(&mut self, buffer_goal: f64) {
        self.audio.buffer_goal = buffer_goal;
        self.video.buffer_goal = buffer_goal;
    }

    /// Get the unique `NextSegmentSelector` for the type of media communicated as a mutable
    /// reference.
    ///
    /// Obtaining the type-specific `NextSegmentSelector` then allows to obtain which segment have
    /// to be loaded next for that particular type.
    ///
    /// Because most of the `NextSegmentSelector`'s method might update its internal state, you
    /// generally want to obtain a mutable reference of it.
    pub(crate) fn get_mut(&mut self, media_type: MediaType) -> &mut NextSegmentSelector {
        match media_type {
            MediaType::Audio => &mut self.audio,
            MediaType::Video => &mut self.video,
        }
    }
}

pub(crate) struct NextSegmentSelector {
    /// History of accepted media.
    ///
    /// This allows to prevent segment-loading-loop for complex cases where the decoding-side is
    /// not reliable.
    ///
    /// Also decides when observed buffer loss warrants another loading attempt.
    validated_segments: SegmentLoadingHistory,

    /// Approximation of the current playback position, which will be used as a base position where
    /// segments should start to be loaded.
    base_pos: f64,

    /// Amount of buffer, ahead of the current position we want to build in seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    buffer_goal: f64,

    /// Status in the `NextSegmentSelector` regarding the initialization segment.
    init_status: InitializationSegmentSelectorStatus,

    /// Playlist ID last considered with a usable playlist. A change invalidates initialization
    /// segment state.
    last_playlist_id: Option<MediaPlaylistPermanentId>,
}

impl NextSegmentSelector {
    /// Select media from the wanted position through the buffer goal.
    fn new(base_pos: f64, buffer_goal: f64) -> Self {
        let real_base_pos = f64::max(0., base_pos);
        Self {
            base_pos: real_base_pos,
            buffer_goal,
            last_playlist_id: None,
            validated_segments: SegmentLoadingHistory::default(),
            init_status: InitializationSegmentSelectorStatus::Unchecked,
        }
    }

    /// Reset the `NextSegmentSelector` state, as if no media segment nor init segment was
    /// returned by it, and start back from the given "base position".
    pub(crate) fn reset(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.init_status = InitializationSegmentSelectorStatus::Unchecked;
        self.last_playlist_id = None;
        self.validated_segments = SegmentLoadingHistory::default();
    }

    /// See `NextSegmentSelectors`'s `advance_position` method.
    pub(crate) fn advance_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);

        self.validated_segments.evict_before(self.base_pos);
    }

    /// See `NextSegmentSelectors`'s `restart_from_position` method.
    pub(crate) fn restart_from_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.validated_segments.clear();
    }

    /// Check reported buffer ranges for removed data. Times are in playlist seconds.
    pub(crate) fn check_for_removed_data(&mut self, buffered: &[(f64, f64)]) {
        self.validated_segments.synchronize(buffered, self.base_pos);
    }

    /// Calling this method allows to indicate that the initialization segment identified by `id`
    /// was requested and as such, doesn't need to be returned anymore by this
    /// `NextSegmentSelector`.
    pub(crate) fn validate_init(&mut self, id: f64) {
        if let InitializationSegmentSelectorStatus::Unvalidated(expected_id) = self.init_status {
            if expected_id != id {
                log_warn!(
                    "Validation of an initialization segment with unexpected id. Expected {expected_id}, got {id}."
                );
            }
        }
        self.init_status = InitializationSegmentSelectorStatus::Validated(id);
    }

    /// Record completed media appends so later scans only retry after observed removal.
    pub(crate) fn validate_media(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        identity: SegmentIdentity,
        start: f64,
        end: f64,
    ) {
        if end > self.base_pos {
            self.validated_segments
                .insert(playlist_id, identity, start, end);
        }
    }

    /// Returns the current most needed segment(s) according to the current situation and to the
    /// last "validated" init and media segment.
    ///
    /// `addition_candidate` is used to extend the buffer; `replacement_candidate` is used to
    /// replace lower-quality media already in the buffer. Both can be equal to the same segment
    /// list if it doesn't matter.
    ///
    /// The returned playlist to follow may differ from the one of the segment returned now: e.g. an
    /// addition can be requested while the replacement candidate's list is updated.
    ///
    /// Once returned, the segment objects returned by this method have to be "validated" if they
    /// do have been requested, to avoid just getting the same segment on the next
    /// `most_needed_segment` call.
    /// To "validate" a segment, you can call `validate_init` if we're talking about an
    /// initialization segment, or `validate_media` if we're talking about a media segment.
    pub(crate) fn most_needed_segment<'a>(
        &mut self,
        addition_candidate: SegmentSelectionCandidate<'a>,
        replacement_candidate: SegmentSelectionCandidate<'a>,
        inventory: &[BufferedChunk],
        has_pending_append: impl Fn(f64, f64) -> bool,
    ) -> MostNeededSegmentInfo<'a> {
        let (selected_candidate, replacement_segment, playlist_to_refresh) = match self
            .find_replacement_segment(&replacement_candidate, inventory, &has_pending_append)
        {
            SegmentProposition::Segment(segment) => (
                replacement_candidate,
                Some(segment),
                replacement_candidate.context.playlist_id(),
            ),
            SegmentProposition::NeedsPlaylistUpdate => (
                addition_candidate,
                None,
                replacement_candidate.context.playlist_id(),
            ),
            SegmentProposition::None => (
                addition_candidate,
                None,
                addition_candidate.context.playlist_id(),
            ),
        };

        let Some(segment_list) = selected_candidate.playlist else {
            // The playlist doesn't seem to be fetched yet
            return MostNeededSegmentInfo {
                needed_segments: None,
                loading_complete: false,
                playlist_to_refresh,
            };
        };

        let context = &selected_candidate.context;
        let selected_playlist_id = context.playlist_id();
        if self
            .last_playlist_id
            .is_some_and(|id| id != selected_playlist_id)
        {
            self.init_status = InitializationSegmentSelectorStatus::Unchecked;
        }
        self.last_playlist_id = Some(selected_playlist_id);

        let most_needed_segment = if let Some(segment) = replacement_segment {
            segment
        } else {
            let maximum_position = self.base_pos + self.buffer_goal;
            let Some(segment) = segment_list
                .media()
                .iter()
                .skip_while(|segment| segment.end() <= self.base_pos)
                .take_while(|segment| segment.start() <= maximum_position)
                .find(|segment| {
                    self.needs_media_segment(segment, context, inventory, &has_pending_append)
                })
            else {
                return MostNeededSegmentInfo {
                    needed_segments: None,
                    loading_complete: selected_candidate.is_ended
                        && !segment_list.media().is_empty()
                        && !segment_list.media().iter().any(|segment| {
                            self.needs_media_segment(
                                segment,
                                context,
                                inventory,
                                &has_pending_append,
                            )
                        }),
                    playlist_to_refresh,
                };
            };
            segment
        };
        let associated_init = segment_list.init_for(most_needed_segment);
        let init_segment = if let Some(i) = associated_init {
            match self.init_status {
                InitializationSegmentSelectorStatus::Validated(id) if id == i.id() => None,
                _ => {
                    self.init_status = InitializationSegmentSelectorStatus::Unvalidated(i.id());
                    Some(i)
                }
            }
        } else {
            self.init_status = InitializationSegmentSelectorStatus::NoneExists;
            None
        };
        MostNeededSegmentInfo {
            loading_complete: false,
            needed_segments: Some(NeededSegmentInfo {
                variant_id: selected_candidate.variant_id,
                media_segment: Some((most_needed_segment, associated_init.map(|init| init.id()))),
                init_segment,
            }),
            playlist_to_refresh,
        }
    }

    /// Find the first replacement segment that can replace buffered media and has not been validated.
    /// Negative inspection results are not remembered: live lists and inventory may change.
    /// Cached segments may be proposed while their playlist is refreshed.
    fn find_replacement_segment<'a>(
        &self,
        replacement_candidate: &SegmentSelectionCandidate<'a>,
        inventory: &[BufferedChunk],
        has_pending_append: &impl Fn(f64, f64) -> bool,
    ) -> SegmentProposition<'a> {
        let Some(first_index) = inventory
            .iter()
            .position(|chunk| chunk.playlist_end() > self.base_pos)
        else {
            return SegmentProposition::None;
        };
        let mut previous_end = self.base_pos;
        let maximum_position = self.base_pos + self.buffer_goal;
        for chunk in &inventory[first_index..] {
            if chunk.playlist_start() > previous_end + 0.001
                || chunk.appears_garbage_collected(previous_end)
            {
                break;
            }
            if previous_end > maximum_position {
                break;
            }

            let chunk_end = chunk.playlist_end();
            // TODO: Check the replacement segment's own start and duration as well. With
            // different segment boundaries, replacing this distant chunk may select a segment
            // which starts near the playhead and defeats the required lead below.
            let required_lead =
                MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS.max(chunk_end - chunk.playlist_start());
            let can_replace = chunk.is_worse_than(&replacement_candidate.context)
                && previous_end - self.base_pos > required_lead;
            if can_replace {
                let Some(list) = replacement_candidate.playlist else {
                    return SegmentProposition::NeedsPlaylistUpdate;
                };
                if let Some(segment) = list.media().iter().find(|seg| {
                    seg.duration() > 0.
                        && seg.end() > previous_end
                        && !has_pending_append(seg.start(), seg.end())
                        && !self
                            .was_segment_validated(replacement_candidate.context.playlist_id(), seg)
                        && seg.start() < chunk_end
                        && seg.start() <= maximum_position
                }) {
                    return SegmentProposition::Segment(segment);
                }
                // TODO: If this replaceable range lies beyond the cached live playlist's window,
                // ask to refresh the replacement playlist while continuing additions. Otherwise,
                // unchanged candidates can leave newer replacement opportunities undiscovered.
            }

            previous_end = chunk_end;
        }
        SegmentProposition::None
    }

    fn was_segment_validated(
        &self,
        playlist_id: MediaPlaylistPermanentId,
        segment: &MediaSegmentInfo,
    ) -> bool {
        self.validated_segments
            .contains(playlist_id, segment.identity())
    }

    fn needs_media_segment(
        &self,
        segment: &MediaSegmentInfo,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
        has_pending_append: &impl Fn(f64, f64) -> bool,
    ) -> bool {
        segment.duration() > 0.
            && segment.end() > self.base_pos
            && !has_pending_append(segment.start(), segment.end())
            && self
                .validated_segments
                .needs_loading(context.playlist_id(), segment.identity())
            && !self.is_range_buffered(segment.start(), segment.end(), inventory)
    }

    /// Check coverage for additions. Quality replacements are selected separately.
    fn is_range_buffered(&self, start: f64, end: f64, inventory: &[BufferedChunk]) -> bool {
        let first_seg_pos = inventory.iter().position(|s| s.playlist_end() > start);
        if let Some(mut curr_idx) = first_seg_pos {
            let mut prev_seg_end = start;
            while let Some(mut curr_seg) = inventory.get(curr_idx) {
                if curr_seg.appears_garbage_collected(self.base_pos)
                    || curr_seg.playlist_start() > (prev_seg_end + 0.05)
                {
                    return false;
                }

                if curr_seg.playlist_end() >= end {
                    return true;
                }

                curr_idx += 1;
                if curr_idx >= inventory.len() {
                    return false;
                }

                prev_seg_end = curr_seg.playlist_end();
                curr_seg = inventory.get(curr_idx).unwrap();
                if curr_seg.playlist_start() >= end {
                    return true;
                }
            }
        }
        false
    }
}

/// "Validation status" regarding the initialization segment.
///
/// This enumeration allows to keep track of if the initialization segment for the last asked media
/// was validated (or if it doesn't exist), or not, thus allowing the `NextSegmentSelector` to
/// indicate whether it should be loaded or not.
#[derive(Clone, Copy, Debug)]
enum InitializationSegmentSelectorStatus {
    /// We did not check for an initialization segment yet.
    Unchecked,
    /// No initialization segment exist for that media
    NoneExists,
    /// We checked an returned an initialization segment the last call which add as an `id`
    /// property the f64 attached to this enum variant.
    ///
    /// Note that this `id` only identifies initialization segments per-quality. This identifier
    /// can be repeated in other qualities.
    Unvalidated(f64),
    /// An initialization segment exists and was "validated" it is not necessary to return it
    /// anymore.
    /// The associated `f64` is the `id` of that initialization segment.
    ///
    /// Note that this `id` only identifies initialization segments per-quality. This identifier
    /// can be repeated in other qualities.
    Validated(f64),
}

/// One variant the selector may use, with its latest available media playlist.
#[derive(Clone, Copy)]
pub(crate) struct SegmentSelectionCandidate<'a> {
    pub(crate) variant_id: u32,
    pub(crate) context: SegmentQualityContext,
    /// Cached segments may be selected without waiting for a live playlist refresh. `None` means
    /// no playlist has been loaded; refresh timing remains the caller's responsibility.
    ///
    /// TODO: When integrating the dispatcher, recover from expired cached segment requests using
    /// the failed segment's own media playlist. Refresh it with the normal reload timing and
    /// reconsider selection on terminal 404/410 instead of stopping playable buffered content.
    pub(crate) playlist: Option<&'a SegmentList>,
    pub(crate) is_ended: bool,
}

/// Candidate for a segment that could be loaded for a given set of criteria.
enum SegmentProposition<'a> {
    /// None seems to match the given criteria currently.
    None,
    /// A segment matching the criteria might be found if/when refreshing the corresponding
    /// Media Playlist.
    NeedsPlaylistUpdate,
    /// There's a proposition matching given criteria.
    Segment(&'a MediaSegmentInfo),
}

pub(crate) struct MostNeededSegmentInfo<'a> {
    /// Segments to request now. They may come from addition_candidate while replacement_candidate's playlist is updated.
    needed_segments: Option<NeededSegmentInfo<'a>>,
    /// All required media through the end of a finalized playlist has been supplied.
    loading_complete: bool,
    /// ID of the playlist that should be fetched or normally refreshed. It may differ from
    /// the playlist containing `needed_segments`.
    playlist_to_refresh: MediaPlaylistPermanentId,
}

impl<'a> MostNeededSegmentInfo<'a> {
    pub(crate) fn loading_complete(&self) -> bool {
        self.loading_complete
    }

    pub(crate) fn needed_segments(&self) -> Option<&NeededSegmentInfo<'a>> {
        self.needed_segments.as_ref()
    }

    pub(crate) fn playlist_to_refresh(&self) -> MediaPlaylistPermanentId {
        self.playlist_to_refresh
    }
}

/// Segment information for segments that may now be loaded as returned by the
/// `NextSegmentSelector`.
///
/// Its lifetime is generally linked to the `MediaPlaylist` to which those information are
/// initially linked to.
pub(crate) struct NeededSegmentInfo<'a> {
    // Variant linked to the segment(s) that should be loaded.
    variant_id: u32,
    /// The initialization segment that should now be needed, corresponding to the inner
    /// information.
    ///
    /// `None` either if there's no needed initialization segment or if we consider that the last
    /// validated one is still compatible.
    init_segment: Option<&'a InitSegmentInfo>,
    /// The media segment to load and its associated initialization ID, even if already validated.
    ///
    /// `None` if no media segment is currently needed.
    media_segment: Option<(&'a MediaSegmentInfo, Option<f64>)>,
}

impl<'a> NeededSegmentInfo<'a> {
    pub(crate) fn variant_id(&self) -> u32 {
        self.variant_id
    }

    /// Returns the media segment to load and its associated initialization ID, if needed.
    pub(crate) fn media_segment(&self) -> Option<(&'a MediaSegmentInfo, Option<f64>)> {
        self.media_segment
    }

    /// Returns the initialization segment that should be loaded.
    ///
    /// `None` if no initialization segment exists or the last validated one is still compatible.
    pub(crate) fn init_segment(&self) -> Option<&InitSegmentInfo> {
        self.init_segment
    }
}

#[cfg(test)]
mod tests {
    use super::{NextSegmentSelector, NextSegmentSelectors, SegmentSelectionCandidate};
    use crate::{
        bindings::MediaType,
        media_element::{BufferedChunk, SegmentQualityContext},
        parser::{MediaPlaylistPermanentId, MediaSegmentInfo, SegmentList, TopLevelPlaylist},
        utils::url::Url,
    };

    fn test_playlist_id(id: u32) -> MediaPlaylistPermanentId {
        MediaPlaylistPermanentId::new_for_test(id)
    }

    fn candidate<'a>(
        list: &'a SegmentList,
        context: &SegmentQualityContext,
    ) -> SegmentSelectionCandidate<'a> {
        SegmentSelectionCandidate {
            variant_id: context.playlist_id().as_u32(),
            context: *context,
            playlist: Some(list),
            is_ended: false,
        }
    }

    fn validate(
        selector: &mut NextSegmentSelector,
        segment: &MediaSegmentInfo,
        context: &SegmentQualityContext,
    ) {
        selector.validate_media(
            context.playlist_id(),
            segment.identity(),
            segment.start(),
            segment.end(),
        );
    }

    fn playlist_starting_at(start_second: u32, segment_count: u32) -> TopLevelPlaylist {
        let mut text = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-PROGRAM-DATE-TIME:1970-01-01T00:00:{start_second:02}Z\n"
        );
        for number in 0..segment_count {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/media.m3u8".to_owned()),
        )
        .unwrap()
    }

    #[test]
    fn append_results_establish_coverage_and_detect_loss_before_the_next_playback_tick() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 2) else {
            panic!("expected media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        for initially_empty in [false, true] {
            let mut selector = NextSegmentSelector::new(0., 30.);
            // Observations before completion must not become the attempt's baseline.
            selector.check_for_removed_data(&[(0., 4.)]);
            let pending = |start, end| start < 4. && end > 0.;
            let next = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                pending,
            );
            assert_eq!(
                next.needed_segments
                    .unwrap()
                    .media_segment
                    .unwrap()
                    .0
                    .start(),
                4.
            );

            validate(&mut selector, &list.media()[0], &context);
            selector.check_for_removed_data(if initially_empty { &[] } else { &[(0., 4.)] });
            let next = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                |_, _| false,
            );
            assert_eq!(
                next.needed_segments
                    .unwrap()
                    .media_segment
                    .unwrap()
                    .0
                    .start(),
                4.
            );
            if initially_empty {
                // Coverage may only become visible in a later observation.
                selector.check_for_removed_data(&[(0., 4.)]);
            }

            // The next append's result removes the first segment's prefix.
            validate(&mut selector, &list.media()[1], &context);
            selector.check_for_removed_data(&[(2., 8.)]);
            let retry = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                |_, _| false,
            );
            assert_eq!(
                retry
                    .needed_segments
                    .unwrap()
                    .media_segment
                    .unwrap()
                    .0
                    .start(),
                0.
            );

            // A retry which restores nothing must not cause a loading loop.
            validate(&mut selector, &list.media()[0], &context);
            for _ in 0..3 {
                selector.check_for_removed_data(&[(2., 8.)]);
                assert!(selector
                    .most_needed_segment(
                        candidate(list, &context),
                        candidate(list, &context),
                        &[],
                        |_, _| false,
                    )
                    .needed_segments
                    .is_none());
            }
        }
    }

    #[test]
    fn rescanning_does_not_request_pending_appends_after_a_history_reset() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 3) else {
            panic!("expected media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let first = &list.media()[0];
        for (reset_history, pending_start, pending_end, expected_start) in [
            (false, 0., 4., 4.),
            (true, 0., 4., 4.),
            // An append from another quality may overlap two playlist segments.
            (false, 2., 6., 8.),
            (true, 2., 6., 8.),
        ] {
            let has_pending_append = |start, end| start < pending_end && end > pending_start;
            let mut selector = NextSegmentSelector::new(0., 30.);
            validate(&mut selector, first, &context);
            if reset_history {
                selector.restart_from_position(0.);
            }
            // Inventory may have lost its pending entry before completion.
            // The append queue must still prevent another request for it.
            for _ in 0..3 {
                let selected = selector.most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &[],
                    has_pending_append,
                );
                assert_eq!(
                    selected
                        .needed_segments
                        .unwrap()
                        .media_segment
                        .unwrap()
                        .0
                        .start(),
                    expected_start
                );
            }
        }
    }

    #[test]
    fn rescanning_recovers_lost_coverage_without_loading_history() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 5) else {
            panic!("expected media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        for lost_tail in [false, true] {
            let mut inventory: Vec<_> = list.media()[..4]
                .iter()
                .map(|segment| BufferedChunk::new_for_test(segment.start(), segment.end(), context))
                .collect();
            let mut selector = NextSegmentSelector::new(1., 30.);
            let selected = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &inventory,
                |_, _| false,
            );
            let segment = selected.needed_segments.unwrap().media_segment.unwrap().0;
            assert_eq!(segment.start(), 16.);
            validate(&mut selector, segment, &context);

            if lost_tail {
                inventory.truncate(2);
            } else {
                // The 4-8s segment loses its prefix, as when an earlier append
                // overlaps its first keyframe. The remainder stays buffered.
                inventory[1] = BufferedChunk::new_for_test(6., 8., context);
            }
            let selected = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &inventory,
                |_, _| false,
            );
            let segment = selected.needed_segments.unwrap().media_segment.unwrap().0;
            assert_eq!(segment.start(), if lost_tail { 8. } else { 4. });
            validate(&mut selector, segment, &context);

            // Repeatedly observing the same missing coverage after supplying
            // the retry must not keep proposing that segment.
            let next = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &inventory,
                |_, _| false,
            );
            assert!(next.needed_segments.is_none_or(|needed| {
                needed.media_segment.unwrap().0.start() != segment.start()
            }));
        }
    }

    #[test]
    fn completion_distinguishes_buffer_goal_endlist_and_missing_media() {
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n0.m4s\n#EXTINF:4,\n1.m4s\n#EXT-X-ENDLIST\n",
            Url::new("https://example.com/video.m3u8".into()),
        ).unwrap() else { panic!("expected media playlist"); };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        let mut selector = NextSegmentSelector::new(0., 2.);
        let mut inventory = vec![BufferedChunk::new_for_test(0., 4., context)];
        validate(&mut selector, &list.media()[0], &context);

        let goal = selector.most_needed_segment(ended(), ended(), &inventory, |_, _| false);
        assert!(goal.needed_segments().is_none());
        assert!(!goal.loading_complete());
        selector.buffer_goal = 20.;
        let last = selector.most_needed_segment(ended(), ended(), &inventory, |_, _| false);
        assert!(!last.loading_complete());
        assert_eq!(
            last.needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            4.
        );
        validate(&mut selector, &list.media()[1], &context);
        inventory.push(BufferedChunk::new_for_test(4., 8., context));
        selector.check_for_removed_data(&[(0., 8.)]);
        assert!(selector
            .most_needed_segment(ended(), ended(), &inventory, |_, _| false)
            .loading_complete());

        // A live playlist may still grow even when all its current media is present.
        assert!(!selector
            .most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &inventory,
                |_, _| false
            )
            .loading_complete());
        let missing = || SegmentSelectionCandidate {
            playlist: None,
            is_ended: true,
            ..candidate(list, &context)
        };
        assert!(!selector
            .most_needed_segment(missing(), missing(), &inventory, |_, _| false)
            .loading_complete());

        inventory.pop();
        selector.check_for_removed_data(&[(0., 4.)]);
        let evicted = selector.most_needed_segment(ended(), ended(), &inventory, |_, _| false);
        assert!(!evicted.loading_complete());
        assert_eq!(
            evicted
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            4.
        );
    }

    #[test]
    fn completion_ignores_time_before_the_playlist_start() {
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-PROGRAM-DATE-TIME:2024-01-02T03:04:05Z\n#EXTINF:4,\n0.m4s\n#EXTINF:4,\n1.m4s\n#EXT-X-ENDLIST\n",
            Url::new("https://example.com/video.m3u8".into()),
        ).unwrap() else { panic!("expected media playlist"); };
        let list = playlist.playlist().segment_list();
        assert_eq!(list.media()[0].start(), 1_704_164_645.);
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        let inventory: Vec<_> = list
            .media()
            .iter()
            .map(|segment| BufferedChunk::new_for_test(segment.start(), segment.end(), context))
            .collect();
        let playlist_start = list.media()[0].start();
        for base_pos in [0., playlist_start - 0.2, playlist_start] {
            let mut selector = NextSegmentSelector::new(base_pos, 30.);
            assert!(selector
                .most_needed_segment(ended(), ended(), &inventory, |_, _| false)
                .loading_complete());
        }
    }

    #[test]
    fn completion_survives_supplying_earlier_media_after_the_final_segment() {
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n0.m4s\n#EXTINF:4,\n1.m4s\n#EXT-X-ENDLIST\n",
            Url::new("https://example.com/video.m3u8".into()),
        ).unwrap() else { panic!("expected media playlist"); };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        let mut selector = NextSegmentSelector::new(4., 20.);
        let mut inventory = vec![BufferedChunk::new_for_test(4., 8., context)];
        validate(&mut selector, &list.media()[1], &context);
        assert!(selector
            .most_needed_segment(ended(), ended(), &inventory, |_, _| false)
            .loading_complete());

        selector.restart_from_position(0.);
        let earlier = selector.most_needed_segment(ended(), ended(), &inventory, |_, _| false);
        assert!(!earlier.loading_complete());
        assert_eq!(
            earlier
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            0.
        );
        validate(&mut selector, &list.media()[0], &context);
        inventory.insert(0, BufferedChunk::new_for_test(0., 4., context));
        assert!(selector
            .most_needed_segment(ended(), ended(), &inventory, |_, _| false)
            .loading_complete());
        // Completion doesn't require advancing the buffer goal past already-buffered data.
        selector.buffer_goal = 0.;
        assert!(selector
            .most_needed_segment(ended(), ended(), &inventory, |_, _| false)
            .loading_complete());
    }

    #[test]
    fn gc_recovery_revisits_earlier_media_before_reaching_the_playlist_end() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 10) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let mut selector = NextSegmentSelector::new(0., 20.);
        let mut inventory = vec![];
        for segment in &list.media()[..3] {
            validate(&mut selector, segment, &context);
            inventory.push(BufferedChunk::new_for_test(
                segment.start(),
                segment.end(),
                context,
            ));
        }
        selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &inventory,
            |_, _| false,
        );
        selector.check_for_removed_data(&[(0., 12.)]);
        inventory.remove(1);
        selector.check_for_removed_data(&[(0., 4.), (8., 12.)]);
        let selected = selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &inventory,
            |_, _| false,
        );
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            4.
        );
        validate(&mut selector, &list.media()[1], &context);
        let selected = selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &inventory,
            |_, _| false,
        );
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            12.
        );
    }

    #[test]
    fn a_gap_in_the_playlist_does_not_prevent_completion() {
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-PROGRAM-DATE-TIME:1970-01-01T00:00:00Z\n#EXTINF:4,\n0.m4s\n#EXT-X-DISCONTINUITY\n#EXT-X-PROGRAM-DATE-TIME:1970-01-01T00:00:08Z\n#EXTINF:4,\n1.m4s\n#EXT-X-ENDLIST\n",
            Url::new("https://example.com/video.m3u8".into()),
        ).unwrap() else { panic!("expected media playlist"); };
        let list = playlist.playlist().segment_list();
        assert_eq!(list.media()[1].start(), 8.);
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let inventory = vec![
            BufferedChunk::new_for_test(0., 4., context),
            BufferedChunk::new_for_test(8., 12., context),
        ];
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        let mut selector = NextSegmentSelector::new(0., 30.);
        let selected = selector.most_needed_segment(ended(), ended(), &inventory, |_, _| false);
        assert!(selected.needed_segments().is_none());
        assert!(selected.loading_complete());
    }

    #[test]
    fn buffer_loss_and_retry_do_not_require_inventory_entries() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 3) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        let mut selector = NextSegmentSelector::new(0., 30.);
        for segment in list.media() {
            validate(&mut selector, segment, &context);
        }
        // The inventory lost its entries before append completion. History must
        // still stop duplicate loads, then notice data appearing and disappearing.
        selector.check_for_removed_data(&[]);
        let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
        assert!(selected.loading_complete());
        assert!(selected.needed_segments().is_none());
        selector.check_for_removed_data(&[(0., 12.)]);
        selector.check_for_removed_data(&[(0., 4.), (8., 12.)]);
        let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
        assert!(!selected.loading_complete());
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            4.
        );
        validate(&mut selector, &list.media()[1], &context);
        // The retry does not restore the missing data: do not request it in a loop.
        for _ in 0..3 {
            selector.check_for_removed_data(&[(0., 4.), (8., 12.)]);
            let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
            assert!(selected.loading_complete());
            assert!(selected.needed_segments().is_none());
        }
        // If data really returns, a separate later removal can be recovered again.
        selector.check_for_removed_data(&[(0., 12.)]);
        selector.check_for_removed_data(&[(0., 4.), (8., 12.)]);
        let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
        assert!(!selected.loading_complete());
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            4.
        );
    }

    #[test]
    fn accepted_media_without_inventory_can_complete_without_reloading() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(10, 2) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let mut selector = NextSegmentSelector::new(9.8, 30.);
        let ended = || SegmentSelectionCandidate {
            is_ended: true,
            ..candidate(list, &context)
        };
        for segment in list.media() {
            validate(&mut selector, segment, &context);
        }
        for _ in 0..3 {
            let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
            assert!(selected.needed_segments().is_none());
            assert!(selected.loading_complete());
        }
        selector.restart_from_position(10.);
        let selected = selector.most_needed_segment(ended(), ended(), &[], |_, _| false);
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            10.
        );
        assert!(!selected.loading_complete());
    }

    #[test]
    fn forward_selection_keeps_up_with_playback_without_validation() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 10) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let mut selector = NextSegmentSelector::new(0., 30.);

        for (position, expected_start) in [(0., 0.), (3.8, 0.), (4., 4.), (12.1, 12.), (12.1, 12.)]
        {
            selector.advance_position(position);
            let selected = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                |_, _| false,
            );
            assert_eq!(
                selected
                    .needed_segments()
                    .unwrap()
                    .media_segment()
                    .unwrap()
                    .0
                    .start(),
                expected_start
            );
        }
    }

    #[test]
    fn late_validation_does_not_skip_missing_earlier_media() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 10) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let mut selector = NextSegmentSelector::new(0., 30.);
        // Establish the playlist before validating media, as during playback.
        selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &[],
            |_, _| false,
        );
        for (validated_index, expected_start) in [(5, 8.), (0, 8.)] {
            selector.advance_position(10.);
            validate(&mut selector, &list.media()[validated_index], &context);
            let selected = selector.most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                |_, _| false,
            );
            assert_eq!(
                selected
                    .needed_segments()
                    .unwrap()
                    .media_segment()
                    .unwrap()
                    .0
                    .start(),
                expected_start
            );
        }
        selector.restart_from_position(0.);
        let selected = selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &[],
            |_, _| false,
        );
        assert_eq!(
            selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            0.
        );
    }

    #[test]
    fn live_refresh_reconsiders_previously_unavailable_replacements() {
        let TopLevelPlaylist::DirectMedia(initial) = playlist_starting_at(0, 2) else {
            panic!("expected direct media playlist");
        };
        let TopLevelPlaylist::DirectMedia(updated) = playlist_starting_at(0, 10) else {
            panic!("expected direct media playlist");
        };
        let best_list = updated.playlist().segment_list();
        let safe_list = initial.playlist().segment_list();
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);

        let first = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            candidate(safe_list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        let first = first.needed_segments().unwrap();
        assert_eq!(
            first.variant_id(),
            addition_candidate.playlist_id().as_u32()
        );
        validate(
            &mut selector,
            first.media_segment().unwrap().0,
            &addition_candidate,
        );

        let refreshed = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            candidate(best_list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(refreshed.playlist_to_refresh(), test_playlist_id(2));
        assert_eq!(
            refreshed
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            8.
        );
    }

    #[test]
    fn accepted_replacements_survive_missing_buffer_and_candidate_changes() {
        let TopLevelPlaylist::DirectMedia(playlist) = playlist_starting_at(0, 10) else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        // Keep lower-quality inventory unchanged, as if every replacement failed to buffer.
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);
        // Validate a later segment first: this must not suppress earlier untried segments.
        validate(&mut selector, &list.media()[4], &replacement_candidate);
        for expected_start in [8., 12., 20.] {
            let selected = selector.most_needed_segment(
                candidate(list, &addition_candidate),
                candidate(list, &replacement_candidate),
                &inventory,
                |_, _| false,
            );
            let segment = selected
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0;
            assert_eq!(segment.start(), expected_start);
            validate(&mut selector, segment, &replacement_candidate);
        }

        let changed_best = SegmentQualityContext::new(4., test_playlist_id(4));
        let stale = selector.most_needed_segment(
            candidate(list, &changed_best),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: SegmentQualityContext::new(2.5, replacement_candidate.playlist_id()),
                playlist: Some(list),
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(stale.playlist_to_refresh(), test_playlist_id(4));

        selector.restart_from_position(0.);
        let retry = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            candidate(list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(
            retry
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            8.
        );
    }

    #[test]
    fn additions_resume_after_buffer_when_replacements_stop_on_the_same_playlist() {
        let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/media.m3u8".into()),
        )
        .unwrap() else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let low = SegmentQualityContext::new(1., test_playlist_id(1));
        let high = SegmentQualityContext::new(2., test_playlist_id(2));
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(number as f64 * 4., (number + 1) as f64 * 4., low)
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);
        let replacement = selector
            .most_needed_segment(
                candidate(list, &high),
                candidate(list, &high),
                &inventory,
                |_, _| false,
            )
            .needed_segments
            .unwrap()
            .media_segment
            .unwrap()
            .0;
        assert_eq!(replacement.start(), 8.);
        validate(&mut selector, replacement, &high);

        // Throughput no longer permits replacements, but BOLA still permits high additions.
        let addition = selector
            .most_needed_segment(
                candidate(list, &high),
                candidate(list, &low),
                &inventory,
                |_, _| false,
            )
            .needed_segments
            .unwrap()
            .media_segment
            .unwrap()
            .0;
        assert_eq!(addition.start(), 24.);
        validate(&mut selector, addition, &high);
        let following = selector
            .most_needed_segment(
                candidate(list, &high),
                candidate(list, &low),
                &inventory,
                |_, _| false,
            )
            .needed_segments
            .unwrap()
            .media_segment
            .unwrap()
            .0;
        assert_eq!(following.start(), 28.);

        // Recovery may resume replacements without losing the ability to return to additions.
        let replacement = selector
            .most_needed_segment(
                candidate(list, &high),
                candidate(list, &high),
                &inventory,
                |_, _| false,
            )
            .needed_segments
            .unwrap()
            .media_segment
            .unwrap()
            .0;
        assert_eq!(replacement.start(), 12.);
    }

    #[test]
    fn selects_safe_for_replacement_and_best_for_buffer_addition() {
        let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/media.m3u8".to_owned()),
        )
        .unwrap() else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);

        let replacement = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 30,
                context: addition_candidate,
                playlist: Some(list),
                is_ended: false,
            },
            SegmentSelectionCandidate {
                variant_id: 20,
                context: replacement_candidate,
                playlist: Some(list),
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(replacement.playlist_to_refresh, test_playlist_id(2));
        let replacement = replacement.needed_segments.unwrap();
        assert_eq!(replacement.variant_id, 20);
        assert_eq!(replacement.media_segment.unwrap().0.start(), 8.);

        let repeated = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            candidate(list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(
            repeated
                .needed_segments
                .unwrap()
                .media_segment
                .unwrap()
                .0
                .start(),
            8.
        );

        validate(
            &mut selector,
            replacement.media_segment.unwrap().0,
            &replacement_candidate,
        );
        let next = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            candidate(list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(
            next.needed_segments
                .unwrap()
                .media_segment
                .unwrap()
                .0
                .start(),
            12.
        );

        selector.restart_from_position(0.);
        let addition = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 30,
                context: addition_candidate,
                playlist: Some(list),
                is_ended: false,
            },
            SegmentSelectionCandidate {
                variant_id: 20,
                context: replacement_candidate,
                playlist: Some(list),
                is_ended: false,
            },
            &[],
            |_, _| false,
        );
        let addition = addition.needed_segments.unwrap();
        assert_eq!(addition.variant_id, 30);
        assert_eq!(addition.media_segment.unwrap().0.start(), 0.);
    }

    #[test]
    fn loads_best_segments_while_requesting_missing_safe_playlist() {
        let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/media.m3u8".to_owned()),
        )
        .unwrap() else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);

        let while_missing = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: replacement_candidate,
                playlist: None,
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(while_missing.playlist_to_refresh(), test_playlist_id(2));
        let best_segment = while_missing.needed_segments().unwrap();
        assert_eq!(best_segment.variant_id(), 3);
        assert_eq!(best_segment.media_segment().unwrap().0.start(), 24.);

        selector.buffer_goal = 20.;
        selector.restart_from_position(0.);
        let while_full = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: replacement_candidate,
                playlist: None,
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(while_full.playlist_to_refresh(), test_playlist_id(2));
        assert!(while_full.needed_segments().is_none());

        selector.buffer_goal = 40.;

        let after_load = selector.most_needed_segment(
            candidate(list, &addition_candidate),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: replacement_candidate,
                playlist: Some(list),
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(after_load.playlist_to_refresh(), test_playlist_id(2));
        let safe_segment = after_load.needed_segments().unwrap();
        assert_eq!(safe_segment.variant_id(), 20);
        assert_eq!(safe_segment.media_segment().unwrap().0.start(), 8.);
    }

    #[test]
    fn cached_safe_window_rules_out_older_buffered_media() {
        let best_playlist = playlist_starting_at(0, 12);
        let safe_playlist = playlist_starting_at(28, 3);
        let TopLevelPlaylist::DirectMedia(best_playlist) = &best_playlist else {
            panic!("expected direct media playlist");
        };
        let TopLevelPlaylist::DirectMedia(safe_playlist) = &safe_playlist else {
            panic!("expected direct media playlist");
        };
        let best_list = best_playlist.playlist().segment_list();
        let safe_list = safe_playlist.playlist().segment_list();
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        let mut inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 48.);

        let missing = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            SegmentSelectionCandidate {
                variant_id: 2,
                context: replacement_candidate,
                playlist: None,
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(missing.playlist_to_refresh(), test_playlist_id(2));
        assert_eq!(missing.needed_segments().unwrap().variant_id(), 3);

        let inspected = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            candidate(safe_list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(inspected.playlist_to_refresh(), test_playlist_id(3));
        assert!(!selector
            .was_segment_validated(replacement_candidate.playlist_id(), &safe_list.media()[0]));

        let repeated = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            SegmentSelectionCandidate {
                variant_id: 2,
                context: replacement_candidate,
                playlist: Some(safe_list),
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(repeated.playlist_to_refresh(), test_playlist_id(3));

        inventory.extend((6..8).map(|number| {
            BufferedChunk::new_for_test(
                number as f64 * 4.,
                (number + 1) as f64 * 4.,
                SegmentQualityContext::new(1., test_playlist_id(1)),
            )
        }));
        let new_opportunity = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: addition_candidate.playlist_id().as_u32(),
                context: addition_candidate,
                playlist: None,
                is_ended: false,
            },
            SegmentSelectionCandidate {
                variant_id: 2,
                context: replacement_candidate,
                playlist: Some(safe_list),
                is_ended: false,
            },
            &inventory,
            |_, _| false,
        );
        assert_eq!(new_opportunity.playlist_to_refresh(), test_playlist_id(2));
        // Use the cached replacement_candidate segment immediately, even without a addition_candidate playlist, and refresh
        // replacement_candidate alongside the segment request. No fresh-playlist confirmation is required.
        let cached_segment = new_opportunity.needed_segments().unwrap();
        assert_eq!(
            cached_segment.variant_id(),
            replacement_candidate.playlist_id().as_u32()
        );
        assert_eq!(cached_segment.media_segment().unwrap().0.start(), 28.);

        let replacement = selector.most_needed_segment(
            candidate(best_list, &addition_candidate),
            candidate(safe_list, &replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(replacement.playlist_to_refresh(), test_playlist_id(2));
        assert_eq!(
            replacement
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            28.
        );
    }

    #[test]
    fn safe_playlist_can_replace_a_later_buffered_chunk() {
        let best_playlist = playlist_starting_at(0, 12);
        let safe_playlist = playlist_starting_at(16, 4);
        let TopLevelPlaylist::DirectMedia(best_playlist) = &best_playlist else {
            panic!("expected direct media playlist");
        };
        let TopLevelPlaylist::DirectMedia(safe_playlist) = &safe_playlist else {
            panic!("expected direct media playlist");
        };
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., test_playlist_id(1)),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);
        let result = selector.most_needed_segment(
            candidate(
                best_playlist.playlist().segment_list(),
                &SegmentQualityContext::new(3., test_playlist_id(3)),
            ),
            candidate(
                safe_playlist.playlist().segment_list(),
                &SegmentQualityContext::new(2., test_playlist_id(2)),
            ),
            &inventory,
            |_, _| false,
        );
        assert_eq!(result.playlist_to_refresh(), test_playlist_id(2));
        assert_eq!(
            result
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .0
                .start(),
            16.
        );
    }

    #[test]
    fn requests_selected_playlist_before_selecting_media() {
        let mut selector = NextSegmentSelector::new(0., 40.);
        let replacement_candidate = SegmentQualityContext::new(2., test_playlist_id(2));
        let addition_candidate = SegmentQualityContext::new(3., test_playlist_id(3));
        let missing = |context: &SegmentQualityContext| SegmentSelectionCandidate {
            variant_id: context.playlist_id().as_u32(),
            context: *context,
            playlist: None,
            is_ended: false,
        };
        let inventory = vec![
            BufferedChunk::new_for_test(
                0.,
                6.,
                SegmentQualityContext::new(1., test_playlist_id(1)),
            ),
            BufferedChunk::new_for_test(
                6.,
                10.,
                SegmentQualityContext::new(1., test_playlist_id(1)),
            ),
        ];

        let replacement = selector.most_needed_segment(
            missing(&addition_candidate),
            missing(&replacement_candidate),
            &inventory,
            |_, _| false,
        );
        assert_eq!(replacement.playlist_to_refresh, test_playlist_id(2));
        assert!(replacement.needed_segments.is_none());

        let addition = selector.most_needed_segment(
            missing(&addition_candidate),
            missing(&replacement_candidate),
            &[],
            |_, _| false,
        );
        assert_eq!(addition.playlist_to_refresh, test_playlist_id(3));
        assert!(addition.needed_segments.is_none());
    }

    #[test]
    fn restart_reconsiders_buffered_coverage_and_preserves_playlist_identity() {
        let mut text =
            String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\"\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        let TopLevelPlaylist::DirectMedia(playlist) = TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/high.m3u8".to_owned()),
        )
        .unwrap() else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        for (has_replacement, seek_position, expected_start, change_media) in [
            (false, 4., 24., false),
            (true, 4., 8., false),
            (false, 30., 28., false),
            (false, 4., 24., true),
        ] {
            let context = SegmentQualityContext::new(2., test_playlist_id(2));
            let inventory: Vec<_> = (0..6)
                .map(|number| {
                    BufferedChunk::new_for_test(
                        number as f64 * 4.,
                        (number + 1) as f64 * 4.,
                        if has_replacement {
                            SegmentQualityContext::new(1., test_playlist_id(1))
                        } else {
                            context
                        },
                    )
                })
                .collect();
            let mut selectors = NextSegmentSelectors::new(0., 40.);
            let selector = selectors.get_mut(MediaType::Video);
            let first = selector
                .most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &inventory,
                    |_, _| false,
                )
                .needed_segments
                .unwrap();
            let init_segment_id = first.init_segment.unwrap().id();
            assert_eq!(first.media_segment.unwrap().1, Some(init_segment_id));
            selector.validate_init(init_segment_id);
            validate(selector, first.media_segment.unwrap().0, &context);

            selector.restart_from_position(seek_position - 0.2);
            let context =
                SegmentQualityContext::new(2., test_playlist_id(if change_media { 3 } else { 2 }));
            let next = selector
                .most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &inventory,
                    |_, _| false,
                )
                .needed_segments
                .unwrap();
            assert_eq!(next.media_segment.unwrap().0.start(), expected_start);
            assert_eq!(next.init_segment.is_some(), change_media);
            assert_eq!(next.media_segment.unwrap().1, Some(init_segment_id));
            validate(selector, next.media_segment.unwrap().0, &context);

            let following = selector
                .most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &inventory,
                    |_, _| false,
                )
                .needed_segments
                .unwrap();
            assert_eq!(
                following.media_segment.unwrap().0.start(),
                expected_start + 4.
            );
        }
    }

    #[test]
    fn above_throughput_switch_can_continue_after_buffer_without_replacement() {
        let mut selector = NextSegmentSelector::new(0., 30.);
        let buffered = vec![
            BufferedChunk::new_for_test(
                0.,
                6.,
                SegmentQualityContext::new(1., test_playlist_id(1)),
            ),
            BufferedChunk::new_for_test(
                6.,
                10.,
                SegmentQualityContext::new(1., test_playlist_id(1)),
            ),
        ];
        let higher_quality = SegmentQualityContext::new(2., test_playlist_id(2));

        let result = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 3,
                context: SegmentQualityContext::new(3., test_playlist_id(3)),
                playlist: None,
                is_ended: false,
            },
            SegmentSelectionCandidate {
                variant_id: 2,
                context: higher_quality,
                playlist: None,
                is_ended: false,
            },
            &buffered,
            |_, _| false,
        );
        assert_eq!(result.playlist_to_refresh(), test_playlist_id(2));
    }

    #[test]
    fn fast_quality_switch_requires_the_minimum_lead() {
        for (lead, duration, replacement_allowed) in [
            (3., 2., false),
            (3.01, 2., true),
            (4., 6., false),
            (6., 6., false),
            (6.01, 6., true),
        ] {
            let mut selector = NextSegmentSelector::new(0., 30.);
            let low = SegmentQualityContext::new(1., test_playlist_id(1));
            let buffered = vec![
                BufferedChunk::new_for_test(0., lead, low),
                BufferedChunk::new_for_test(lead, lead + duration, low),
            ];
            let result = selector.most_needed_segment(
                SegmentSelectionCandidate {
                    variant_id: 3,
                    context: SegmentQualityContext::new(3., test_playlist_id(3)),
                    playlist: None,
                    is_ended: false,
                },
                SegmentSelectionCandidate {
                    variant_id: 2,
                    context: SegmentQualityContext::new(2., test_playlist_id(2)),
                    playlist: None,
                    is_ended: false,
                },
                &buffered,
                |_, _| false,
            );
            // A permitted replacement needs its playlist; otherwise follow the addition playlist.
            assert_eq!(
                result.playlist_to_refresh(),
                test_playlist_id(if replacement_allowed { 2 } else { 3 }),
                "lead={lead}, duration={duration}",
            );
        }
    }

    #[test]
    fn selection_skips_trailing_zero_duration_segment() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:3
#EXTINF:3,
seg-0.m4s
#EXTINF:0,
seg-1.m4s
"#;
        let parsed = TopLevelPlaylist::parse(
            playlist.as_bytes(),
            Url::new("https://example.com/media.m3u8".to_owned()),
        )
        .unwrap();
        let TopLevelPlaylist::DirectMedia(ref playlist) = parsed else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let context = SegmentQualityContext::new(1., test_playlist_id(1));
        let mut selector = NextSegmentSelector::new(2.5, 10.);
        let next = selector.most_needed_segment(
            candidate(list, &context),
            candidate(list, &context),
            &[],
            |_, _| false,
        );
        let segment = next.needed_segments.unwrap().media_segment.unwrap().0;
        assert_eq!(segment.sequence(), 0);
        validate(&mut selector, segment, &context);
        assert!(selector
            .most_needed_segment(
                candidate(list, &context),
                candidate(list, &context),
                &[],
                |_, _| false
            )
            .needed_segments
            .is_none());
    }
}
