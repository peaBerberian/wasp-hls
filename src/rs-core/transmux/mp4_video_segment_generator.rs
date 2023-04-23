use super::{nal_unit_producer::ParsedNalUnit, frame_utils::{GopsSet, extend_first_key_frame, group_nals_into_frames, group_frames_into_gops, concatenate_nal_data}, track_dts_info::TrackDtsInfo, fmp4::{generate_sample_table, create_mdat, create_moof}};

/// Constructs a single-track, ISO BMFF media segment from H264 data.
pub(super) struct Mp4VideoSegmentGenerator {
  sequence_number: u32,
  nal_units: Vec<ParsedNalUnit>,
  gops_to_align_with: Vec<GopsSet>,
  has_parsed_track_info: bool,
  pps: Option<Vec<u8>>,
  should_align_gops_at_end: bool,
  keep_original_timestamps: bool,
  dts_track_info: TrackDtsInfo,
  current_track_info: Option<TrackMetadata>,
}

struct TrackMetadata {
    sps: Vec<u8>,
    width: u32,
    height: u32,
    profile_idc: u8,
    level_idc: u8,
    profile_compatibility: u8,
    sar_ratio: (u16, u16),
}

fn u8_equals(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        false
    } else {
        a.iter()
            .zip(b)
            .all(|(a,b)| a == b)
    }
}

impl Mp4VideoSegmentGenerator {
    /// Create a new `Mp4VideoSegmentGenerator`
    /// 
    /// # Arguments
    ///
    /// * `track_info`
    ///
    /// * `first_sequence_number`
    ///
    /// * `align_gops_at_end` - If true, start from the end of the gops_to_align_with list when
    /// attempting to align gop pts
    ///
    /// * `keep_original_timestamps`: If true, keep the timestamps in the source; false to adjust
    /// the first segment to start at 0.
    pub(super) fn new(
        track_info: TrackDtsInfo,
        first_sequence_number: u32,
        align_gops_at_end: bool,
        keep_original_timestamps: bool
    ) -> Self {
        Self {
            sequence_number: first_sequence_number,
            should_align_gops_at_end: align_gops_at_end,
            keep_original_timestamps,
            dts_track_info: track_info,
            nal_units: vec![],
            gops_to_align_with: vec![],
            has_parsed_track_info: false,
            pps: None,
            current_track_info: None,
        }
    }

