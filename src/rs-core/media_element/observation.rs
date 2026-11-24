use crate::bindings::{JsTimeRanges, PlaybackTickReason};

pub struct MediaObservation {
    last_media_command_number: u64,
    reason: PlaybackTickReason,
    current_time: f64,
    ready_state: u8,
    buffered: JsTimeRanges,
    paused: bool,
    seeking: bool,
    ended: bool,
    duration: f64,
    audio_buffered: Option<JsTimeRanges>,
    video_buffered: Option<JsTimeRanges>,
}

impl MediaObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reason: PlaybackTickReason,
        current_time: f64,
        ready_state: u8,
        buffered: JsTimeRanges,
        paused: bool,
        seeking: bool,
        ended: bool,
        duration: f64,
        audio_buffered: Option<JsTimeRanges>,
        video_buffered: Option<JsTimeRanges>,
        last_media_command_number: u64,
    ) -> Self {
        Self {
            last_media_command_number,
            reason,
            current_time,
            ready_state,
            buffered,
            paused,
            seeking,
            ended,
            duration,
            audio_buffered,
            video_buffered,
        }
    }
}

impl MediaObservation {
    /// Latest worker media command received before this snapshot was captured.
    #[inline(always)]
    pub fn last_media_command_number(&self) -> u64 {
        self.last_media_command_number
    }

    #[inline(always)]
    pub fn reason(&self) -> PlaybackTickReason {
        self.reason
    }

    #[inline(always)]
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    #[inline(always)]
    pub fn ready_state(&self) -> u8 {
        self.ready_state
    }

    #[inline(always)]
    pub fn buffered(&self) -> &JsTimeRanges {
        &self.buffered
    }

    #[inline(always)]
    pub fn paused(&self) -> bool {
        self.paused
    }

    #[inline(always)]
    pub fn seeking(&self) -> bool {
        self.seeking
    }

    #[inline(always)]
    pub fn ended(&self) -> bool {
        self.ended
    }

    #[inline(always)]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    #[inline(always)]
    pub fn audio_buffered(&self) -> Option<&JsTimeRanges> {
        self.audio_buffered.as_ref()
    }

    #[inline(always)]
    pub fn video_buffered(&self) -> Option<&JsTimeRanges> {
        self.video_buffered.as_ref()
    }
}
