use crate::{
    bindings::MediaType,
    media_element::{BufferedChunk, SegmentQualityContext},
    parser::{InitSegmentInfo, SegmentInfo, SegmentList, SegmentTimeInfo},
    Logger,
};

pub(crate) struct NextSegmentSelectors {
    audio: NextSegmentSelector,
    video: NextSegmentSelector,
}

impl NextSegmentSelectors {
    pub(crate) fn new(base_pos: f64, buffer_goal: f64) -> Self {
        Self {
            audio: NextSegmentSelector::new(base_pos, buffer_goal),
            video: NextSegmentSelector::new(base_pos, buffer_goal),
        }
    }

    pub(crate) fn update_buffer_goal(&mut self, buffer_goal: f64) {
        self.audio.buffer_goal = buffer_goal;
        self.video.buffer_goal = buffer_goal;
    }

    pub(crate) fn get(&self, media_type: MediaType) -> &NextSegmentSelector {
        match media_type {
            MediaType::Audio => &self.audio,
            MediaType::Video => &self.video,
        }
    }

    pub(crate) fn get_mut(&mut self, media_type: MediaType) -> &mut NextSegmentSelector {
        match media_type {
            MediaType::Audio => &mut self.audio,
            MediaType::Video => &mut self.video,
        }
    }

    pub(crate) fn update_base_position(&mut self, pos: f64) {
        self.audio.update_base_position(pos);
        self.video.update_base_position(pos);
    }

    pub(crate) fn reset(&mut self, pos: f64) {
        let pos = f64::max(0., pos);
        self.audio.reset(pos);
        self.video.reset(pos);
    }

    pub(crate) fn restart_from(&mut self, pos: f64) {
        let pos = f64::max(0., pos);
        self.audio.restart_from(pos);
        self.video.restart_from(pos);
    }
}

pub(crate) struct NextSegmentSelector {
    /// Interface allowing to keep track of which audio and video segments we need to load next.
    segment_queue: SegmentQueue,

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitializationSegmentSelectorStatus {
    None,
    Unvalidated,
    Validated,
}

impl NextSegmentSelector {
    fn new(base_pos: f64, buffer_goal: f64) -> Self {
        let real_base_pos = f64::max(0., base_pos);
        Self {
            segment_queue: SegmentQueue::new(base_pos),
            base_pos: real_base_pos,
            buffer_goal,
            last_media_id: None,
            init_status: InitializationSegmentSelectorStatus::Unvalidated,
            skipped_segments: vec![],
        }
    }

    /// Reset the `NextSegmentSelector` state, as if no media segment nor init segment was
    /// returned by it, and start back from the given position.
    pub(crate) fn reset(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.init_status = InitializationSegmentSelectorStatus::Unvalidated;
        self.last_media_id = None;
        self.segment_queue = SegmentQueue::new(base_pos);
        self.skipped_segments.clear();
    }

    pub(crate) fn restart_from(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.segment_queue = SegmentQueue::new(base_pos);
        self.skipped_segments.clear();
    }

    pub(crate) fn update_base_position(&mut self, base_pos: f64) {
        self.base_pos = f64::max(0., base_pos);
        self.clean_skipped_segments();
    }

    /// Indicate that the initialization segment was requested and as such, don't need to be
    /// returned anymore by `most_needed_segment`.
    pub(crate) fn validate_init(&mut self) {
        self.init_status = InitializationSegmentSelectorStatus::Validated;
    }

    /// Indicate that the initialization segment ending at `pos` was requested and as such, don't
    /// need to be returned anymore by `most_needed_segment`.
    pub(crate) fn validate_media_until(&mut self, pos: f64) {
        self.segment_queue.validate_until(pos);
    }

    /// Returns the current most needed segment according to the current situation and to the last
    /// "validated" init and media segment (see the other methods).
    pub(crate) fn most_needed_segment<'a>(
        &mut self,
        segment_list: &'a SegmentList,
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> NextSegmentInfo<'a> {
        let new_media_id = context.media_id();
        let has_quality_changed = Some(new_media_id) != self.last_media_id;
        self.last_media_id = Some(new_media_id);

        if has_quality_changed {
            Logger::debug("Selector: Quality changed, recomputing segment queue start");
            self.init_status = InitializationSegmentSelectorStatus::Unvalidated;
            self.segment_queue.reset(self.base_pos);
            self.skipped_segments.clear();
            let queue_start = self.recompute_queue_start(context, inventory);
            self.segment_queue.validate_until(queue_start);
        }

