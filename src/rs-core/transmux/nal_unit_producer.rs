// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

use crate::transmux::elementary_packet_parser::ElementaryMediaType;

use super::{elementary_packet_parser::MediaElementaryPacket, transport_packet_parser::Pid};

/// Accepts a NAL unit byte stream and unpacks the embedded NAL units.
struct NalUnitFinder {
    sync_point: usize,
    buffer: Vec<u8>,
    cursor: Option<usize>,
}

impl NalUnitFinder {
    fn new() -> Self {
        Self {
            sync_point: 0,
            buffer: vec![],
            cursor: None,
        }
    }

    /// Scans a byte stream and return the NAL units found.
    fn find_next(&mut self, data: Vec<u8>) -> Vec<u8> {
        if self.buffer.is_empty() {
            self.buffer = data;
        } else {
            self.buffer.extend(data);
        }

        let mut res: Vec<u8> = vec![];

        // Rec. ITU-T H.264, Annex B
        // scan for NAL unit boundaries

        // a match looks like self:
        // 0 0 1 .. NAL .. 0 0 1
        // ^ sync point        ^ i
        // or self:
        // 0 0 1 .. NAL .. 0 0 0
        // ^ sync point        ^ i

        // advance the sync point to a NAL start, if necessary
        while self.sync_point < self.buffer.len() - 3 {
            if self.buffer[self.sync_point + 2] == 1 {
                // the sync point is properly aligned
                self.cursor = Some(self.sync_point + 5);
                break;
            }
            self.sync_point += 1;
        }

        while let Some(mut curs) = self.cursor {
            if curs >= self.buffer.len() || curs == 0 {
                break;
            }
            // look at the current byte to determine if we've hit the end of
            // a NAL unit boundary
            match self.buffer[curs] {
                0 => {
                    // skip past non-sync sequences
                    if curs >= 1 && self.buffer[curs - 1] != 0 {
                        self.cursor = Some(curs + 2);
                    } else if curs >= 2 && self.buffer[curs - 2] != 0 {
                        self.cursor = Some(curs + 1);
                    } else if curs >= 2 {
                        // deliver the NAL unit if it isn't empty
                        if self.sync_point + 3 < curs - 2 {
                            res.extend(self.buffer[self.sync_point + 3..curs - 2].to_owned());
                        }

                        // drop trailing zeroes
                        curs += 1;
                        while curs < self.buffer.len() && self.buffer[curs] != 1 {
                            curs += 1;
                        }
                        self.sync_point = curs - 2;
                        self.cursor = Some(curs + 3);
                    } else {
                        self.cursor = Some(curs + 3);
                    }
                }
                1 => {
                    // skip past non-sync sequences
                    if (curs >= 1 && self.buffer[curs - 1] != 0)
                        || (curs >= 2 && self.buffer[curs - 2] != 0)
                    {
                        self.cursor = Some(curs + 3);
                    } else if curs >= 2 {
                        if self.sync_point + 3 < curs - 2 {
                            // deliver the NAL unit
                            res.extend(self.buffer[self.sync_point + 3..curs - 2].to_owned());
                        }
                        self.sync_point = curs - 2;
                        self.cursor = Some(curs + 3);
                    } else {
                        self.cursor = Some(curs + 3);
                    }
                }
                _ => {
                    // the current byte isn't a one or zero, so it cannot be part
                    // of a sync sequence
                    self.cursor = Some(curs + 3);
                }
            }
        }

        // filter out the NAL units that were delivered
        if self.sync_point < self.buffer.len() {
            self.buffer = self.buffer[self.sync_point..].to_owned();
        } else {
            self.buffer.clear();
        }
        if let Some(curs) = self.cursor {
            self.cursor = Some(curs - self.sync_point);
        }
        self.sync_point = 0;
        res
    }

    fn reset(&mut self) {
        self.buffer = vec![];
        self.cursor = None;
        self.sync_point = 0;
    }

    fn flush(&mut self) -> Option<Vec<u8>> {
        let mut nal_unit = None;
        // deliver the last buffered NAL unit
        if self.buffer.len() > self.sync_point + 3 {
            nal_unit = Some(self.buffer[self.sync_point + 3..].to_owned());
        }
        self.buffer = vec![];
        self.cursor = None;
        self.sync_point = 0;
        nal_unit
    }
}

/// values of profile_idc that indicate additional fields are included in the SPS see
/// Recommendation ITU-T H.264 (4/2013),
/// 7.3.2.1.1 Sequence parameter set data syntax
static PROFILES_WITH_OPTIONAL_SPS_DATA: [u8; 12] = [
    100, 110, 122, 244, 44, 83, 86, 118, 128,
    // TODO: the three profiles below don't appear to have sps data in the specificiation anymore?
    138, 139, 124,
];

pub(super) enum ParsedNalUnit {
    SliceLayerWo(UnitTypeBase),
    SeiRbsp(UnitTypeBase, Rbsp),
    SeqParamSetRbsp(UnitTypeBase, Rbsp, NalVideoProperties),
    PicParamSet(UnitTypeBase),
    AccessUnitDelim(UnitTypeBase),
    Undefined(UnitTypeBase),
}

pub(super) struct UnitTypeBase {
    track_id: Pid,
    pts: u32,
    dts: u32,
    data: Vec<u8>,
}

pub(super) struct Rbsp {
    escaped_rbsp: Vec<u8>,
}

pub(super) struct NalVideoProperties {
    profile_idc: u8,
    level_idc: u8,
    profile_compatibility: u8,
    width: u64,
    height: u64,
    sar_ratio: (u8, u8),
}

/// Produces H.264 NAL unit data events.
pub(super) struct NalUnitProducer {
    nal_unit_finder: NalUnitFinder,
    last_track_id: Option<Pid>,
    last_pts: Option<u32>,
    last_dts: Option<u32>,
}

impl NalUnitProducer {
    pub(super) fn new() -> Self {
        Self {
            nal_unit_finder: NalUnitFinder::new(),
            last_track_id: None,
            last_pts: None,
            last_dts: None,
        }
    }
    /// Pushes a video packet to parse its inner Nal Units.
    pub(super) fn push_packet(&mut self, packet: MediaElementaryPacket) -> Vec<ParsedNalUnit> {
        if packet.media_type() != ElementaryMediaType::Video {
            return vec![];
        }
        let track_id = packet.track_id();
        let pts = packet.pts();
        let dts = packet.dts();
        self.last_track_id = Some(track_id);
        self.last_pts = pts;
        self.last_dts = dts;
        let _nal_units = self.nal_unit_finder.find_next(packet.data().to_owned());
        // TODO
        // nal_units.into_iter().map(|data| {
        //   self.on_nal_unit(data);
        // }).collect()
        vec![]
    }
}
