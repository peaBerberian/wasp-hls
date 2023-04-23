use super::{nal_unit_producer::ParsedNalUnit, frame_utils::GopsSet, track_dts_info::TrackDtsInfo};

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

// struct GopCacheInfo {
//     pps: Vec<u8>,
//     sps: Vec<u8>,
//     gops: GopsSet,
// }

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
      self.dts_track_info.collect_info(nal_unit);


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
  pub(super) fn generate_boxes() -> Option<Mp4VideoSegmentData> {
    // Throw away nalUnits at the start of the byte stream until
    // we find the first AUD
    while (self.nal_units.length > 0) {
      if (self.nal_units[0].nalUnitType === NalUnitType.AccessUnitDelim) {
        break;
      }
      self.nal_units.shift();
    }

    // Return early if no video data has been observed
    if (self.nal_units.length === 0) {
      self.reset_stream();
      return null;
    }

    // Organize the raw nal-units into arrays that represent
    // higher-level constructs such as frames and gops
    // (group-of-pictures)
    let frames = groupNalsIntoFrames(self.nal_units);
    let mut gops = groupFramesIntoGops(frames);

    // If the first frame of self fragment is not a keyframe we have
    // a problem since MSE (on Chrome) requires a leading keyframe.
    //
    // We have two approaches to repairing self situation:
    // 1) GOP-FUSION:
    //    self is where we keep track of the GOPS (group-of-pictures)
    //    from previous fragments and attempt to find one that we can
    //    prepend to the current fragment in order to create a valid
    //    fragment.
    // 2) KEYFRAME-PULLING:
    //    Here we search for the first keyframe in the fragment and
    //    throw away all the frames between the start of the fragment
    //    and that keyframe. We then extend the duration and pull the
    //    PTS of the keyframe forward so that it covers the time range
    //    of the frames that were disposed of.
    //
    // #1 is far prefereable over #2 which can cause "stuttering" but
    // requires more things to be just right.
    if (!(gops[0][0].keyFrame as boolean)) {
      // Search for a gop for fusion from our gopCache
      let gopForFusion = self.gop_for_fusion(self.nal_units[0]);

      if (gopForFusion !== null) {
        gops.unshift(gopForFusion);
        // Adjust Gops' metadata to account for the inclusion of the
        // new gop at the beginning
        gops.byteLength += gopForFusion.byteLength;
        gops.nalCount += gopForFusion.nalCount;
        gops.pts = gopForFusion.pts;
        gops.dts = gopForFusion.dts;
        gops.duration += gopForFusion.duration;
      } else {
        // If we didn't find a candidate gop fall back to keyframe-pulling
        gops = extendFirstKeyFrame(gops);
      }
    }

    // Trim gops to align with gopsToAlignWith
    if (self.gops_to_align_with.length) {
      let mut aligned_gops: any;
      if (self.should_align_gops_at_end) {
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
      clearDtsInfo(self.dts_track_info);

      gops = aligned_gops;
    }

    collectDtsInfo(self.dts_track_info, gops);

    // First, we have to build the index from byte locations to
    // samples (that is, frames) in the video data
    self.dts_track_info.samples = generateSampleTable(gops);

    // Concatenate the video data and construct the mdat
    let mdat = createMdat(concatenateNalData(gops));

    self.dts_track_info.baseMediaDecodeTime = calculateTrackBaseMediaDecodeTime(
      self.dts_track_info,
      self.keep_original_timestamps
    );

    // Clear nalUnits
    self.nal_units = [];

    let moof = createMoof(self.sequence_number, [self.dts_track_info]);

    // it would be great to allocate self array up front instead of
    // throwing away hundreds of media segment fragments
    let boxes = new Uint8Array(moof.byteLength + mdat.byteLength);

    // Bump the sequence number for next time
    self.sequence_number++;

    boxes.set(moof);
    boxes.set(mdat, moof.byteLength);

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

//   /// Search for a candidate Gop for gop-fusion from the gop cache and
//   /// return it or return None if no good candidate was found
//   pub(super) fn gop_for_fusion(&mut self, nal_unit: ParsedNalUnit) -> Option<GopsSet> {
//     let half_second = 45000; // Half-a-second in a 90khz clock
//     let allowable_overlap = 10000; // About 3 frames @ 30fps
//     let mut nearest_distance: Option<u32> = None;
//     let mut nearest_go_obj: Option<GopCacheInfo> = None;

//     // Search for the GOP nearest to the beginning of self nal unit
//     self.gop_cache.iter().for_each(|current_gop_obj| {
//       // Reject Gops with different SPS or PPS
//       let Some(pps) = self.pps else {
//           return ;
//       };
//       if !u8_equals(&pps, &current_gop_obj.pps) {
//           return;
//       }
//       let Some(curr) = self.current_track_info else {
//           return ;
//       };
//       if !u8_equals(&curr.sps, &current_gop_obj.sps) {
//           return;
//       }

//       let current_gop = current_gop_obj.gops;

//       // Reject Gops that would require a negative baseMediaDecodeTime
//       let Some(start_dts) = self.dts_track_info.start_dts() else {
//           return;
//       };
//       if current_gop.dts() < start_dts {
//         return;
//       }

//       // The distance between the end of the gop and the start of the nal_unit
//       let dts_distance = nal_unit.dts() - current_gop.dts() - current_gop.duration();

//       // Only consider GOPS that start before the nal unit and end within
//       // a half-second of the nal unit
//       if dts_distance >= -allowable_overlap && dts_distance <= half_second {
//         // Always use the closest GOP we found if there is more than
//         // one candidate
//         if (nearest_go_obj === undefined || nearest_distance > dts_distance) {
//           nearest_go_obj = Some(current_gop_obj.clone());
//           nearest_distance = Some(dts_distance);
//         }
//       }
//     });

//     nearest_go_obj.map(|n| n.gops)
//   }

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