  /// Push new Nal Unit to the `Mp4VideoSegmentGenerator` which will be used to
  /// generate a video segment once `generateBoxes` is called (once all Nal
  /// Units) of a segment have been pushed.
  pub(super) fn push_nal_unit(&mut self, nal_unit: ParsedNalUnit) {
      self.dts_track_info.collect_info_from_nal(nal_unit);


      if self.current_track_info.is_none() {
          if let ParsedNalUnit::SeqParamSetRbsp(base, rbsp_data, nal_props) = nal_unit {
              self.current_track_info = Some(TrackMetadata {
                  width: nal_props.width(),
                  height: nal_props.height(),
                  profile_idc: nal_props.profile_idc(),
                  level_idc: nal_props.level_idc(),
                  profile_compatibility: nal_props.profile_compatibility(),
                  sar_ratio: nal_props.sar_ratio(),
                  sps: base.data().to_owned(),
              });
          }
      }

    if self.pps.is_none() {
        if let ParsedNalUnit::PicParamSet(base) = nal_unit {
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
    let nal_units = std::mem::replace(&mut self.nal_units, vec![]);
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
    if gops.gops().first().and_then(|g| g.frames().first()).map(|f| f.key_frame()) == Some(false) {
        // If we didn't find a candidate gop fall back to keyframe-pulling
        gops = extend_first_key_frame(gops);
    }

    // Trim gops to align with gopsToAlignWith
    if !self.gops_to_align_with.is_empty() {
      let mut aligned_gops: any;
      if self.should_align_gops_at_end {
        aligned_gops = self._alignGopsAtEnd(gops);
      } else {
        aligned_gops = self._alignGopsAtStart(gops);
      }

      if (aligned_gops === null) {
        // Clear nalUnits
        self.nal_units = [];

        // return early no gops can be aligned with desired gopsToAlignWith
        self.reset_stream();
        return null;
      }

      // Some gops were trimmed. clear dts info so minSegmentDts and pts are correct
      // when recalculated.
      self.dts_track_info.clear_info();

      gops = aligned_gops;
    }

    self.dts_track_info.collect_info_from_gops(gops);

    //     XXX TODO needed?
    // // First, we have to build the index from byte locations to
    // // samples (that is, frames) in the video data
    // self.dts_track_info.samples = generate_sample_table(gops, 0);

    // Concatenate the video data and construct the mdat
    let mdat = create_mdat(concatenate_nal_data(gops));

    self.dts_track_info.set_base_media_decode_time(self.dts_track_info.calculate_base_media_decode_time(
      self.keep_original_timestamps,
      None
    ));

    let moof = create_moof(self.sequence_number, [self.dts_track_info]);

    // it would be great to allocate self array up front instead of
    // throwing away hundreds of media segment fragments
    let boxes = Vec::with_capacity(moof.len() + mdat.len());
    boxes.extend(moof);
    boxes.extend(mdat);

    // Bump the sequence number for next time
    self.sequence_number += 1;
    let trackInfo = { ...self.dts_track_info };
    self.reset_stream();
    return { trackInfo, boxes };
  }

  pub(super) fn cancel(&mut self) {
    self.reset_stream();
    self.nal_units = vec![];
    self.gops_to_align_with = vec![];
  }

  pub(super) fn reset_stream(&mut self) {
      self.dts_track_info.clear_info();

      // reset config and pps because they may differ across segments
      // for instance, when we are rendition switching
      self.has_parsed_track_info = false;
      self.pps = None;
  }

  // trim gop list to the first gop found that has a matching pts with a gop in the list
  // of gopsToAlignWith starting from the START of the list
  pub(super) fn _alignGopsAtStart(gops: any): any {
    let mut byteLength = gops.byteLength;
    let mut nalCount = gops.nalCount;
    let mut duration = gops.duration;
    let mut alignIndex = 0;
    let mut gopIndex = 0;

    while (
      alignIndex < self.gops_to_align_with.length &&
      gopIndex < gops.length
    ) {
      let align = self.gops_to_align_with[alignIndex];
      let gop = gops[gopIndex];

      if (align.pts === gop.pts) {
        break;
      }

      if (gop.pts > align.pts) {
        // self current gop starts after the current gop we want to align on, so increment
        // align index
        alignIndex++;
        continue;
      }

      // current gop starts before the current gop we want to align on. so increment gop
      // index
      gopIndex++;
      byteLength -= gop.byteLength;
      nalCount -= gop.nalCount;
      duration -= gop.duration;
    }

    if (gopIndex === 0) {
      // no gops to trim
      return gops;
    }

    if (gopIndex === gops.length) {
      // all gops trimmed, skip appending all gops
      return null;
    }

    let aligned_gops = gops.slice(gopIndex);
    aligned_gops.byteLength = byteLength;
    aligned_gops.duration = duration;
    aligned_gops.nalCount = nalCount;
    aligned_gops.pts = aligned_gops[0].pts;
    aligned_gops.dts = aligned_gops[0].dts;

    return aligned_gops;
  }

  // trim gop list to the first gop found that has a matching pts with a gop in the list
  // of gopsToAlignWith starting from the END of the list
  private _alignGopsAtEnd(gops: any): any {
    let mut alignIndex = self.gops_to_align_with.length - 1;
    let mut gopIndex = gops.length - 1;
    let mut alignEndIndex: number | null = null;
    let mut matchFound = false;

    while (alignIndex >= 0 && gopIndex >= 0) {
      let align = self.gops_to_align_with[alignIndex];
      let gop = gops[gopIndex];

      if (align.pts === gop.pts) {
        matchFound = true;
        break;
      }

      if (align.pts > gop.pts) {
        alignIndex--;
        continue;
      }

      if (alignIndex === self.gops_to_align_with.length - 1) {
        // gop.pts is greater than the last alignment candidate. If no match is found
        // by the end of this loop, we still want to append gops that come after this
        // point
        alignEndIndex = gopIndex;
      }

      gopIndex--;
    }

    if (!matchFound && alignEndIndex === null) {
      return null;
    }

    let mut trimIndex: number | null;
    if (matchFound) {
      trimIndex = gopIndex;
    } else {
      trimIndex = alignEndIndex;
    }

    if (trimIndex === 0) {
      return gops;
    }

    let aligned_gops = gops.slice(trimIndex);
    let metadata = aligned_gops.reduce(
      (total: any, gop: any): any => {
        total.byteLength += gop.byteLength;
        total.duration += gop.duration;
        total.nalCount += gop.nalCount;
        return total;
      },
      { byteLength: 0, duration: 0, nalCount: 0 }
    );

    aligned_gops.byteLength = metadata.byteLength;
    aligned_gops.duration = metadata.duration;
    aligned_gops.nalCount = metadata.nalCount;
    aligned_gops.pts = aligned_gops[0].pts;
    aligned_gops.dts = aligned_gops[0].dts;

    return aligned_gops;
  }

  pub(super) fn alignGopsWith(newGopsToAlignWith: any) {
    self.gops_to_align_with = newGopsToAlignWith;
  }
}