        if self.init_status == InitializationSegmentSelectorStatus::Unvalidated {
            if let Some(i) = segment_list.init() {
                return NextSegmentInfo::InitSegment(i);
            } else {
                self.init_status = InitializationSegmentSelectorStatus::None;
            }
        }
        self.check_skipped_segments(context, inventory);
        self.recursively_check_most_needed_media_segment(segment_list.media(), context, inventory)
    }

    /// Starts from `self.base_pos`, look at what is already buffered, and determine a new optimal
    /// starting point for segments of the given quality.
    ///
    /// Note that the quality has an influence here because of "fast-switching" which is the concept
    /// of replacing segments of a poor quality by segments of a higher quality. If segments of a
    /// poorer quality is detected in the currently buffered `inventory`, the returned f64 might
    /// thus be earlier than in the opposite case.
    fn recompute_queue_start(
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
                    //
                    // TODO based on segment target duration instead of `5.`?
                    // Or maybe the duration of the next segment in the SegmentInfo slice
                    if seg_i.last_buffered_end() - self.base_pos > 5. {
                        Logger::debug(&format!("Selector: Fast switching from {prev_end}"));
                        return prev_end;
                    }
                }
                prev_end = seg_i.playlist_end();
                curr_idx += 1;
            }
            Logger::debug(&format!(
                "Selector: Queue start after inventory: {prev_end}"
            ));
            prev_end
        } else {
            Logger::debug(&format!(
                "Selector: Queue start at beginning {}",
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

    fn recursively_check_most_needed_media_segment<'a>(
        &mut self,
        media_segments: &'a [SegmentInfo],
        context: &SegmentQualityContext,
        inventory: &[BufferedChunk],
    ) -> NextSegmentInfo<'a> {
        let maximum_position = self.buffer_goal + self.base_pos;
        match self
            .segment_queue
            .get_next(media_segments, maximum_position)
        {
            None => NextSegmentInfo::None,
            Some(si) => {
                let segment_end = si.end();

                // Check for "smart-switching", which is to avoid returning segments who have
                // already an equal or even better quality in the buffer.
                if self.can_be_skipped(si.start(), segment_end, context, inventory) {
                    Logger::debug(&format!(
                        "Selector: Segment can be skipped (s:{}, d: {})",
                        si.start(),
                        si.duration()
                    ));
                    let skipped = SegmentTimeInfo::new(si.start(), segment_end);
                    match self
                        .skipped_segments
                        .iter()
                        .position(|sk| sk.start() > si.start())
                    {
                        Some(pos) => self.skipped_segments.insert(pos, skipped),
                        None => self.skipped_segments.push(skipped),
                    }
                    self.segment_queue.validate_until(segment_end);
                    self.recursively_check_most_needed_media_segment(
                        media_segments,
                        context,
                        inventory,
                    )
                } else {
                    NextSegmentInfo::MediaSegment(si)
                }
            }
        }
    }

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

pub(crate) enum NextSegmentInfo<'a> {
    None,
    MediaSegment(&'a SegmentInfo),
    InitSegment(&'a InitSegmentInfo),
}

#[derive(Clone, Debug)]
pub(crate) struct SegmentQueue {
    validated_pos: Option<f64>,
    initial_pos: f64,
}

impl SegmentQueue {
    pub(crate) fn new(initial_pos: f64) -> Self {
        Self {
            validated_pos: None,
            initial_pos,
        }
    }

    pub(crate) fn reset(&mut self, initial_pos: f64) {
        self.validated_pos = None;
        self.initial_pos = initial_pos;
    }

    pub(crate) fn validate_until(&mut self, seg_start: f64) {
        self.validated_pos = Some(seg_start);
    }

    pub(crate) fn get_next<'a>(
        &mut self,
        media_segments: &'a [SegmentInfo],
        maximum_position: f64,
    ) -> Option<&'a SegmentInfo> {
        let position = self.validated_pos.unwrap_or(self.initial_pos);
        let next_seg = media_segments.iter().find(|s| (s.end()) > position);
        match next_seg {
            Some(seg) if seg.start() <= maximum_position => next_seg,
            _ => None,
        }
    }
}
