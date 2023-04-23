// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

const ONE_SECOND_IN_TS: u32 = 90000;

pub(super) fn seconds_to_video_ts(seconds: u32) -> u32 {
    seconds * ONE_SECOND_IN_TS
}

pub(super) fn seconds_to_audio_ts(seconds: u32, sample_rate: u32) -> u32 {
    seconds * sample_rate
}

pub(super) fn video_ts_to_seconds(timestamp: u32) -> u32 {
    timestamp / ONE_SECOND_IN_TS
}

pub(super) fn audio_ts_to_seconds(timestamp: u32, sample_rate: u32) -> u32 {
    timestamp / sample_rate
}

pub(super) fn audio_ts_to_video_ts(timestamp: u32, sample_rate: u32) -> u32 {
    seconds_to_video_ts(audio_ts_to_seconds(timestamp, sample_rate))
}

pub(super) fn video_ts_to_audio_ts(timestamp: u32, sample_rate: u32) -> u32 {
    seconds_to_audio_ts(video_ts_to_seconds(timestamp), sample_rate)
}

/**
 * Adjust ID3 tag or caption timing information by the timeline pts values
 * (if keepOriginalTimestamps is false) and convert to seconds
 */
pub(super) fn metadata_ts_to_seconds(
    timestamp: u32,
    timeline_start_pts: u32,
    keep_original_timestamps: bool,
) -> u32 {
    video_ts_to_seconds(if keep_original_timestamps {
        timestamp
    } else {
        timestamp - timeline_start_pts
    })
}
