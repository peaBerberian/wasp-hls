use crate::parser::SegmentInfo;
use std::slice::Iter;

#[derive(Clone, Debug)]
pub(crate) struct SegmentQueue {
    last_seg_start: Option<f64>,
    initial_pos: f64,
}

impl SegmentQueue {
    pub(crate) fn new(initial_pos: f64) -> Self {
        Self {
            last_seg_start: None,
            initial_pos,
        }
    }

    pub(crate) fn reset(&mut self, initial_pos: f64) {
        self.last_seg_start = None;
        self.initial_pos = initial_pos;
    }

    pub(crate) fn validate_start(&mut self, seg_start: f64) {
        self.last_seg_start = Some(seg_start);
    }

    pub(crate) fn get_next<'a>(
        &mut self,
        segment_list: &'a [SegmentInfo],
        maximum_position: f64,
    ) -> Option<&'a SegmentInfo> {
        let next_seg = match self.last_seg_start {
            Some(last_start) => Self::iter_segment_list_from_position(segment_list, last_start)
                .find(|s| s.start > last_start),
            None => Self::iter_segment_list_from_position(segment_list, self.initial_pos).next(),
        };
        match next_seg {
            Some(seg) if seg.start <= maximum_position => next_seg,
            _ => None,
        }
    }

    fn iter_segment_list_from_position(
        segment_list: &[SegmentInfo],
        position: f64,
    ) -> Iter<SegmentInfo> {
        segment_list
            .iter()
            .position(|s| s.start + s.duration > position)
            .map_or([].iter(), |start_idx| segment_list[start_idx..].iter())
    }
}
