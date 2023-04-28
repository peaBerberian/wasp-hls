use super::{
    fmp4::{create_mdat, create_moof, generate_sample_table, IsobmffMetadata, TrackInfo},
    frame_utils::{
        concatenate_nal_data, extend_first_key_frame, group_frames_into_gops,
        group_nals_into_frames,
    },
    nal_unit_producer::{NalVideoProperties, ParsedNalUnit},
    track_dts_info::TrackDtsInfo,
    transport_packet_parser::Pid,
};

/// Constructs a single-track, ISO BMFF media segment from H264 data.
pub(super) struct Mp4VideoSegmentGenerator {
    sequence_number: u32,
    nal_units: Vec<ParsedNalUnit>,
    has_parsed_track_info: bool,
    keep_original_timestamps: bool,
    track_dts_info: TrackDtsInfo,
    track_info: Option<Mp4VideoTrackInfo>,
    pps: Option<Vec<u8>>,
}

struct Mp4VideoTrackInfo {
    nal_video_properties: NalVideoProperties,
    sps: Vec<u8>,
    track_id: Pid,
}

fn u8_equals(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        false
    } else {
        a.iter().zip(b).all(|(a, b)| a == b)
    }
}

pub(super) struct Mp4VideoSegmentData {
    data: Vec<u8>,
}

impl Mp4VideoSegmentGenerator {
    /// Create a new `Mp4VideoSegmentGenerator`
    ///
    /// # Arguments
    ///
    /// * `first_sequence_number`
    ///
    /// * `keep_original_timestamps`: If true, keep the timestamps in the source; false to adjust
    /// the first segment to start at 0.
    pub(super) fn new(first_sequence_number: u32, keep_original_timestamps: bool) -> Self {
        Self {
            sequence_number: first_sequence_number,
            keep_original_timestamps,
            track_dts_info: TrackDtsInfo::default(),
            nal_units: vec![],
            has_parsed_track_info: false,
            track_info: None,
            pps: None,
        }
    }

    /// Push new Nal Unit to the `Mp4VideoSegmentGenerator` which will be used to
    /// generate a video segment once `generateBoxes` is called (once all Nal
    /// Units) of a segment have been pushed.
    pub(super) fn push_nal_unit(&mut self, nal_unit: ParsedNalUnit) {
        self.track_dts_info.collect_info_from_nal(&nal_unit);

        if self.track_info.is_none() {
            if let ParsedNalUnit::SeqParamSetRbsp(base, _, nal_props) = &nal_unit {
                self.track_info = Some(Mp4VideoTrackInfo {
                    nal_video_properties: nal_props.clone(),
                    track_id: base.track_id(),
                    sps: base.data().to_owned(),
                });
            }
        }

        if self.pps.is_none() {
            if let ParsedNalUnit::PicParamSet(base) = &nal_unit {
                self.pps = Some(base.data().to_owned());
            }
        }
        self.nal_units.push(nal_unit);
    }

    /// Generate ISOBMFF data for the video segment from the Nal Unit pushed thus
    /// far.
    /// Returns `None` if no segment could have been generated.
    pub(super) fn generate_boxes(&mut self) -> Option<Mp4VideoSegmentData> {
        // Throw away nal_units at the start of the byte stream until
        // we find the first AUD
        while !self.nal_units.is_empty() {
            if let ParsedNalUnit::AccessUnitDelim(_) = self.nal_units[0] {
                break;
            }
            self.nal_units.remove(0);
        }

        // Return early if no video data has been observed
        if self.nal_units.is_empty() {
            self.reset_stream();
            return None;
        }

        // Organize the raw nal-units into arrays that represent
        // higher-level constructs such as frames and gops
        // (group-of-pictures)
        let nal_units = std::mem::take(&mut self.nal_units);
        let frames = group_nals_into_frames(nal_units);
        let mut gops = group_frames_into_gops(frames);

        // If the first frame of self fragment is not a keyframe we have a problem since MSE (on Chrome)
        // requires a leading keyframe.
        //
        // Here we search for the first keyframe in the fragment and throw away all the frames between
        // the start of the fragment and that keyframe.
        // We then extend the duration and pull the PTS of the keyframe forward so that it covers the
        // range of the frames that were disposed of.
        //
        // It can create stuttering but those contents should be rare enough anyway
        if gops
            .gops()
            .first()
            .and_then(|g| g.frames().first())
            .map(|f| f.key_frame())
            == Some(false)
        {
            // If we didn't find a candidate gop fall back to keyframe-pulling
            extend_first_key_frame(&mut gops);
        }

        self.track_dts_info.collect_info_from_gops(&gops);

        // First, we have to build the index from byte locations to
        // samples (that is, frames) in the video data
        let samples = generate_sample_table(&gops, 0);

        // Concatenate the video data and construct the mdat
        let mdat = create_mdat(concatenate_nal_data(gops));

        let base_media_decode_time = self
            .track_dts_info
            .calculate_base_media_decode_time(self.keep_original_timestamps, None);
        self.track_dts_info
            .set_base_media_decode_time(base_media_decode_time);

        let (Some(track_info), Some(pps)) = (&self.track_info, &self.pps) else {
        return None;
    };

        let video_md = IsobmffMetadata::new_video(
            track_info.nal_video_properties.clone(),
            vec![track_info.sps.clone()],
            vec![pps.clone()],
        );
        let track_info = TrackInfo {
            md: video_md,
            track_id: track_info.track_id as u32,
            base_media_decode_time,
            duration: None,
            samples,
            sample_rate: None,
        };
        let moof = create_moof(self.sequence_number, &[track_info]);

        // it would be great to allocate self array up front instead of
        // throwing away hundreds of media segment fragments
        let mut boxes = Vec::with_capacity(moof.len() + mdat.len());
        boxes.extend(moof);
        boxes.extend(mdat);

        // Bump the sequence number for next time
        self.sequence_number += 1;
        self.reset_stream();
        Some(Mp4VideoSegmentData { data: boxes })
    }

    pub(super) fn cancel(&mut self) {
        self.reset_stream();
        self.nal_units = vec![];
    }

    pub(super) fn reset_stream(&mut self) {
        self.track_dts_info.clear_info();

        // reset config and pps because they may differ across segments
        // for instance, when we are rendition switching
        self.has_parsed_track_info = false;
        self.pps = None;
    }
}
