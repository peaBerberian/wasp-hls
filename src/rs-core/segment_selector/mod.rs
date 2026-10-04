use crate::{
    bindings::MediaType,
    media_element::{BufferedChunk, SegmentQualityContext},
    parser::{InitSegmentInfo, MediaPlaylist, MediaSegmentInfo, SegmentList, SegmentTimeInfo},
    playlist_store::MediaPlaylistPermanentId,
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
    /// "fast quality switching", when the quality changes.
    last_media_id: Option<u32>,

    /// Whether a quality change may replace already-buffered lower-quality segments.
    ///
    /// Set for the chosen request source: additions must not revisit buffered lower quality.
    allow_fast_quality_switching: bool,

    /// Permission used when the current segment cursor was last considered. A change must
    /// reconsider replacement opportunities even when the selected playlist stays the same.
    /// `None` means the cursor restarted and buffered coverage must be reconsidered.
    last_fast_quality_switching: Option<bool>,

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

/// Which recommendation should supply the next request for one media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SegmentSourceChoice {
    Additive,
    Replacement,
}

/// One recommendation and the playlist currently available for using it.
/// A missing playlist means it needs preparation before segments can be selected.
#[derive(Clone)]
pub(crate) struct SegmentSource<'a> {
    pub(crate) playlist_id: MediaPlaylistPermanentId,
    pub(crate) quality_context: SegmentQualityContext,
    pub(crate) playlist: Option<&'a MediaPlaylist>,
}

pub(crate) enum SelectedSegment {
    Init(InitSegmentInfo),
    Media {
        segment: MediaSegmentInfo,
        init_segment_id: Option<f64>,
    },
}

pub(crate) struct SelectedSegmentRequest {
    pub(crate) playlist_id: MediaPlaylistPermanentId,
    pub(crate) quality_context: SegmentQualityContext,
    pub(crate) segment: SelectedSegment,
}

pub(crate) struct NextSegmentSelection {
    /// Sources to load or keep refreshed, including preparation for subsequent additions.
    pub(crate) needed_playlists: Vec<MediaPlaylistPermanentId>,
    pub(crate) request: Option<SelectedSegmentRequest>,
}

impl NextSegmentSelector {
    /// Choose both the source and segment. When a request is already pending, only emit
    /// preparation hints: changing the cursor then would interfere with init validation.
    pub(crate) fn select_next_segment(
        &mut self,
        best: &SegmentSource<'_>,
        safe: Option<&SegmentSource<'_>>,
        inventory: &[BufferedChunk],
        request_pending: bool,
    ) -> NextSegmentSelection {
        let additive_end = best
            .playlist
            .filter(|playlist| playlist.is_ended())
            .and_then(|playlist| playlist.segment_list().media().last())
            .map(|segment| segment.end());
        let Some(choice) = self.next_request_role(
            safe.map(|source| &source.quality_context),
            inventory,
            additive_end,
        ) else {
            return NextSegmentSelection {
                needed_playlists: Vec::new(),
                request: None,
            };
        };
        let mut needed_playlists = Vec::with_capacity(2);
        // Prepare subsequent additions even when the next request is a replacement.
        needed_playlists.push(best.playlist_id);
        let replacement = choice == SegmentSourceChoice::Replacement;
        if replacement || best.playlist.is_none() {
            if let Some(safe) = safe {
                if !needed_playlists.contains(&safe.playlist_id) {
                    needed_playlists.push(safe.playlist_id);
                }
            }
        }
        let source = if replacement {
            safe
        } else if best.playlist.is_some() {
            Some(best)
        } else {
            safe
        };
        let request = if request_pending {
            None
        } else {
            source.and_then(|source| {
                let playlist = source.playlist?;
                self.allow_fast_quality_switching = replacement;
                let needed = self.most_needed_segment(
                    playlist.segment_list(),
                    &source.quality_context,
                    inventory,
                );
                let segment = if let Some(init) = needed.init_segment() {
                    SelectedSegment::Init(init.clone())
                } else {
                    let segment = needed.media_segment()?;
                    SelectedSegment::Media {
                        init_segment_id: playlist
                            .segment_list()
                            .init_for(segment)
                            .map(|init| init.id()),
                        segment: segment.clone(),
                    }
                };
                Some(SelectedSegmentRequest {
                    playlist_id: source.playlist_id,
                    quality_context: source.quality_context.clone(),
                    segment,
                })
            })
        };
        NextSegmentSelection {
            needed_playlists,
            request,
        }
    }

