use crate::{
    bindings::MediaType,
    media_element::{BufferedChunk, SegmentQualityContext},
    parser::{InitSegmentInfo, MediaSegmentInfo, SegmentList, SegmentTimeInfo},
    utils::logger::*,
};

/// Minimum distance from playback, in seconds, before replacing buffered media.
const MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS: f64 = 5.;

/// Indicate the most prioritary segment to load according to the given situation.
///
/// Internally, the `NextSegmentSelectors` contains a `NextSegmentSelector` for each type of media,
/// each keeping a state keeping track of which segment has already been loaded (or as written
/// here, "validated"), which did not need to be loaded and other state helping it toward
/// communicating which segment it thinks should be loaded next.
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
    /// If the position completely changes due to a seek, or if the buffer is emptied due to an
    /// exceptional event, the `restart_from_position` method should be called instead to prevent
    /// the `NextSegmentSelector`s from just continuing to provide the following segments.
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

    /// Updates the "base position" - used by the `NextSegmentSelectors` and its inner
    /// `NextSegmentSelector`s - after it advances due to regular content playback.
    ///
    /// The "base position" is the starting position, in playlist time in seconds, from which a
    /// `NextSegmentSelector` might want to look for segments. It is generally intended to be set to
    /// the current position.
    /// As such, the "base position" has to be updated regularly, to improve the
    /// `NextSegmentSelector`'s accuracy.
    ///
    /// /!\ This method should only be called when the position advances due to regular playback.
    /// In case of a seek, or to reset the `NextSegmentSelector` after the buffer has been emptied,
    /// `restart_from_position` should be called instead so the `NextSegmentSelector`s don't just
    /// continue to provide you with the next chronological segment as they do normally (with some
    /// exceptions).
    pub(crate) fn advance_position(&mut self, pos: f64) {
        self.audio.advance_position(pos);
        self.video.advance_position(pos);
    }

    /// Force the `NextSegmentSelectors` and its inner `NextSegmentSelector`s to re-consider
    /// segments from the new given "base position", generally due to a seek or to
    /// exceptional situations like after emptying the buffer.
    ///
    /// The "base position" is the starting position, in playlist time in seconds, from which a
    /// `NextSegmentSelector` might want to look for segments. It is generally intended to be set to
    /// the current position.
    ///
    /// This method is NOT intended to be called when playback regularly advances, in which case you
    /// should call `advance_position` instead.
    ///
    /// The big difference between the two is that `advance_position` allows the
    /// `NextSegmentSelector`s to still rely on their respectively last returned segments to
    /// generally return the consecutive ones.
    /// When seeking or flushing buffers, you generally don't want to pick-up from the last returned
    /// segment, as the position might have completely changed. You want to restart from scratch
    /// instead.
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

/// The `NextSegmentSelector` may download a higher-quality version of a segment already in the
/// buffer. This cursor remembers where to resume looking for such segments.
struct SafeInspectionCursor {
    /// Variant whose segments would replace the buffered media.
    variant_id: u32,
    /// Context alongside that variant.
    context: SegmentQualityContext,
    /// Cutoff in playlist seconds: buffered chunks ending at or before it are no
    /// longer replacement candidates for this variant and context.
    inspected_until: f64,
    /// End of a replacement segment selected but not yet taken into account in `inspected_until`.
    pending_segment_end: Option<f64>,
}

impl SafeInspectionCursor {
    fn matches(&self, candidate: &SegmentSelectionCandidate<'_>) -> bool {
        self.variant_id == candidate.variant_id
            && self.context.media_id() == candidate.context.media_id()
            && self.context.variant_score() == candidate.context.variant_score()
    }
}

pub(crate) struct NextSegmentSelector {
    /// Interface allowing to keep track of which audio and video segments we need to load next.
    segment_cursor: SegmentCursor,

    /// Alternative cursor only used for segment replacement (of lower quality content to higher
    /// quality content).
    replacement_cursor: Option<SafeInspectionCursor>,

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

    /// `media_id` of the last segment pushed. Allows to determine when a quality switch
    /// occured, and to only check if some optimizations have to be performed, such as
    /// "fast quality switching", when the quality changes.
    last_media_id: Option<u32>,

    /// When `false`, previous state (e.g. `segment_cursor`) can be relied on to check for the
    /// segment to request.
    /// If `true`, we'll have to restart computing what the next position to request should be.
    needs_starting_position_recompute: bool,

