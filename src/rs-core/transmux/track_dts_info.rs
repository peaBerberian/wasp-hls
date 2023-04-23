use super::nal_unit_producer::ParsedNalUnit;

#[derive(Clone, Debug, Default)]
pub(super) struct TrackDtsInfo {
    base_media_decode_time: Option<u32>,
    start_pts: Option<u32>,
    start_dts: Option<u32>,
    min_segment_pts: Option<u32>,
    min_segment_dts: Option<u32>,
    max_segment_pts: Option<u32>,
    max_segment_dts: Option<u32>,
}

impl TrackDtsInfo {
    /// Get information about the start and end of the track and the
    /// duration for each frame/sample we process in order to calculate
    /// the baseMediaDecodeTime.
    pub(super) fn collect_info(&mut self, data: ParsedNalUnit) {
        if self.start_pts.is_none() {
            self.start_pts = Some(data.pts());
        }
        if self.start_dts.is_none() {
            self.start_dts = Some(data.dts());
        }

        if let Some(old_min_pts) = self.min_segment_pts {
            self.min_segment_pts = Some(u32::min(old_min_pts, data.pts()));
        } else {
            self.min_segment_pts = Some(data.pts());
        }

        if let Some(old_max_pts) = self.max_segment_pts {
            self.max_segment_pts = Some(u32::max(old_max_pts, data.pts()));
        } else {
            self.max_segment_pts = Some(data.pts());
        }

        if let Some(old_min_dts) = self.min_segment_dts {
            self.min_segment_dts = Some(u32::min(old_min_dts, data.dts()));
        } else {
            self.min_segment_dts = Some(data.dts());
        }

        if let Some(old_max_dts) = self.max_segment_dts {
            self.max_segment_dts = Some(u32::max(old_max_dts, data.dts()));
        } else {
            self.max_segment_dts = Some(data.dts());
        }
    }

    pub(super) fn start_dts(&self) -> Option<u32> {
        self.start_dts
    }

    pub(super) fn start_pts(&self) -> Option<u32> {
        self.start_dts
    }

    /// Clear values used to calculate the baseMediaDecodeTime between
    /// tracks
    pub(super) fn clear_info(&mut self) {
        self.min_segment_dts = None;
        self.max_segment_dts = None;
        self.min_segment_pts = None;
        self.max_segment_pts = None;
    }

    pub(super) fn set_base_media_decode_time(&mut self, bmdt: u32) {
        self.base_media_decode_time = Some(bmdt);
    }

    /// Calculate the track"s baseMediaDecodeTime based on the earliest
    /// DTS the transmuxer has ever seen and the minimum DTS for the
    /// current track
    pub (super) fn calculate_base_media_decode_time(
        &mut self,
        keep_original_timestamps: bool,
        audio_sample_rate: Option<u32>
    ) -> u32 {
        let mut min_segment_dts = self.min_segment_dts.unwrap_or(0);

        // Optionally adjust the time so the first segment starts at zero.
        if !keep_original_timestamps {
            if let Some(dts) = self.start_dts {
                min_segment_dts -= dts;
            }
        }

        // baseMediaDecodeTime is the location, in time, where we want the start of the first segment to be placed
        let mut base_media_decode_time = self.base_media_decode_time.unwrap_or(0);
        base_media_decode_time += min_segment_dts;

        if let Some(sample_rate) = audio_sample_rate {
            // Audio has a different clock equal to the sampling_rate so we need to
            // scale the PTS values into the clock rate of the track
            let scale = sample_rate / 90000;
            base_media_decode_time *= scale;
        }
        base_media_decode_time
    }
}