    /// Prioritize extensions at low buffer, otherwise consider optional replacements.
    fn next_request_role(
        &self,
        replacement: Option<&SegmentQualityContext>,
        inventory: &[BufferedChunk],
        additive_end: Option<f64>,
    ) -> Option<SegmentSourceChoice> {
        let mut buffered_until = self.base_pos;
        for segment in inventory
            .iter()
            .filter(|segment| segment.playlist_end() > self.base_pos)
        {
            if segment.playlist_start() > buffered_until + 0.001
                || segment.appears_garbage_collected(buffered_until)
            {
                break;
            }
            buffered_until = buffered_until.max(segment.playlist_end());
        }
        let wanted_end = additive_end.map_or(self.base_pos + self.buffer_goal, |end| {
            end.min(self.base_pos + self.buffer_goal)
        });
        let needs_addition = buffered_until < wanted_end;
        if needs_addition && buffered_until - self.base_pos <= MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS
        {
            Some(SegmentSourceChoice::Additive)
        } else if replacement
            .and_then(|context| self.fast_quality_switch_position(context, inventory))
            .is_some()
        {
            Some(SegmentSourceChoice::Replacement)
        } else if needs_addition {
            Some(SegmentSourceChoice::Additive)
        } else {
            None
        }
    }

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
            allow_fast_quality_switching: true,
            last_fast_quality_switching: Some(true),
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
        self.allow_fast_quality_switching = true;
        self.last_fast_quality_switching = Some(true);
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
        self.last_fast_quality_switching = None;
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
    fn most_needed_segment<'a>(
        &mut self,
        segment_list: &'a SegmentList,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> NeededSegmentInfo<'a> {
        let new_media_id = context.media_id();
        let previous_media_id = self.last_media_id;
        let has_quality_changed =
            previous_media_id.is_some() && previous_media_id != Some(new_media_id);
        let should_recompute_starting_position = previous_media_id != Some(new_media_id)
            || self.last_fast_quality_switching != Some(self.allow_fast_quality_switching);
        self.last_media_id = Some(new_media_id);
        self.last_fast_quality_switching = Some(self.allow_fast_quality_switching);

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
    /// Note that quality has an influence here because "fast quality switching" replaces
    /// lower-quality buffered segments with higher-quality ones. If a lower-quality segment is
    /// detected in `inventory`, the returned position may therefore move backwards.
    fn recompute_starting_position(
        &self,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> f64 {
        if self.allow_fast_quality_switching {
            if let Some(position) = self.fast_quality_switch_position(context, inventory) {
                log_debug!("Selector: Fast quality switching from {}", position);
                return position;
            }
        }

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

    /// Returns if found a candidate position that we could re-load (moving the cursor backward)
    /// if it would mean buffering higher quality data (by basing us on `context` for the new
    /// quality).
    ///
    /// Getting a value basically means that "fast quality switching" is possible here, and
    /// indicates where.
    fn fast_quality_switch_position(
        &self,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> Option<f64> {
        let mut curr_idx = inventory
            .iter()
            .position(|segment| segment.playlist_end() > self.base_pos)?;
        let mut prev_end = self.base_pos;
        while let Some(segment) = inventory.get(curr_idx) {
            if segment.playlist_start() > (prev_end + 0.001)
                || segment.appears_garbage_collected(prev_end)
            {
                return None;
            }
            let replacement_lead = prev_end - self.base_pos;
            let segment_duration = segment.playlist_end() - segment.playlist_start();
            let required_lead = MIN_FAST_QUALITY_SWITCH_LEAD_SECONDS.max(segment_duration);
            if segment.is_worse_than(context) && replacement_lead > required_lead {
                return Some(prev_end);
            }
            prev_end = segment.playlist_end();
            curr_idx += 1;
        }
        None
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
                    || (self.allow_fast_quality_switching && curr_seg.is_worse_than(context))
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
struct NeededSegmentInfo<'a> {
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
    /// Returns initialization segment that should be loaded.
    ///
    /// `None` either if there's no needed initialization segment or if we consider that the last
    /// validated one is still compatible.
    fn media_segment(&self) -> Option<&MediaSegmentInfo> {
        self.media_segment
    }

    /// Returns media segment that should be loaded, corresponding to the inner information.
    ///
    /// `None` if no media segment is currently needed.
    fn init_segment(&self) -> Option<&InitSegmentInfo> {
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
        NextSegmentSelector, NextSegmentSelectors, SegmentCursor, SegmentSource,
        SegmentSourceChoice, SelectedSegment, SelectedSegmentRequest,
    };
    use crate::{
        bindings::MediaType,
        media_element::{BufferedChunk, SegmentQualityContext},
        parser::TopLevelPlaylist,
        utils::url::Url,
    };

    fn check_permission_transition(initial_permission: bool, expected_start: f64) {
        let mut text =
            String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\"\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        let parsed = TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new("https://example.com/high.m3u8".to_owned()),
        )
        .unwrap();
        let TopLevelPlaylist::DirectMedia(playlist) = parsed else {
            panic!("expected direct media playlist");
        };
        let list = playlist.playlist().segment_list();
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        let context = SegmentQualityContext::new(2., 2);
        let mut selectors = NextSegmentSelectors::new(0., 40.);
        selectors
            .get_mut(MediaType::Video)
            .allow_fast_quality_switching = initial_permission;
        let first = selectors
            .get_mut(MediaType::Video)
            .most_needed_segment(list, &context, &inventory);
        assert_eq!(
            first.media_segment.unwrap().start(),
            if initial_permission { 8. } else { 24. }
        );
        selectors
            .get_mut(MediaType::Video)
            .validate_init(first.init_segment.unwrap().id());
        selectors
            .get_mut(MediaType::Video)
            .validate_media_until(first.media_segment.unwrap().end());

        selectors
            .get_mut(MediaType::Video)
            .allow_fast_quality_switching = !initial_permission;
        let next = selectors
            .get_mut(MediaType::Video)
            .most_needed_segment(list, &context, &inventory);
        assert!(
            next.init_segment.is_none(),
            "the validated init is preserved"
        );
        assert_eq!(next.media_segment.unwrap().start(), expected_start);
        selectors
            .get_mut(MediaType::Video)
            .validate_media_until(next.media_segment.unwrap().end());

        // Repeated recommendations with the same permission must keep forward progress.
        selectors
            .get_mut(MediaType::Video)
            .allow_fast_quality_switching = !initial_permission;
        let following = selectors
            .get_mut(MediaType::Video)
            .most_needed_segment(list, &context, &inventory);
        assert!(following.init_segment.is_none());
        assert_eq!(
            following.media_segment.unwrap().start(),
            expected_start + 4.
        );
    }

    #[test]
    fn two_sources_choose_safe_replacement_and_best_addition_independently() {
        let safe_playlist = selection_test_playlist("safe");
        let best_playlist = selection_test_playlist("best");
        let TopLevelPlaylist::DirectMedia(safe_playlist) = safe_playlist else {
            panic!("expected media playlist")
        };
        let TopLevelPlaylist::DirectMedia(best_playlist) = best_playlist else {
            panic!("expected media playlist")
        };
        let safe = SegmentSource {
            playlist_id: selection_test_id(0),
            quality_context: SegmentQualityContext::new(2., selection_test_id(0).as_u32()),
            playlist: Some(safe_playlist.playlist()),
        };
        let best = SegmentSource {
            playlist_id: selection_test_id(1),
            quality_context: SegmentQualityContext::new(3., selection_test_id(1).as_u32()),
            playlist: Some(best_playlist.playlist()),
        };
        let audio: Vec<_> = (0..6)
            .map(|i| {
                BufferedChunk::new_for_test(
                    i as f64 * 4.,
                    (i + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., u32::MAX),
                )
            })
            .collect();
        let video = vec![BufferedChunk::new_for_test(
            0.,
            24.,
            safe.quality_context.clone(),
        )];
        let mut selectors = NextSegmentSelectors::new(0., 40.);
        let audio_request =
            select_media_request(selectors.get_mut(MediaType::Audio), &best, &safe, &audio);
        let video_request =
            select_media_request(selectors.get_mut(MediaType::Video), &best, &safe, &video);
        assert_eq!(audio_request.playlist_id, safe.playlist_id);
        assert_eq!(video_request.playlist_id, best.playlist_id);
        let SelectedSegment::Media {
            segment: audio_segment,
            ..
        } = audio_request.segment
        else {
            panic!("expected media")
        };
        let SelectedSegment::Media {
            segment: video_segment,
            ..
        } = video_request.segment
        else {
            panic!("expected media")
        };
        assert_eq!(audio_segment.start(), 8.);
        assert_eq!(video_segment.start(), 24.);
        assert!(
            !selectors
                .get_mut(MediaType::Video)
                .allow_fast_quality_switching
        );
    }

    #[test]
    fn selector_uses_safe_additions_until_best_is_ready_and_accepts_fresh_recommendations() {
        let safe_playlist = selection_test_playlist("safe");
        let best_playlist = selection_test_playlist("best");
        let TopLevelPlaylist::DirectMedia(safe_playlist) = safe_playlist else {
            panic!("expected media playlist")
        };
        let TopLevelPlaylist::DirectMedia(best_playlist) = best_playlist else {
            panic!("expected media playlist")
        };
        let safe = SegmentSource {
            playlist_id: selection_test_id(0),
            quality_context: SegmentQualityContext::new(1., selection_test_id(0).as_u32()),
            playlist: Some(safe_playlist.playlist()),
        };
        let mut best = SegmentSource {
            playlist_id: selection_test_id(1),
            quality_context: SegmentQualityContext::new(2., selection_test_id(1).as_u32()),
            playlist: None,
        };
        let mut inventory = vec![BufferedChunk::new_for_test(
            0.,
            24.,
            safe.quality_context.clone(),
        )];
        let mut selector = NextSegmentSelector::new(0., 40.);
        let pending = selector.select_next_segment(&best, Some(&safe), &inventory, true);
        assert!(pending.request.is_none());
        assert_eq!(
            pending.needed_playlists,
            vec![best.playlist_id, safe.playlist_id]
        );
        let fallback = select_media_request(&mut selector, &best, &safe, &inventory);
        assert_eq!(fallback.playlist_id, safe.playlist_id);
        let SelectedSegment::Media { segment, .. } = fallback.segment else {
            panic!("expected media")
        };
        assert_eq!(segment.start(), 24.);
        assert!(!selector.allow_fast_quality_switching);
        selector.validate_media_until(segment.end());
        inventory.push(BufferedChunk::new_for_test(
            24.,
            28.,
            safe.quality_context.clone(),
        ));

        // A recommendation change while best is loading must also change the hints.
        let fresh = selector.select_next_segment(&safe, Some(&safe), &inventory, true);
        assert_eq!(fresh.needed_playlists, vec![safe.playlist_id]);
        assert!(fresh.request.is_none());
        best.playlist = Some(best_playlist.playlist());
        let next = select_media_request(&mut selector, &best, &safe, &inventory);
        assert_eq!(next.playlist_id, best.playlist_id);
        let SelectedSegment::Media { segment, .. } = next.segment else {
            panic!("expected media")
        };
        assert_eq!(segment.start(), 28.);
        assert_eq!(
            selector
                .select_next_segment(&best, Some(&safe), &inventory, true)
                .needed_playlists,
            vec![best.playlist_id]
        );
    }

    #[test]
    fn preparation_hints_do_not_change_the_pending_init_source() {
        let parsed = selection_test_playlist("safe");
        let TopLevelPlaylist::DirectMedia(playlist) = parsed else {
            panic!("expected media playlist")
        };
        let safe = SegmentSource {
            playlist_id: *playlist.id(),
            quality_context: SegmentQualityContext::new(1., playlist.id().as_u32()),
            playlist: Some(playlist.playlist()),
        };
        let mut selector = NextSegmentSelector::new(0., 40.);
        let init = selector
            .select_next_segment(&safe, Some(&safe), &[], false)
            .request
            .unwrap();
        let SelectedSegment::Init(init) = init.segment else {
            panic!("expected init")
        };
        let previous_media_id = selector.last_media_id;
        let parsed_best = selection_test_playlist("best");
        let TopLevelPlaylist::DirectMedia(best_playlist) = parsed_best else {
            panic!("expected media playlist")
        };
        let best = SegmentSource {
            playlist_id: selection_test_id(1),
            quality_context: SegmentQualityContext::new(2., selection_test_id(1).as_u32()),
            playlist: Some(best_playlist.playlist()),
        };
        let pending = selector.select_next_segment(&best, Some(&safe), &[], true);
        assert!(pending.request.is_none());
        assert_eq!(selector.last_media_id, previous_media_id);
        selector.validate_init(init.id());
        let next = selector
            .select_next_segment(&safe, Some(&safe), &[], false)
            .request
            .unwrap();
        assert!(matches!(next.segment, SelectedSegment::Media { .. }));
    }

    fn selection_test_playlist(name: &str) -> TopLevelPlaylist {
        let mut text =
            String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\"\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        TopLevelPlaylist::parse(
            text.as_bytes(),
            Url::new(format!("https://example.com/{name}/main.m3u8")),
        )
        .unwrap()
    }

    fn selection_test_id(index: usize) -> crate::playlist_store::MediaPlaylistPermanentId {
        let parsed = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nsafe.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000\nbest.m3u8\n",
            Url::new("https://example.com/master.m3u8".to_owned()),
        ).unwrap();
        let TopLevelPlaylist::Multivariant(playlist) = parsed else {
            panic!("expected master")
        };
        playlist
            .video_media_playlist_id_for(playlist.variant_from_idx(index).unwrap())
            .unwrap()
    }

    fn select_media_request(
        selector: &mut NextSegmentSelector,
        best: &SegmentSource<'_>,
        safe: &SegmentSource<'_>,
        inventory: &[BufferedChunk],
    ) -> SelectedSegmentRequest {
        let selection = selector.select_next_segment(best, Some(safe), inventory, false);
        let request = selection.request.unwrap();
        if let SelectedSegment::Init(init) = &request.segment {
            selector.validate_init(init.id());
            selector
                .select_next_segment(best, Some(safe), inventory, false)
                .request
                .unwrap()
        } else {
            request
        }
    }

    #[test]
    fn request_role_prioritizes_low_buffer_and_does_not_replace_across_holes() {
        let selector = NextSegmentSelector::new(0., 30.);
        let safe = SegmentQualityContext::new(2., 2);
        for inventory in [
            vec![BufferedChunk::new_for_test(
                0.,
                4.,
                SegmentQualityContext::new(1., 1),
            )],
            vec![
                BufferedChunk::new_for_test(0., 6., SegmentQualityContext::new(1., 1)),
                BufferedChunk::new_for_test(8., 30., SegmentQualityContext::new(1., 1)),
            ],
        ] {
            assert_eq!(
                selector
                    .next_request_role(Some(&safe), &inventory, None)
                    .unwrap(),
                SegmentSourceChoice::Additive
            );
        }
    }

    #[test]
    fn request_role_is_absent_when_no_work_remains() {
        let selector = NextSegmentSelector::new(0., 30.);
        let safe = SegmentQualityContext::new(2., 2);
        let inventory = vec![BufferedChunk::new_for_test(0., 12., safe.clone())];
        assert!(selector
            .next_request_role(Some(&safe), &inventory, Some(12.))
            .is_none());
        let full = vec![BufferedChunk::new_for_test(0., 30., safe.clone())];
        assert!(selector
            .next_request_role(Some(&safe), &full, None)
            .is_none());
        assert_eq!(
            selector.next_request_role(None, &inventory, None).unwrap(),
            SegmentSourceChoice::Additive
        );
        assert!(selector.next_request_role(None, &full, None).is_none());
    }

    #[test]
    fn additive_selection_skips_lower_quality_coverage_even_after_cursor_restart() {
        let mut selector = NextSegmentSelector::new(0., 30.);
        selector.allow_fast_quality_switching = false;
        let best = SegmentQualityContext::new(3., 3);
        let inventory = vec![BufferedChunk::new_for_test(
            0.,
            12.,
            SegmentQualityContext::new(1., 1),
        )];
        assert!(selector.can_be_skipped(0., 12., &best, &inventory));
        selector.allow_fast_quality_switching = true;
        assert!(!selector.can_be_skipped(0., 12., &best, &inventory));
    }

    #[test]
    fn withdrawing_fast_switch_permission_continues_after_buffer_with_same_playlist() {
        check_permission_transition(true, 24.);
    }

    #[test]
    fn granting_fast_switch_permission_reconsiders_buffer_with_same_playlist() {
        check_permission_transition(false, 8.);
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
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    SegmentQualityContext::new(1., 1),
                )
            })
            .collect();
        for (allow_replacement, seek_position, expected_start, change_media) in [
            (false, 4., 24., false),
            (true, 4., 12., false),
            (false, 30., 28., false),
            (false, 4., 24., true),
        ] {
            let context = SegmentQualityContext::new(2., 2);
            let mut selectors = NextSegmentSelectors::new(0., 40.);
            selectors
                .get_mut(MediaType::Video)
                .allow_fast_quality_switching = allow_replacement;
            let selector = selectors.get_mut(MediaType::Video);
            let first = selector.most_needed_segment(list, &context, &inventory);
            selector.validate_init(first.init_segment.unwrap().id());
            selector.validate_media_until(first.media_segment.unwrap().end());

            selector.restart_from_position(seek_position - 0.2);
            let context = SegmentQualityContext::new(2., if change_media { 3 } else { 2 });
            let next = selector.most_needed_segment(list, &context, &inventory);
            assert_eq!(next.media_segment.unwrap().start(), expected_start);
            assert_eq!(next.init_segment.is_some(), change_media);
            selector.validate_media_until(next.media_segment.unwrap().end());

            let following = selector.most_needed_segment(list, &context, &inventory);
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

        assert!(selector
            .fast_quality_switch_position(&higher_quality, &buffered)
            .is_some());
        assert_eq!(
            selector.recompute_starting_position(&higher_quality, &buffered),
            6.
        );
        selector.allow_fast_quality_switching = false;
        assert_eq!(
            selector.recompute_starting_position(&higher_quality, &buffered),
            10.
        );
    }

    #[test]
    fn fast_quality_switch_requires_the_minimum_lead() {
        let selector = NextSegmentSelector::new(0., 30.);
        let buffered = vec![
            BufferedChunk::new_for_test(0., 5., SegmentQualityContext::new(1., 1)),
            BufferedChunk::new_for_test(5., 9., SegmentQualityContext::new(1., 1)),
        ];
        let higher_quality = SegmentQualityContext::new(2., 2);

        assert!(!selector
            .fast_quality_switch_position(&higher_quality, &buffered)
            .is_some());
        assert_eq!(
            selector.recompute_starting_position(&higher_quality, &buffered),
            9.
        );
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
