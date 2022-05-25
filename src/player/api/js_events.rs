use crate::{
    wasm_bindgen,
    js_functions::{RequestId, SourceBufferId},
    requester::FinishedRequestType,
    Logger,
};

use super::super::{MediaSourceReadyState, WaspHlsPlayer};

/// Methods triggered on JavaScript events by the JavaScript code
#[wasm_bindgen]
impl WaspHlsPlayer {
    /// Called by the JavaScript code each time an HTTP(S) request started with
    /// `jsFetchU8` finished with success.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by the JavaScript when the request
    ///   was started. This allows the `WaspHlsPlayer` to identify which request
    ///   is actually finished
    ///
    /// * `result` - The data returned.
    pub fn on_u8_request_finished(&mut self, request_id: RequestId, result: Vec<u8>) {
        match self.requester.remove_pending_request(request_id) {
            Some(FinishedRequestType::Segment(seg_info)) =>
                self.on_segment_fetch_success(seg_info, result),
            Some(FinishedRequestType::Playlist(pl_info)) =>
                self.on_playlist_fetch_success(pl_info, result),
            _ => {
                Logger::warn("Unknown request finished");
            },
        }
    }

    pub fn on_u8_request_failed(&mut self) {
        // TODO retry and whatnot
        self.fail_on_error("A segment request failed.");
    }

    /// Called by the JavaScript code when the MediaSource's readyState changed.
    ///
    /// # Arguments
    ///
    /// * `state` - The new `readyState` of the MediaSource.
    pub fn on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        self.internal_on_media_source_state_change(state);
    }

    /// Called by the JavaScript code when a SourceBuffer emits an `updateend`
    /// event.
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given by the JavaScript when the
    ///   SourceBuffer was created. This allows the `WaspHlsPlayer` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        if let Some(ref mut sb) = self.source_buffer_store.audio {
            if sb.id == source_buffer_id {
                sb.on_update_end();
                return;
            }
        }
        if let Some(ref mut sb) = self.source_buffer_store.video {
            if sb.id == source_buffer_id {
                sb.on_update_end();
                return;
            }
        }
    }

    pub fn on_source_buffer_error(&mut self) {
        // TODO check QuotaExceededError and so on...
        self.fail_on_error("A SourceBuffer emitted an error");
    }

    /// Called by the JavaScript code once regular playback "tick" are enabled
    /// after the `jsStartObservingPlayback` function has been called.
    /// This function will be continuously called at each important media events
    /// (seek operations, end of the streams, known stalls etc.) until
    /// `jsStopObservingPlayback` is called.
    pub fn on_playback_tick(&mut self,
        reason: PlaybackTickReason,
        position: f64) {
        match reason {
            PlaybackTickReason::Seeking => self.on_seek(position),
            _ => self.on_regular_tick(position),
        }
    }
}

#[wasm_bindgen]
pub enum PlaybackTickReason {
    Init,
    Seeking,
    Seeked,
    RegularInterval,
}
