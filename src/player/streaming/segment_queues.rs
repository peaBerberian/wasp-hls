use std::slice::Iter;
use crate::{parser::SegmentInfo, source_buffer::MediaType};

#[derive(Clone, Debug)]
pub struct SegmentQueue {
    last_seg_start: Option<f64>,
    initial_pos: f64,
}

impl SegmentQueue {
    pub(crate) fn new(initial_pos: f64) -> Self {
        Self { last_seg_start: None, initial_pos }
    }

    pub(crate) fn reset(&mut self, initial_pos: f64) {
        self.last_seg_start = None;
        self.initial_pos = initial_pos;
    }

    pub(crate) fn get_next<'a>(&mut self,
        segment_list: &'a [SegmentInfo],
        maximum_position: f64
    ) -> Option<&'a SegmentInfo> {
        let next_seg = match self.last_seg_start {
            Some(last_start) =>
                Self::iter_segment_list_from_position(segment_list, last_start)
                    .find(|s|  s.start > last_start),
            None => Self::iter_segment_list_from_position(segment_list, self.initial_pos)
                .next(),
        };
        match next_seg {
            Some(seg) if seg.start <= maximum_position => {
                self.last_seg_start = Some(seg.start);
                next_seg
            }
            _ => None
        }
    }

    fn iter_segment_list_from_position(
        segment_list: &[SegmentInfo],
        position: f64
    ) -> Iter<SegmentInfo> {
        segment_list.iter()
            .position(|s| s.start + s.duration > position)
            .map_or([].iter(), |start_idx| segment_list[start_idx..].iter())
    }
}

pub struct SegmentQueues {
    audio: SegmentQueue,
    video: SegmentQueue,
}

impl SegmentQueues {
    pub(crate) fn new() -> Self {
        Self {
            audio: SegmentQueue::new(0.),
            video: SegmentQueue::new(0.),
        }
    }

    pub(crate) fn get(&self, media_type: MediaType) -> &SegmentQueue {
        match media_type {
            MediaType::Video => &self.video,
            MediaType::Audio => &self.audio,
        }
    }

    pub(crate) fn get_mut(&mut self, media_type: MediaType) -> &mut SegmentQueue {
        match media_type {
            MediaType::Video => &mut self.video,
            MediaType::Audio => &mut self.audio,
        }
    }

    pub(crate) fn reset(&mut self, position: f64) {
        self.audio = SegmentQueue::new(position);
        self.video = SegmentQueue::new(position);
    }
}