    /// Information on segments that were voluntarily not returned by the `NextSegmentSelector`
    /// because "better" segments were already present in the buffer at its place.
    ///
    /// For example, let's say we're now loading 720p video segments. While iterating on the next
    /// chronological segment, we find out that a 1080p segment is already found for
    /// the same wanted positions. In such cases, that new 720p segment is skipped (it is not
    /// returned by the `NextSegmentSelector`) and its time information is added to this property.
    ///
    /// Because they were not part of the current `NextSegmentSelector` iteration, already buffered
    /// segments which led to the filling of that object may disappear from the buffer at any time,
    /// for example because a previous buffer cleaning operation to remove them was pending before
    /// and has now finished.
    /// To ensure that playback can still continue, segments that have been previously skipped
    /// should be re-checked regularly, if it is needed again, the segment should be loaded.
    skipped_segments: Vec<SegmentTimeInfo>,
}

impl NextSegmentSelector {
    /// Create a new `NextSegmentSelector`, which will start loading segments from `base_pos` (as a
    /// playlist time in seconds) for a single type of media and will stop loading segments if
    /// enough to fill the buffer until `base_pos + buffer_goal` are already loaded.
    ///
    /// As the current playback position advances, it is then recommended to update the base
    /// position (here indicated by `base_pos`) regularly by calling the `advance_position` method
    /// on the returned instance.
    ///
    /// If the position completely changes due to a seek, or if the buffer is emptied due to an
    /// exceptional event, the `restart_from_position` method should be called instead to prevent
    /// the `NextSegmentSelector`s from just continuing to provide the following segments.
    ///
    /// Likewise, the buffer goal (here indicated by `buffer_goal`), can be updated by calling the
    /// `update_buffer_goal` method.
    fn new(base_pos: f64, buffer_goal: f64) -> Self {
        let real_base_pos = f64::max(0., base_pos);
        Self {
            segment_cursor: SegmentCursor::new(base_pos),
            base_pos: real_base_pos,
            buffer_goal,
            last_media_id: None,
            needs_starting_position_recompute: true,
            replacement_cursor: None,
            init_status: InitializationSegmentSelectorStatus::Unchecked,
            skipped_segments: vec![],
        }
    }

    /// Reset the `NextSegmentSelector` state, as if no media segment nor init segment was
    /// returned by it, and start back from the given "base position".
    pub(crate) fn reset(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.init_status = InitializationSegmentSelectorStatus::Unchecked;
        self.last_media_id = None;
        self.needs_starting_position_recompute = true;
        self.replacement_cursor = None;
        self.segment_cursor = SegmentCursor::new(base_pos);
        self.skipped_segments.clear();
    }

