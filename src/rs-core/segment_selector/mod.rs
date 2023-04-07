use crate::{
    bindings::MediaType,
    media_element::{BufferedChunk, SegmentQualityContext},
    parser::{InitSegmentInfo, SegmentInfo, SegmentList, SegmentTimeInfo},
    Logger,
};

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
    /// position (here indicated by `base_pos`) regularly by calling the `advance_position` method
    /// on the returned instance.
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

pub(crate) struct NextSegmentSelector {
    /// Interface allowing to keep track of which audio and video segments we need to load next.
    segment_cursor: SegmentCursor,

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
    /// "fast-switching", when the
    /// quality change.
    last_media_id: Option<u32>,

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
        self.segment_cursor = SegmentCursor::new(base_pos);
        self.skipped_segments.clear();
    }

    /// See `NextSegmentSelectors`'s `advance_position` method.
    pub(crate) fn advance_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.clean_skipped_segments();
    }

    /// See `NextSegmentSelectors`'s `restart_from_position` method.
    pub(crate) fn restart_from_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.segment_cursor = SegmentCursor::new(base_pos);
        self.skipped_segments.clear();
    }

    /// Calling this method allows to indicate that the initialization segment was requested and as
    /// such, don't need to be returned anymore by this `NextSegmentSelector`.
    pub(crate) fn validate_init(&mut self) {
        if let InitializationSegmentSelectorStatus::Unvalidated(start) = self.init_status {
            self.init_status = InitializationSegmentSelectorStatus::Validated(start);
        } else {
            Logger::warn("Validation an initialization segment, but none were returned.");
        }
    }

    /// Calling this method allows to indicate that the media segment ending at `pos` was requested
    /// and as such, don't need to be returned anymore by this `NextSegmentSelector`.
    pub(crate) fn validate_media_until(&mut self, pos: f64) {
        self.segment_cursor.move_cursor(pos);
    }

    /// Returns the current most needed segment(s) according to the current situation and to the
    /// last "validated" init and media segment.
    ///
    /// Once returned, the segment objects returned by this method have to be "validated" if they
    /// do have been requested, to avoid just getting the same segment on the next
    /// `most_needed_segment` call.
    /// To "validate" a segment, you can call `validate_init` if we're talking about an
    /// initialization segment, or `validate_media_until` if we're talking about a media segment.
    pub(crate) fn most_needed_segment<'a>(
        &mut self,
        segment_list: &'a SegmentList,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> NeededSegmentInfo<'a> {
        let new_media_id = context.media_id();
        let has_quality_changed = Some(new_media_id) != self.last_media_id;
        self.last_media_id = Some(new_media_id);

        if has_quality_changed {
            Logger::debug("Selector: Quality changed, recomputing starting position");
            self.init_status = InitializationSegmentSelectorStatus::Unchecked;
            self.segment_cursor.move_cursor(self.base_pos);
            self.skipped_segments.clear();
            let start_pos = self.recompute_starting_position(context, inventory);
            self.segment_cursor.move_cursor(start_pos);
        }

        if let Some(val) = self.check_skipped_segments(context, inventory) {
            self.segment_cursor.move_cursor(val);
        }
        let most_needed_segment = if let Some(seg) = self
            .recursively_check_most_needed_media_segment(segment_list.media(), context, inventory)
        {
            seg
        } else {
            return NeededSegmentInfo {
                init_segment: None,
                media_segment: None,
            };
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
        NeededSegmentInfo {
            media_segment: Some(most_needed_segment),
            init_segment,
        }
    }

    /// Starts from `self.base_pos`, look at what is already buffered, and determine a new optimal
    /// starting point for segments of the given quality.
    ///
    /// Note that the quality has an influence here because of "fast-switching" which is the concept
    /// of replacing segments of a poor quality by segments of a higher quality. If segments of a
    /// poorer quality is detected in the currently buffered `inventory`, the returned f64 might
    /// thus be earlier than in the opposite case.
    fn recompute_starting_position(
        &self,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> f64 {
        let inv_start = inventory
            .iter()
            .position(|s| s.playlist_end() > self.base_pos);
        if let Some(mut curr_idx) = inv_start {
            let mut prev_end = self.base_pos;
            while let Some(seg_i) = inventory.get(curr_idx) {
                if seg_i.playlist_start() > (prev_end + 0.001)
                    || seg_i.appears_garbage_collected(prev_end)
                {
                    // Either not contiguous to the previous segment, or garbage collected.
                    // Start loading from there.
                    Logger::debug(&format!(
                        "Selector: Segment non-contiguous or GCed starting from {}",
                        prev_end
                    ));
                    return prev_end;
                }
                if seg_i.is_worse_than(context) {
                    // We found a segment of worse quality, we can replace it, unless it is
                    // ending soon, to avoid rebuffering.
                    let next_seg_duration = inventory
                        .iter()
                        .find(|s| s.playlist_end() > seg_i.playlist_end())
                        .map(|s| s.playlist_end() - s.playlist_start())
                        .unwrap_or(5.);
                    if seg_i.last_buffered_end() - self.base_pos > next_seg_duration {
                        Logger::debug(&format!("Selector: Fast switching from {prev_end}"));
                        return prev_end;
                    }
                }
                prev_end = seg_i.playlist_end();
                curr_idx += 1;
            }
            Logger::debug(&format!(
                "Selector: Starting position after inventory: {prev_end}"
            ));
            prev_end
        } else {
            Logger::debug(&format!(
                "Selector: Starting position at base position: {}",
                self.base_pos
            ));
            self.base_pos
        }
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
                Logger::debug(&format!(
                    "Selector: Skipped segment can no longer be skipped (s:{})",
                    seg_start
                ));
                self.skipped_segments.remove(seg_index);
                return Some(seg_start);
            }
        }
        None
    }

    /// Returns the most needed segment according to the current situation.
    /// Internally, this method may be validating and re-calling itself (hence its name) if it sees
    /// that segments of a higher or similar quality are already present in the buffer, through a
    /// process we here call "smart-switching".
    fn recursively_check_most_needed_media_segment<'a>(
        &mut self,
        media_segments: &'a [SegmentInfo],
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> Option<&'a SegmentInfo> {
        let maximum_position = self.buffer_goal + self.base_pos;
        let si = self
            .segment_cursor
            .get_next(media_segments, maximum_position)?;
        let segment_end = si.end();

        // Check for "smart-switching", which is to avoid returning segments who have
        // already an equal or even better quality in the buffer.
        if self.can_be_skipped(si.start(), segment_end, context, inventory) {
            Logger::debug(&format!(
                "Selector: Segment can be skipped (s:{}, d: {})",
                si.start(),
                si.duration()
            ));
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
    /// This method is part of what we call the "smart-switching" algorithm, which allows to avoid
    /// downloading segments when some of a higher or similar quality are already present in the
    /// buffer.
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

/// Segment information for segments that may now be loaded as returned by the
/// `NextSegmentSelector`.
///
/// Its lifetime is generally linked to the `MediaPlaylist` to which those information are
/// initially linked to.
pub(crate) struct NeededSegmentInfo<'a> {
    /// The initialization segment that should now be needed, corresponding to the inner
    /// information.
    ///
    /// `None` either if there's no needed initialization segment or if we consider that the last
    /// validated one is still compatible.
    init_segment: Option<&'a InitSegmentInfo>,
    /// The media segment that should now be needed, corresponding to the inner information.
    ///
    /// `None` if no media segment is currently needed.
    media_segment: Option<&'a SegmentInfo>,
}

impl<'a> NeededSegmentInfo<'a> {
    /// Returns initialization segment that should be loaded.
    ///
    /// `None` either if there's no needed initialization segment or if we consider that the last
    /// validated one is still compatible.
    pub(crate) fn media_segment(&self) -> Option<&SegmentInfo> {
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
    /// Then, can be updated to the end playlist time of the last returned `SegmentInfo`'s.
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
        media_segments: &'a [SegmentInfo],
        maximum_position: f64,
    ) -> Option<&'a SegmentInfo> {
        let position = self.current_cursor;
        let next_seg = media_segments.iter().find(|s| (s.end()) > position);
        match next_seg {
            Some(seg) if seg.start() <= maximum_position => next_seg,
            _ => None,
        }
    }
}
