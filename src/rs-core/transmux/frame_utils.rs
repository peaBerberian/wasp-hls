// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

use super::nal_unit_producer::{ParsedNalUnit, UnitTypeBase};

pub(super) struct FrameObject {
    dts: u32,
    pts: u32,
    nb_bytes: u32,
    duration: u32,
    key_frame: bool,
    data: Vec<UnitTypeBase>,
}

impl FrameObject {
    pub(super) fn dts(&self) -> u32 {
        self.dts
    }
    pub(super) fn pts(&self) -> u32 {
        self.pts
    }
    pub(super) fn nb_bytes(&self) -> u32 {
        self.nb_bytes
    }
    pub(super) fn duration(&self) -> u32 {
        self.duration
    }
    pub(super) fn key_frame(&self) -> bool {
        self.key_frame
    }
    pub(super) fn data(&self) -> &[UnitTypeBase] {
        self.data.as_slice()
    }
}

pub(super) struct FramesObject {
    nal_count: u32,
    duration: u32,
    nb_bytes: u32,
    frames: Vec<FrameObject>,
}

pub(super) struct GopData {
    frames: Vec<FrameObject>,
    nb_bytes: u32,
    nal_count: u32,
    duration: u32,
    pts: u32,
    dts: u32,
}

impl GopData {
    pub(super) fn pts(&self) -> u32 {
        self.pts
    }
    pub(super) fn dts(&self) -> u32 {
        self.dts
    }
    pub(super) fn duration(&self) -> u32 {
        self.duration
    }
    pub(super) fn nb_bytes(&self) -> u32 {
        self.nb_bytes
    }
    pub(super) fn nal_count(&self) -> u32 {
        self.nal_count
    }
    pub(super) fn frames(&self) -> &[FrameObject] {
        self.frames.as_slice()
    }
}

pub(super) struct GopsSet {
    gops: Vec<GopData>,
    nb_bytes: u32,
    nal_count: u32,
    duration: u32,
    pts: u32,
    dts: u32,
}

impl GopsSet {
    pub(super) fn pts(&self) -> u32 {
        self.pts
    }
    pub(super) fn dts(&self) -> u32 {
        self.dts
    }
    pub(super) fn duration(&self) -> u32 {
        self.duration
    }
    pub(super) fn gops(&self) -> &[GopData] {
        self.gops.as_slice()
    }
    pub(super) fn nb_bytes(&self) -> u32 {
        self.nb_bytes
    }
    pub(super) fn nal_count(&self) -> u32 {
        self.nal_count
    }
}

// as the frame duration, starting pts, etc.
// XXX TODO I think we're not supposed to lose rbsp and so on here
pub(super) fn group_nals_into_frames(nal_units: Vec<ParsedNalUnit>) -> FramesObject {
    let mut frames_obj = FramesObject {
        nal_count: 0,
        duration: 0,
        nb_bytes: 0,
        frames: vec![],
    };
    let mut current_frame = FrameObject {
        dts: 0,
        pts: 0,
        duration: 0,
        nb_bytes: 0,
        key_frame: false,
        data: vec![],
    };
    nal_units.into_iter().for_each(|nal| {
        match nal {
            ParsedNalUnit::AccessUnitDelim(dat) => {
                let dts = dat.dts();
                let new_frame = FrameObject {
                    dts,
                    pts: dat.pts(),
                    key_frame: false,
                    duration: 0,
                    nb_bytes: dat.data().len() as u32,
                    data: vec![dat],
                };

                // Since the very first nal unit is expected to be an AUD
                // only push to the frames array when current_frame is not empty
                if !current_frame.data.is_empty() {
                    current_frame.duration = dts - current_frame.dts;
                    // TODO added for LHLS, make sure this is OK
                    frames_obj.nb_bytes += current_frame.nb_bytes;
                    frames_obj.nal_count += current_frame.data.len() as u32;
                    frames_obj.duration += current_frame.duration;
                    let prev_curr = std::mem::replace(&mut current_frame, new_frame);
                    frames_obj.frames.push(prev_curr);
                } else {
                    current_frame = new_frame;
                }
            }
            ParsedNalUnit::SliceLayerWo(dat) => {
                current_frame.key_frame = true;
                current_frame.duration = dat.dts() - current_frame.dts;
                current_frame.nb_bytes += dat.data().len() as u32;
                current_frame.data.push(dat);
            }
            ParsedNalUnit::SeiRbsp(dat, _)
            | ParsedNalUnit::SeqParamSetRbsp(dat, _, _)
            | ParsedNalUnit::PicParamSet(dat)
            | ParsedNalUnit::Undefined(dat) => {
                current_frame.duration = dat.dts() - current_frame.dts;
                current_frame.nb_bytes += dat.data().len() as u32;
                current_frame.data.push(dat);
            }
        }
    });

    // For the last frame, use the duration of the previous frame if we
    // have nothing better to go on
    if !frames_obj.frames.is_empty() && current_frame.duration == 0 {
        current_frame.duration = frames_obj.frames.last().unwrap().duration;
    }

    // Push the final frame
    // TODO added for LHLS, make sure this is OK
    frames_obj.nb_bytes += current_frame.nb_bytes;
    frames_obj.nal_count += current_frame.data.len() as u32;
    frames_obj.duration += current_frame.duration;

    frames_obj.frames.push(current_frame);
    frames_obj
}

