use crate::{
    bindings::MediaType,
    parser::{MapInfo, MediaPlaylist, SegmentInfo},
};

mod segment_queue;
use segment_queue::SegmentQueue;

pub struct NextSegmentSelectors {
    audio: NextSegmentSelector,
    video: NextSegmentSelector,
}

impl NextSegmentSelectors {
    pub fn new(base_pos: f64, buffer_goal: f64) -> Self {
        Self {
            audio: NextSegmentSelector::new(base_pos, buffer_goal),
            video: NextSegmentSelector::new(base_pos, buffer_goal),
        }
    }

    pub fn update_buffer_goal(&mut self, buffer_goal: f64) {
        self.audio.buffer_goal = buffer_goal;
        self.video.buffer_goal = buffer_goal;
    }

    pub fn get(&self, media_type: MediaType) -> &NextSegmentSelector {
        match media_type {
            MediaType::Audio => &self.audio,
            MediaType::Video => &self.video,
        }
    }

    pub fn get_mut(&mut self, media_type: MediaType) -> &mut NextSegmentSelector {
        match media_type {
            MediaType::Audio => &mut self.audio,
            MediaType::Video => &mut self.video,
        }
    }

    pub fn update_base_position(&mut self, pos: f64) {
        self.audio.update_base_position(pos);
        self.video.update_base_position(pos);
    }

    pub fn reset_position(&mut self, pos: f64) {
        self.audio.reset_position(pos);
        self.video.reset_position(pos);
    }

    pub fn reset_position_for_type(&mut self, mt: MediaType, pos: f64) {
        match mt {
            MediaType::Audio => self.audio.reset_position(pos),
            MediaType::Video => self.video.reset_position(pos),
        }
    }
}

pub struct NextSegmentSelector {
    /// Interface allowing to keep track of which audio and video segments we need to load next.
    segment_queue: SegmentQueue,

    /// Amount of buffer, ahead of the current position we want to build in seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    buffer_goal: f64,

    base_pos: f64,

    /// The starting position of the last segment returned through `get_next_segment_info`.
    last_returned_position: Option<f64>,

    /// The starting position of the last validated segment.
    last_validated_position: Option<f64>,

    init_status: InitializationSegmentSelectorStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitializationSegmentSelectorStatus {
    Unreturned,
    None,
    Returned,
    Validated,
}

impl NextSegmentSelector {
    fn new(base_pos: f64, buffer_goal: f64) -> Self {
        Self {
            segment_queue: SegmentQueue::new(base_pos),
            base_pos,
            last_returned_position: None,
            buffer_goal,
            last_validated_position: None,
            init_status: InitializationSegmentSelectorStatus::Unreturned,
        }
    }

    pub fn reset_init_segment(&mut self) {
        self.init_status = InitializationSegmentSelectorStatus::Unreturned;
    }

    pub fn rollback(&mut self) {
        if let Some(pos) = self.last_validated_position {
            self.segment_queue.validate_start(pos);
        } else {
            self.segment_queue.reset(self.base_pos);
        }
        self.last_returned_position = self.last_validated_position;
        if self.init_status == InitializationSegmentSelectorStatus::Returned {
            self.init_status = InitializationSegmentSelectorStatus::Unreturned;
        }
    }

    pub fn update_base_position(&mut self, base_pos: f64) {
        self.base_pos = base_pos;
    }

    pub fn reset_position(&mut self, pos: f64) {
        self.base_pos = pos;
        self.last_returned_position = None;
        self.last_validated_position = None;
        self.init_status = InitializationSegmentSelectorStatus::Unreturned;
        self.segment_queue = SegmentQueue::new(pos);
    }

    pub fn validate_init(&mut self) {
        self.init_status = InitializationSegmentSelectorStatus::Validated;
    }

    pub fn validate_media(&mut self, pos: f64) {
        self.last_validated_position = Some(pos);
    }

    pub fn get_next_segment_info<'a>(&mut self, pl: &'a MediaPlaylist) -> NextSegmentInfo<'a> {
        if self.init_status == InitializationSegmentSelectorStatus::Unreturned {
            if let Some(i) = pl.init_segment() {
                self.init_status = InitializationSegmentSelectorStatus::Returned;
                return NextSegmentInfo::InitSegment(i);
            } else {
                self.init_status = InitializationSegmentSelectorStatus::None;
            }
        }
        let maximum_position = self.buffer_goal + self.base_pos;
        match self
            .segment_queue
            .get_next(&pl.segment_list(), maximum_position)
        {
            None => NextSegmentInfo::None,
            Some(si) => {
                self.segment_queue.validate_start(si.start);
                self.last_returned_position = Some(si.start);
                NextSegmentInfo::MediaSegment(si)
            }
        }
    }
}

pub enum NextSegmentInfo<'a> {
    None,
    MediaSegment(&'a SegmentInfo),
    InitSegment(&'a MapInfo),
}
