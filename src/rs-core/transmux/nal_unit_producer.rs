// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

use crate::transmux::elementary_packet_parser::ElementaryMediaType;

use super::{elementary_packet_parser::MediaElementaryPacket, transport_packet_parser::Pid};

use super::exp_golomb::ExpGolomb;

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
    fn find_next(&mut self, data: Vec<u8>) -> Vec<Vec<u8>> {
        if self.buffer.is_empty() {
            self.buffer = data;
        } else {
            self.buffer.extend(data);
        }

        let mut res = vec![];

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
                            res.push(self.buffer[self.sync_point + 3..curs - 2].to_owned());
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
                            res.push(self.buffer[self.sync_point + 3..curs - 2].to_owned());
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

impl UnitTypeBase {
    pub(super) fn pts(&self) -> u32 {
        self.pts
    }
    pub(super) fn dts(&self) -> u32 {
        self.dts
    }
    pub(super) fn track_id(&self) -> Pid {
        self.track_id
    }
    pub(super) fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
    pub(super) fn take_data(self) -> Vec<u8> {
        self.data
    }
}

pub(super) struct Rbsp {
    escaped_rbsp: Vec<u8>,
}

pub(super) struct NalVideoProperties {
    profile_idc: u8,
    level_idc: u8,
    profile_compatibility: u8,
    width: u32,
    height: u32,
    sar_ratio: (u16, u16),
}

impl NalVideoProperties {
    pub(super) fn width(&self) -> u32 {
        self.width
    }
    pub(super) fn height(&self) -> u32 {
        self.height
    }
    pub(super) fn profile_idc(&self) -> u8 {
        self.profile_idc
    }
    pub(super) fn level_idc(&self) -> u8 {
        self.level_idc
    }
    pub(super) fn profile_compatibility(&self) -> u8 {
        self.profile_compatibility
    }
    pub(super) fn sar_ratio(&self) -> (u16, u16) {
        self.sar_ratio
    }
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
        let nal_units = self.nal_unit_finder.find_next(packet.data().to_owned());
        nal_units
            .into_iter()
            .map(|data| self.on_nal_unit(data, track_id, pts.unwrap_or(0), dts.unwrap_or(0)))
            .collect()
    }

    /// Identify NAL unit types and pass on the NALU, trackId, presentation and
    /// decode timestamps.
    /// Also, preprocess caption and sequence parameter NALUs.
    fn on_nal_unit(&mut self, data: Vec<u8>, track_id: Pid, pts: u32, dts: u32) -> ParsedNalUnit {
        let nal_unit_type_code = if data.is_empty() { 0 } else { data[0] & 0x1f };
        match nal_unit_type_code {
            0x05 => {
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::SliceLayerWo(parsed)
            }
            0x06 => {
                let rbsp_data = self.discard_emulation_prevention_bytes(&data[1..]);
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::SeiRbsp(
                    parsed,
                    Rbsp {
                        escaped_rbsp: rbsp_data,
                    },
                )
            }
            0x07 => {
                let rbsp_data = self.discard_emulation_prevention_bytes(&data[1..]);
                let props = self.read_sequence_parameter_set(&rbsp_data);
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::SeqParamSetRbsp(
                    parsed,
                    Rbsp {
                        escaped_rbsp: rbsp_data,
                    },
                    props,
                )
            }
            0x08 => {
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::PicParamSet(parsed)
            }
            0x09 => {
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::AccessUnitDelim(parsed)
            }
            _ => {
                let parsed = UnitTypeBase {
                    track_id,
                    pts,
                    dts,
                    data,
                };
                ParsedNalUnit::Undefined(parsed)
            }
        }
    }

    fn flush(&mut self) -> Option<ParsedNalUnit> {
        let last_nal_unit = self.nal_unit_finder.flush();
        match (
            last_nal_unit,
            self.last_track_id,
            self.last_pts,
            self.last_dts,
        ) {
            (Some(data), Some(track_id), Some(pts), Some(dts)) => {
                Some(self.on_nal_unit(data, track_id, pts, dts))
            }
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.nal_unit_finder.reset();
    }

    /// Expunge any "Emulation Prevention" bytes from a "Raw Byte
    /// Sequence Payload"
    fn discard_emulation_prevention_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        let length = data.len();
        let mut emulation_prevention_bytes_positions = vec![];
        let mut i = 1;

        // Find all `Emulation Prevention Bytes`
        while i < length - 2 {
            if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0x03 {
                emulation_prevention_bytes_positions.push(i + 2);
                i += 2;
            } else {
                i += 1;
            }
        }

        // If no Emulation Prevention Bytes were found just return the original
        // array
        if emulation_prevention_bytes_positions.is_empty() {
            return data.to_owned();
        }

        // Create a new array to hold the NAL unit data
        let new_len = length - emulation_prevention_bytes_positions.len();
        let mut source_idx = 0;
        let mut emu_idx = 0;
        (0..new_len)
            .map(|_| {
                if source_idx == emulation_prevention_bytes_positions[emu_idx] {
                    // Skip this byte
                    source_idx += 1;
                    emu_idx += 1;
                }
                let datum = data[source_idx];
                source_idx += 1;
                datum
            })
            .collect()
    }

    /// Read a sequence parameter set and return some interesting video
    /// properties. A sequence parameter set is the H264 metadata that
    /// describes the properties of upcoming video frames.
    fn read_sequence_parameter_set(&mut self, data: &[u8]) -> NalVideoProperties {
        let mut frame_crop_left_offset = 0;
        let mut frame_crop_right_offset = 0;
        let mut frame_crop_top_offset = 0;
        let mut frame_crop_bottom_offset = 0;
        let mut sar_ratio = (1, 1);

        let mut exp_golomb_decoder = ExpGolomb::new(data);
        let profile_idc = exp_golomb_decoder.read_unsigned_byte(); // profile_idc
                                                                   // constraint_set[0-5]_flag
        let profile_compatibility = exp_golomb_decoder.read_unsigned_byte();
        let level_idc = exp_golomb_decoder.read_unsigned_byte(); // level_idc u(8)
        exp_golomb_decoder.skip_unsigned(); // seq_parameter_set_id

        // some profiles have more optional data we don't need
        if PROFILES_WITH_OPTIONAL_SPS_DATA.contains(&profile_idc) {
            let chroma_format_idc = exp_golomb_decoder.read_unsigned();
            if chroma_format_idc == 3 {
                exp_golomb_decoder.skip_bits(1); // separate_colour_plane_flag
            }
            exp_golomb_decoder.skip_unsigned(); // bit_depth_luma_minus8
            exp_golomb_decoder.skip_unsigned(); // bit_depth_chroma_minus8
            exp_golomb_decoder.skip_bits(1); // qpprime_y_zero_transform_bypass_flag
            if exp_golomb_decoder.read_boolean() {
                // seq_scaling_matrix_present_flag
                let scaling_list_count = if chroma_format_idc != 3 { 8 } else { 12 };
                for i in 0..scaling_list_count {
                    if exp_golomb_decoder.read_boolean() {
                        // seq_scaling_list_present_flag[ i ]
                        if i < 6 {
                            self.skip_scaling_list(16, &mut exp_golomb_decoder);
                        } else {
                            self.skip_scaling_list(64, &mut exp_golomb_decoder);
                        }
                    }
                }
            }
        }

        exp_golomb_decoder.skip_unsigned(); // log2_max_frame_num_minus4
        let pic_order_cnt_type = exp_golomb_decoder.read_unsigned();

        if pic_order_cnt_type == 0 {
            exp_golomb_decoder.read_unsigned(); // log2_max_pic_order_cnt_lsb_minus4
        } else if pic_order_cnt_type == 1 {
            exp_golomb_decoder.skip_bits(1); // delta_pic_order_always_zero_flag
            exp_golomb_decoder.skip_signed(); // offset_for_non_ref_pic
            exp_golomb_decoder.skip_signed(); // offset_for_top_to_bottom_field
            let num_ref_frames_in_pic_order_cnt_cycle = exp_golomb_decoder.read_unsigned();
            for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                exp_golomb_decoder.skip_signed(); // offset_for_ref_frame[ i ]
            }
        }

        exp_golomb_decoder.skip_unsigned(); // max_num_ref_frames
        exp_golomb_decoder.skip_bits(1); // gaps_in_frame_num_value_allowed_flag

        let pic_width_in_mbs_minus_1 = exp_golomb_decoder.read_unsigned();
        let pic_height_in_mbs_minus_1 = exp_golomb_decoder.read_unsigned();

        let frame_mbs_only_flag = exp_golomb_decoder.read_bits(1);
        if frame_mbs_only_flag == 0 {
            exp_golomb_decoder.skip_bits(1); // mb_adaptive_frame_field_flag
        }

        exp_golomb_decoder.skip_bits(1); // direct_8x8_inference_flag
        if exp_golomb_decoder.read_boolean() {
            // frame_cropping_flag
            frame_crop_left_offset = exp_golomb_decoder.read_unsigned();
            frame_crop_right_offset = exp_golomb_decoder.read_unsigned();
            frame_crop_top_offset = exp_golomb_decoder.read_unsigned();
            frame_crop_bottom_offset = exp_golomb_decoder.read_unsigned();
        }

        if exp_golomb_decoder.read_boolean() {
            // vui_parameters_present_flag
            if exp_golomb_decoder.read_boolean() {
                // aspect_ratio_info_present_flag
                let aspect_ratio_idc = exp_golomb_decoder.read_unsigned_byte();
                sar_ratio = match aspect_ratio_idc {
                    1 => (1, 1),
                    2 => (12, 11),
                    3 => (10, 11),
                    4 => (16, 11),
                    5 => (40, 33),
                    6 => (24, 11),
                    7 => (20, 11),
                    8 => (32, 11),
                    9 => (80, 33),
                    10 => (18, 11),
                    11 => (15, 11),
                    12 => (64, 33),
                    13 => (160, 99),
                    14 => (4, 3),
                    15 => (3, 2),
                    16 => (2, 1),
                    255 => (
                        ((exp_golomb_decoder.read_unsigned_byte() as u16) << 8)
                            | (exp_golomb_decoder.read_unsigned_byte() as u16),
                        ((exp_golomb_decoder.read_unsigned_byte() as u16) << 8)
                            | (exp_golomb_decoder.read_unsigned_byte() as u16),
                    ),
                    _ => (1, 1),
                }
            }
        }

        let width = (pic_width_in_mbs_minus_1 + 1) * 16
            - frame_crop_left_offset * 2
            - frame_crop_right_offset * 2;

        let height = (2 - frame_mbs_only_flag) * (pic_height_in_mbs_minus_1 + 1) * 16
            - frame_crop_top_offset * 2
            - frame_crop_bottom_offset * 2;

        NalVideoProperties {
            profile_idc,
            level_idc,
            profile_compatibility,
            width,
            height,
            // sar is sample aspect ratio
            sar_ratio,
        }
    }

    /// Advance the ExpGolomb decoder past a scaling list. The scaling
    /// list is optionally transmitted as part of a sequence parameter
    /// set and is not relevant to transmuxing.
    /// @see Recommendation ITU-T H.264, Section 7.3.2.1.1.1
    fn skip_scaling_list(&self, count: usize, exp_golomb_decoder: &mut ExpGolomb) {
        let mut last_scale = 8;
        let mut next_scale = 8;

        for _ in 0..count {
            if next_scale != 0 {
                let delta_scale = exp_golomb_decoder.read_signed();
                next_scale = (last_scale + delta_scale + 256) % 256;
            }

            last_scale = if next_scale == 0 {
                last_scale
            } else {
                next_scale
            };
        }
    }
}