/// Convert an array of frames into an array of Gop with each Gop being composed
/// of the frames that make up that Gop
/// Also keep track of cummulative data about the Gop from the frames such as the
/// Gop duration, starting pts, etc.
pub(super) fn group_frames_into_gops(frames_obj: FramesObject) -> GopsSet {
    let mut gops = GopsSet {
        gops: vec![],
        nb_bytes: 0,
        nal_count: 0,
        duration: 0,
        pts: 0,
        dts: 0,
    };
    if frames_obj.frames.is_empty() {
        return gops;
    }

    // We must pre-set some of the values on the Gop since we
    // keep running totals of these values
    let mut current_gop = GopData {
        frames: vec![],
        nb_bytes: 0,
        nal_count: 0,
        duration: 0,
        pts: frames_obj.frames[0].pts,
        dts: frames_obj.frames[0].dts,
    };

    // store some metadata about all the Gops
    gops.pts = frames_obj.frames[0].pts;
    gops.dts = frames_obj.frames[0].dts;

    frames_obj.frames.into_iter().for_each(|frame| {
        if !frame.key_frame {
            let new_gop = GopData {
                nb_bytes: frame.nb_bytes,
                nal_count: frame.data.len() as u32,
                duration: frame.duration,
                pts: frame.pts,
                dts: frame.dts,
                frames: vec![frame],
            };

            // If empty, this is the first key frame encountered, so way not
            // construct a whole gop from this.
            // If not empty, the previous gop was ended and we're currently
            // processing the key frame of the next one
            if !current_gop.frames.is_empty() {
                gops.nb_bytes += current_gop.nb_bytes;
                gops.nal_count += current_gop.nal_count;
                gops.duration += current_gop.duration;
                let prev_curr = std::mem::replace(&mut current_gop, new_gop);
                gops.gops.push(prev_curr);
            } else {
                current_gop = new_gop;
            }
        } else {
            current_gop.duration += frame.duration;
            current_gop.nal_count += frame.data.len() as u32;
            current_gop.nb_bytes += frame.nb_bytes;
            current_gop.frames.push(frame);
        }
    });

    if !gops.gops.is_empty() && current_gop.duration == 0 {
        current_gop.duration = gops.gops.last().unwrap().duration;
    }
    gops.nb_bytes += current_gop.nb_bytes;
    gops.nal_count += current_gop.nal_count;
    gops.duration += current_gop.duration;
    gops.gops.push(current_gop);
    gops
}

/// Search for the first keyframe in the GOPs and throw away all frames
/// until that keyframe. Then extend the duration of the pulled keyframe
/// and pull the PTS and DTS of the keyframe so that it covers the time
/// range of the frames that were disposed.
pub(super) fn extend_first_key_frame(gops: &mut GopsSet) {
    if gops.gops.is_empty() {
        return;
    }
    if !gops.gops[0].frames[0].key_frame && gops.gops.len() > 1 {
        // Remove until the first key frame (so basically, remove the first GOP)
        let first_gop = gops.gops.remove(0);
        gops.nb_bytes -= first_gop.nb_bytes;
        gops.nal_count -= first_gop.nal_count;

        // Extend the first frame of what is now the
        // first gop to cover the time period of the
        // frames we just removed
        gops.gops[0].frames[0].dts = first_gop.dts;
        gops.gops[0].frames[0].pts = first_gop.pts;
        gops.gops[0].frames[0].duration += first_gop.duration;
    }
}

/// generate the track's raw mdat data from an array of gops
pub(super) fn concatenate_nal_data(gops: GopsSet) -> Vec<u8> {
    let nals_byte_length = gops.nb_bytes;
    let number_of_nals = gops.nal_count;
    let total_byte_length = nals_byte_length + 4 * number_of_nals;
    let mut data = Vec::with_capacity(total_byte_length as usize);
    gops.gops
        .iter()
        .flat_map(|g| g.frames())
        .flat_map(|f| f.data())
        .for_each(|d| {
            let u32_len = (d.data().len() as u32).to_be_bytes();
            data.extend(u32_len);
            data.extend(d.data());
        });
    data
}

// generate the track's raw mdat data from a frame
pub(super) fn concatenate_nal_data_for_frame(frame: &FrameObject) -> Vec<u8> {
    let nals_byte_length = frame.nb_bytes;
    let number_of_nals = frame.data.len() as u32;
    let total_byte_length = nals_byte_length + 4 * number_of_nals;
    let mut data = Vec::with_capacity(total_byte_length as usize);
    frame.data().iter().for_each(|d| {
        let u32_len = (d.data().len() as u32).to_be_bytes();
        data.extend(u32_len);
        data.extend(d.data());
    });
    data
}