    /// See `NextSegmentSelectors`'s `advance_position` method.
    pub(crate) fn advance_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);

        // If the new base position is further than our replacement cursor, we
        // can reset it.
        if let Some(cursor) = self.replacement_cursor.as_mut() {
            cursor.inspected_until = cursor.inspected_until.max(self.base_pos);
            if cursor
                .pending_segment_end
                .is_some_and(|end| end <= self.base_pos)
            {
                cursor.pending_segment_end = None;
            }
        }
        self.clean_skipped_segments();
    }

    /// See `NextSegmentSelectors`'s `restart_from_position` method.
    pub(crate) fn restart_from_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.segment_cursor = SegmentCursor::new(base_pos);
        self.needs_starting_position_recompute = true;
        if let Some(cursor) = self.replacement_cursor.as_mut() {
            cursor.inspected_until = self.base_pos;
            cursor.pending_segment_end = None;
        }
        self.skipped_segments.clear();
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

    /// Calling this method indicates that the media segment ending at `pos` with `context` was
    /// requested and no longer needs to be returned by this `NextSegmentSelector`.
    pub(crate) fn validate_media_until(&mut self, pos: f64, context: &SegmentQualityContext) {
        self.segment_cursor.move_cursor(pos);
        if let Some(cursor) = self.replacement_cursor.as_mut() {
            if cursor.pending_segment_end == Some(pos)
                && cursor.context.media_id() == context.media_id()
                && cursor.context.variant_score() == context.variant_score()
            {
                cursor.inspected_until = cursor.inspected_until.max(pos);
                cursor.pending_segment_end = None;
            }
        }
    }

    /// Returns the current most needed segment(s) according to the current situation and to the
    /// last "validated" init and media segment.
    ///
    /// `best` is used to extend the buffer; `safe` is used when replacing lower-quality media
    /// already in the buffer. The playlist to follow may differ from the variant of the segment
    /// returned now: an available best segment can be requested while safe's list is updated.
    ///
    /// Once returned, the segment objects returned by this method have to be "validated" if they
    /// do have been requested, to avoid just getting the same segment on the next
    /// `most_needed_segment` call.
    /// To "validate" a segment, you can call `validate_init` if we're talking about an
    /// initialization segment, or `validate_media_until` if we're talking about a media segment.
    pub(crate) fn most_needed_segment<'a>(
        &mut self,
        best: SegmentSelectionCandidate<'a>,
        safe: SegmentSelectionCandidate<'a>,
        inventory: &[BufferedChunk],
    ) -> MostNeededSegmentInfo<'a> {
        let (selected, replacement_segment, playlist_to_follow) =
            match self.find_safe_replacement(&safe, inventory) {
                SafeReplacement::Segment(segment) => (safe, Some(segment), PlaylistToFollow::Safe),
                SafeReplacement::NeedsUpdate => (best, None, PlaylistToFollow::Safe),
                SafeReplacement::None => (best, None, PlaylistToFollow::Best),
            };

        let Some(segment_list) = selected.playlist.usable() else {
            return MostNeededSegmentInfo {
                needed_segments: None,
                playlist_to_follow,
            };
        };

        let context = &selected.context;
        let selected_media_id = context.media_id();
        let previous_media_id = self.last_media_id;
        let has_quality_changed =
            previous_media_id.is_some() && previous_media_id != Some(selected_media_id);
        let should_recompute_starting_position =
            previous_media_id != Some(selected_media_id) || self.needs_starting_position_recompute;
        self.last_media_id = Some(selected_media_id);
        self.needs_starting_position_recompute = false;

        if should_recompute_starting_position {
            if has_quality_changed {
                log_debug!("Selector: Quality changed, recomputing starting position");
                self.init_status = InitializationSegmentSelectorStatus::Unchecked;
            } else if previous_media_id.is_none() {
                log_debug!("Selector: Initial media selection, computing starting position");
            } else {
                log_debug!("Selector: Recomputing starting position for the current media");
            }
            self.segment_cursor.move_cursor(self.base_pos);
            self.skipped_segments.clear();
            let start_pos = match replacement_segment {
                Some(segment) => segment.start(),
                None => self.recompute_starting_position(inventory),
            };
            self.segment_cursor.move_cursor(start_pos);
        }

        let most_needed_segment = if let Some(segment) = replacement_segment {
            segment
        } else {
            if let Some(val) = self.check_skipped_segments(context, inventory) {
                self.segment_cursor.move_cursor(val);
            }
            let Some(segment) = self.recursively_check_most_needed_media_segment(
                segment_list.media(),
                context,
                inventory,
            ) else {
                return MostNeededSegmentInfo {
                    needed_segments: None,
                    playlist_to_follow,
                };
            };
            segment
        };
        let init_segment = if let Some(i) = segment_list.init_for(most_needed_segment) {
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
            needed_segments: Some(NeededSegmentInfo {
                variant_id: selected.variant_id,
                media_segment: Some(most_needed_segment),
                init_segment,
            }),
            playlist_to_follow,
        }
    }

    /// Find the first safe segment that can replace buffered media not yet inspected with the
    /// current safe candidate. A cached live list can only rule out media before its first segment;
    /// its segments cannot be requested until the list is updated.
    fn find_safe_replacement<'a>(
        &mut self,
        safe: &SegmentSelectionCandidate<'a>,
        inventory: &[BufferedChunk],
    ) -> SafeReplacement<'a> {
        if !self
            .replacement_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.matches(safe))
        {
            self.replacement_cursor = Some(SafeInspectionCursor {
                variant_id: safe.variant_id,
                context: safe.context,
                inspected_until: self.base_pos,
                pending_segment_end: None,
            });
        }
        let cursor = self.replacement_cursor.as_mut().unwrap();

        let Some(first_index) = inventory
            .iter()
            .position(|chunk| chunk.playlist_end() > self.base_pos)
        else {
            return SafeReplacement::None;
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
            if chunk_end <= cursor.inspected_until {
                previous_end = chunk_end;
                continue;
            }

            let required_lead =
                MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS.max(chunk_end - chunk.playlist_start());
            let can_replace =
                chunk.is_worse_than(&safe.context) && previous_end - self.base_pos > required_lead;
            if can_replace {
                match &safe.playlist {
                    PlaylistView::NeedsUpdate { cached } => {
                        let cached_starts_after_chunk = cached
                            .and_then(|list| list.media().iter().find(|seg| seg.duration() > 0.))
                            .is_some_and(|seg| seg.start() >= chunk_end);
                        if !cached_starts_after_chunk {
                            return SafeReplacement::NeedsUpdate;
                        }
                    }
                    PlaylistView::Usable(list) => {
                        if let Some(segment) = list.media().iter().find(|seg| {
                            seg.duration() > 0.
                                && seg.end() > previous_end
                                && seg.end() > cursor.inspected_until
                                && seg.start() < chunk_end
                                && seg.start() <= maximum_position
                        }) {
                            cursor.pending_segment_end = Some(segment.end());
                            return SafeReplacement::Segment(segment);
                        }
                    }
                }
            }

            cursor.inspected_until = chunk_end;
            previous_end = chunk_end;
        }
        SafeReplacement::None
    }

    /// Starts from `self.base_pos`, look at what is already buffered, and determine a new optimal
    /// starting point for segments of the given quality.
    ///
    /// Safe replacements have their own explicitly selected segment; this position is used for
    /// ordinary buffer extension.
    fn recompute_starting_position(&self, inventory: &[BufferedChunk]) -> f64 {
        let Some(mut curr_idx) = inventory
            .iter()
            .position(|segment| segment.playlist_end() > self.base_pos)
        else {
            log_debug!(
                "Selector: Starting position at base position: {}",
                self.base_pos
            );
            return self.base_pos;
        };
        let mut prev_end = self.base_pos;
        while let Some(segment) = inventory.get(curr_idx) {
            if segment.playlist_start() > (prev_end + 0.001)
                || segment.appears_garbage_collected(prev_end)
            {
                log_debug!(
                    "Selector: Segment non-contiguous or GCed starting from {}",
                    prev_end
                );
                return prev_end;
            }
            prev_end = segment.playlist_end();
            curr_idx += 1;
        }
        log_debug!("Selector: Starting position after inventory: {}", prev_end);
        prev_end
    }

    /// Check that all elements in `self.skipped_segments` can still be skipped
    /// (there is non-garbage collected segments of better or equal quality to the given
    /// context in the current buffer).
    ///
    /// If not, remove segment from `self.skipped_segments` and return its starting position.
    fn check_skipped_segments(
        &mut self,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> Option<f64> {
        for (seg_index, seg) in self.skipped_segments.iter().enumerate() {
            let seg_start = seg.start();
            if !self.can_be_skipped(seg_start, seg.end(), context, inventory) {
                log_debug!(
                    "Selector: Skipped segment can no longer be skipped (s:{})",
                    seg_start
                );
                self.skipped_segments.remove(seg_index);
                return Some(seg_start);
            }
        }
        None
    }

    /// Returns the most needed segment according to the current situation.
    /// Internally, this method may be validating and re-calling itself (hence its name) if it sees
    /// that segments of a higher or similar quality are already present in the buffer, through a
    /// project-specific optimization called "smart quality switching": skipping a request when
    /// equal- or higher-quality media already covers its range.
    fn recursively_check_most_needed_media_segment<'a>(
        &mut self,
        media_segments: &'a [MediaSegmentInfo],
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> Option<&'a MediaSegmentInfo> {
        let maximum_position = self.buffer_goal + self.base_pos;
        let si = self
            .segment_cursor
            .get_next(media_segments, maximum_position)?;
        let segment_end = si.end();

        // Apply the project-specific "smart quality switching" optimization: skip the request
        // when equal- or higher-quality media is already buffered for its range.
        if self.can_be_skipped(si.start(), segment_end, context, inventory) {
            log_debug!(
                "Selector: Segment can be skipped (s:{}, d: {})",
                si.start(),
                si.duration()
            );
            let skipped = SegmentTimeInfo::new(si.start(), si.duration());
            match self
                .skipped_segments
                .iter()
                .position(|sk| sk.start() > si.start())
            {
                Some(pos) => self.skipped_segments.insert(pos, skipped),
                None => self.skipped_segments.push(skipped),
            }
            self.segment_cursor.move_cursor(segment_end);
            self.recursively_check_most_needed_media_segment(media_segments, context, inventory)
        } else {
            Some(si)
        }
    }

    /// Returns true if either a segment or a range of segments can be skipped, by communicating
    /// its start and end, context about its quality, and the inventory of already buffered
    /// segment.
    /// If `true`, this generally means that the wanted segment or segment ranges is currently
    /// unneeded.
    ///
    /// This method applies the project-specific "smart quality switching" optimization: skip
    /// downloads when equal- or higher-quality media is already buffered for the requested range.
    fn can_be_skipped(
        &self,
        start: f64,
        end: f64,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> bool {
        let first_seg_pos = inventory.iter().position(|s| s.playlist_end() > start);
        if let Some(mut curr_idx) = first_seg_pos {
            let mut prev_seg_end = start;
            while let Some(mut curr_seg) = inventory.get(curr_idx) {
                if curr_seg.appears_garbage_collected(self.base_pos)
                    || curr_seg.is_worse_than(context)
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

    /// To call regularly as `self.base_pos` changes to clear the `self.skipped_segments` the
    /// elements behind that position, as they now become unneeded.
    fn clean_skipped_segments(&mut self) {
        // remove everything before first skipped segment still concerned by `base_pos`
        match self
            .skipped_segments
            .iter()
            .position(|r| r.end() > self.base_pos)
        {
            None => self.skipped_segments.clear(),
            Some(0) => {}
            Some(x) => {
                self.skipped_segments.drain(0..x);
            }
        }
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

/// One variant the selector may use, with the status of its media playlist.
pub(crate) struct SegmentSelectionCandidate<'a> {
    pub(crate) variant_id: u32,
    pub(crate) context: SegmentQualityContext,
    pub(crate) playlist: PlaylistView<'a>,
}

/// Whether a candidate's listed segments can be requested now.
pub(crate) enum PlaylistView<'a> {
    /// Listed segments may be requested. A live list can still receive normal later refreshes.
    Usable(&'a SegmentList),
    /// Fetch or refresh the list before requesting segments from this candidate. An old live list
    /// may be supplied only to rule out replacement positions before its first segment.
    NeedsUpdate { cached: Option<&'a SegmentList> },
}

impl<'a> PlaylistView<'a> {
    fn usable(&self) -> Option<&'a SegmentList> {
        match self {
            Self::Usable(list) => Some(list),
            Self::NeedsUpdate { .. } => None,
        }
    }
}

/// Which candidate's media playlist should be followed for this media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaylistToFollow {
    Best,
    Safe,
}

enum SafeReplacement<'a> {
    None,
    NeedsUpdate,
    Segment(&'a MediaSegmentInfo),
}

pub(crate) struct MostNeededSegmentInfo<'a> {
    /// Segments to request now. They may come from best while safe's playlist is updated.
    needed_segments: Option<NeededSegmentInfo<'a>>,
    /// The candidate whose playlist should be fetched or normally refreshed. This can differ from
    /// the variant of `needed_segments`.
    playlist_to_follow: PlaylistToFollow,
}

impl<'a> MostNeededSegmentInfo<'a> {
    pub(crate) fn needed_segments(&self) -> Option<&NeededSegmentInfo<'a>> {
        self.needed_segments.as_ref()
    }

    pub(crate) fn playlist_to_follow(&self) -> PlaylistToFollow {
        self.playlist_to_follow
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
    /// The media segment that should now be needed, corresponding to the inner information.
    ///
    /// `None` if no media segment is currently needed.
    media_segment: Option<&'a MediaSegmentInfo>,
}

impl<'a> NeededSegmentInfo<'a> {
    pub(crate) fn variant_id(&self) -> u32 {
        self.variant_id
    }

    /// Returns initialization segment that should be loaded.
    ///
    /// `None` either if there's no needed initialization segment or if we consider that the last
    /// validated one is still compatible.
    pub(crate) fn media_segment(&self) -> Option<&MediaSegmentInfo> {
        self.media_segment
    }

    /// Returns media segment that should be loaded, corresponding to the inner information.
    ///
    /// `None` if no media segment is currently needed.
    pub(crate) fn init_segment(&self) -> Option<&InitSegmentInfo> {
        self.init_segment
    }
}

/// Inner `NextSegmentSelector` mechanism allowing to keep track of until which segment we have
/// validation for now.
///
/// This allows to return the next consecutive segment, which is something you generally want to do
/// under regular playback.
///
/// An alternative would be having to re-determine the next segment each time based on buffer
/// inspection which would also have its own risks due to browsers being not perfect.
#[derive(Clone, Debug)]
pub(crate) struct SegmentCursor {
    /// The `SegmentCursor` will return as next segment the segment ending after this position.
    ///
    /// Can be set to the initially wanted position initially.
    /// Then, can be updated to the end playlist time of the last returned `MediaSegmentInfo`'s.
    current_cursor: f64,
}

impl SegmentCursor {
    /// Create a new `SegmentCursor` which will start from the first segment ending after
    /// `initial_pos`.
    pub(crate) fn new(initial_pos: f64) -> Self {
        Self {
            current_cursor: initial_pos,
        }
    }

    /// move_cursor the cursor at `pos`, so its next segment is the one ending after it.
    pub(crate) fn move_cursor(&mut self, pos: f64) {
        self.current_cursor = pos;
    }

    /// Get the first chronological segment in `media_segments` that ends after the cursor's
    /// position, unless it also starts at or after the given `maximum_position`.
    ///
    /// Returns `None` if no segment in `media_segments` respect those conditions.
    pub(crate) fn get_next<'a>(
        &mut self,
        media_segments: &'a [MediaSegmentInfo],
        maximum_position: f64,
    ) -> Option<&'a MediaSegmentInfo> {
        let position = self.current_cursor;
        let next_seg = media_segments
            .iter()
            .find(|s| s.duration() > 0. && (s.end()) > position);
        match next_seg {
            Some(seg) if seg.start() <= maximum_position => next_seg,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NextSegmentSelector, NextSegmentSelectors, PlaylistToFollow, PlaylistView, SegmentCursor,
        SegmentSelectionCandidate,
    };
    use crate::{
        bindings::MediaType,
        media_element::{BufferedChunk, SegmentQualityContext},
        parser::{SegmentList, TopLevelPlaylist},
        utils::url::Url,
    };

    fn candidate<'a>(
        list: &'a SegmentList,
        context: &SegmentQualityContext,
    ) -> SegmentSelectionCandidate<'a> {
        SegmentSelectionCandidate {
            variant_id: context.media_id(),
            context: *context,
            playlist: PlaylistView::Usable(list),
        }
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
        let best = SegmentQualityContext::new(3., 3);
        let safe = SegmentQualityContext::new(2., 2);
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);

        let replacement = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 30,
                context: best,
                playlist: PlaylistView::Usable(list),
            },
            SegmentSelectionCandidate {
                variant_id: 20,
                context: safe,
                playlist: PlaylistView::Usable(list),
            },
            &inventory,
        );
        assert_eq!(replacement.playlist_to_follow, PlaylistToFollow::Safe);
        let replacement = replacement.needed_segments.unwrap();
        assert_eq!(replacement.variant_id, 20);
        assert_eq!(replacement.media_segment.unwrap().start(), 8.);

        let repeated = selector.most_needed_segment(
            candidate(list, &best),
            candidate(list, &safe),
            &inventory,
        );
        assert_eq!(
            repeated
                .needed_segments
                .unwrap()
                .media_segment
                .unwrap()
                .start(),
            8.
        );

        selector.validate_media_until(replacement.media_segment.unwrap().end(), &safe);
        let next = selector.most_needed_segment(
            candidate(list, &best),
            candidate(list, &safe),
            &inventory,
        );
        assert_eq!(
            next.needed_segments.unwrap().media_segment.unwrap().start(),
            12.
        );

        selector.restart_from_position(0.);
        let addition = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 30,
                context: best,
                playlist: PlaylistView::Usable(list),
            },
            SegmentSelectionCandidate {
                variant_id: 20,
                context: safe,
                playlist: PlaylistView::Usable(list),
            },
            &[],
        );
        let addition = addition.needed_segments.unwrap();
        assert_eq!(addition.variant_id, 30);
        assert_eq!(addition.media_segment.unwrap().start(), 0.);
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
        let best = SegmentQualityContext::new(3., 3);
        let safe = SegmentQualityContext::new(2., 2);
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);

        let while_missing = selector.most_needed_segment(
            candidate(list, &best),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: safe,
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            &inventory,
        );
        assert_eq!(while_missing.playlist_to_follow(), PlaylistToFollow::Safe);
        let best_segment = while_missing.needed_segments().unwrap();
        assert_eq!(best_segment.variant_id(), 3);
        assert_eq!(best_segment.media_segment().unwrap().start(), 24.);

        selector.buffer_goal = 20.;
        selector.restart_from_position(0.);
        let while_full = selector.most_needed_segment(
            candidate(list, &best),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: safe,
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            &inventory,
        );
        assert_eq!(while_full.playlist_to_follow(), PlaylistToFollow::Safe);
        assert!(while_full.needed_segments().is_none());

        selector.buffer_goal = 40.;

        let after_load = selector.most_needed_segment(
            candidate(list, &best),
            SegmentSelectionCandidate {
                variant_id: 20,
                context: safe,
                playlist: PlaylistView::Usable(list),
            },
            &inventory,
        );
        assert_eq!(after_load.playlist_to_follow(), PlaylistToFollow::Safe);
        let safe_segment = after_load.needed_segments().unwrap();
        assert_eq!(safe_segment.variant_id(), 20);
        assert_eq!(safe_segment.media_segment().unwrap().start(), 8.);
    }

    #[test]
    fn inspects_a_fresh_safe_list_once_before_following_best_again() {
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
        let best = SegmentQualityContext::new(3., 3);
        let safe = SegmentQualityContext::new(2., 2);
        let mut inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 48.);

        let missing = selector.most_needed_segment(
            candidate(best_list, &best),
            SegmentSelectionCandidate {
                variant_id: 2,
                context: safe,
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            &inventory,
        );
        assert_eq!(missing.playlist_to_follow(), PlaylistToFollow::Safe);
        assert_eq!(missing.needed_segments().unwrap().variant_id(), 3);

        let inspected = selector.most_needed_segment(
            candidate(best_list, &best),
            candidate(safe_list, &safe),
            &inventory,
        );
        assert_eq!(inspected.playlist_to_follow(), PlaylistToFollow::Best);
        assert_eq!(
            selector
                .replacement_cursor
                .as_ref()
                .unwrap()
                .inspected_until,
            24.
        );

        let repeated = selector.most_needed_segment(
            candidate(best_list, &best),
            SegmentSelectionCandidate {
                variant_id: 2,
                context: safe,
                playlist: PlaylistView::NeedsUpdate {
                    cached: Some(safe_list),
                },
            },
            &inventory,
        );
        assert_eq!(repeated.playlist_to_follow(), PlaylistToFollow::Best);

        inventory.extend((6..8).map(|number| {
            BufferedChunk::new_for_test(
                number as f64 * 4.,
                (number + 1) as f64 * 4.,
                SegmentQualityContext::new(1., 1),
            )
        }));
        let new_opportunity = selector.most_needed_segment(
            candidate(best_list, &best),
            SegmentSelectionCandidate {
                variant_id: 2,
                context: safe,
                playlist: PlaylistView::NeedsUpdate {
                    cached: Some(safe_list),
                },
            },
            &inventory,
        );
        assert_eq!(new_opportunity.playlist_to_follow(), PlaylistToFollow::Safe);

        let replacement = selector.most_needed_segment(
            candidate(best_list, &best),
            candidate(safe_list, &safe),
            &inventory,
        );
        assert_eq!(replacement.playlist_to_follow(), PlaylistToFollow::Safe);
        assert_eq!(
            replacement
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
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
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        let mut selector = NextSegmentSelector::new(0., 40.);
        let result = selector.most_needed_segment(
            candidate(
                best_playlist.playlist().segment_list(),
                &SegmentQualityContext::new(3., 3),
            ),
            candidate(
                safe_playlist.playlist().segment_list(),
                &SegmentQualityContext::new(2., 2),
            ),
            &inventory,
        );
        assert_eq!(result.playlist_to_follow(), PlaylistToFollow::Safe);
        assert_eq!(
            result
                .needed_segments()
                .unwrap()
                .media_segment()
                .unwrap()
                .start(),
            16.
        );
    }

    #[test]
    fn requests_selected_playlist_before_advancing_cursor() {
        let mut selector = NextSegmentSelector::new(0., 40.);
        let safe = SegmentQualityContext::new(2., 2);
        let best = SegmentQualityContext::new(3., 3);
        let missing = |context: &SegmentQualityContext| SegmentSelectionCandidate {
            variant_id: context.media_id(),
            context: *context,
            playlist: PlaylistView::NeedsUpdate { cached: None },
        };
        let inventory = vec![
            BufferedChunk::new_for_test(0., 6., SegmentQualityContext::new(1., 1)),
            BufferedChunk::new_for_test(6., 10., SegmentQualityContext::new(1., 1)),
        ];

        let replacement = selector.most_needed_segment(missing(&best), missing(&safe), &inventory);
        assert_eq!(replacement.playlist_to_follow, PlaylistToFollow::Safe);
        assert!(replacement.needed_segments.is_none());
        assert_eq!(selector.segment_cursor.current_cursor, 0.);

        let addition = selector.most_needed_segment(missing(&best), missing(&safe), &[]);
        assert_eq!(addition.playlist_to_follow, PlaylistToFollow::Best);
        assert!(addition.needed_segments.is_none());
        assert_eq!(selector.segment_cursor.current_cursor, 0.);
    }

    #[test]
    fn cursor_restart_reconsiders_buffered_coverage_and_preserves_media_identity() {
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
            (true, 4., 12., false),
            (false, 30., 28., false),
            (false, 4., 24., true),
        ] {
            let context = SegmentQualityContext::new(2., 2);
            let inventory: Vec<_> = (0..6)
                .map(|number| {
                    BufferedChunk::new_for_test(
                        number as f64 * 4.,
                        (number + 1) as f64 * 4.,
                        if has_replacement {
                            SegmentQualityContext::new(1., 1)
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
                )
                .needed_segments
                .unwrap();
            selector.validate_init(first.init_segment.unwrap().id());
            selector.validate_media_until(first.media_segment.unwrap().end(), &context);

            selector.restart_from_position(seek_position - 0.2);
            let context = SegmentQualityContext::new(2., if change_media { 3 } else { 2 });
            let next = selector
                .most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &inventory,
                )
                .needed_segments
                .unwrap();
            assert_eq!(next.media_segment.unwrap().start(), expected_start);
            assert_eq!(next.init_segment.is_some(), change_media);
            selector.validate_media_until(next.media_segment.unwrap().end(), &context);

            let following = selector
                .most_needed_segment(
                    candidate(list, &context),
                    candidate(list, &context),
                    &inventory,
                )
                .needed_segments
                .unwrap();
            assert_eq!(
                following.media_segment.unwrap().start(),
                expected_start + 4.
            );
        }
    }

    #[test]
    fn above_throughput_switch_can_continue_after_buffer_without_replacement() {
        let mut selector = NextSegmentSelector::new(0., 30.);
        let buffered = vec![
            BufferedChunk::new_for_test(0., 6., SegmentQualityContext::new(1., 1)),
            BufferedChunk::new_for_test(6., 10., SegmentQualityContext::new(1., 1)),
        ];
        let higher_quality = SegmentQualityContext::new(2., 2);

        let result = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 3,
                context: SegmentQualityContext::new(3., 3),
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            SegmentSelectionCandidate {
                variant_id: 2,
                context: higher_quality,
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            &buffered,
        );
        assert_eq!(result.playlist_to_follow(), PlaylistToFollow::Safe);
        assert_eq!(selector.recompute_starting_position(&buffered), 10.);
    }

    #[test]
    fn fast_quality_switch_requires_the_minimum_lead() {
        let mut selector = NextSegmentSelector::new(0., 30.);
        let buffered = vec![
            BufferedChunk::new_for_test(0., 5., SegmentQualityContext::new(1., 1)),
            BufferedChunk::new_for_test(5., 9., SegmentQualityContext::new(1., 1)),
        ];
        let higher_quality = SegmentQualityContext::new(2., 2);

        let result = selector.most_needed_segment(
            SegmentSelectionCandidate {
                variant_id: 3,
                context: SegmentQualityContext::new(3., 3),
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            SegmentSelectionCandidate {
                variant_id: 2,
                context: higher_quality,
                playlist: PlaylistView::NeedsUpdate { cached: None },
            },
            &buffered,
        );
        assert_eq!(result.playlist_to_follow(), PlaylistToFollow::Best);
        assert_eq!(selector.recompute_starting_position(&buffered), 9.);
    }

    #[test]
    fn segment_cursor_skips_trailing_zero_duration_segment() {
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
        let segments = playlist.playlist().segment_list().media();
        let mut cursor = SegmentCursor::new(2.5);

        let next = cursor.get_next(segments, 10.).unwrap();
        assert_eq!(next.sequence(), 0);

        cursor.move_cursor(3.);
        assert!(cursor.get_next(segments, 10.).is_none());
    }
}
