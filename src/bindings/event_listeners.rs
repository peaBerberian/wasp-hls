use crate::{
    wasm_bindgen,
    player::{MediaSourceReadyState, WaspHlsPlayer},
};

use super::js_functions::{self, RequestId, SourceBufferId};

/// Methods triggered on JavaScript events by the JavaScript code
///
/// Those functions are voluntarly written a certain way to put in evidence that
/// those should just be bindings converting to the right types without directly
/// interacting with the `WaspHlsPlayer`'s state (e.g. methods are called with
/// an explicit `WaspHlsPlayer` reference).
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
        WaspHlsPlayer::on_request_succeeded(self, request_id, DataSource::Raw(result));
    }

    pub fn on_u8_no_copy_request_finished(&mut self,
        request_id: RequestId,
        resource_id: u32
    ) {
        let resource_handle = JsMemoryBlob::from_resource_id(resource_id);
        WaspHlsPlayer::on_request_succeeded(self, request_id, DataSource::JsBlob(resource_handle));
    }

    pub fn on_u8_request_failed(&mut self, request_id: RequestId) {
        WaspHlsPlayer::on_request_failed(self, request_id);
    }

    /// Called by the JavaScript code when the MediaSource's readyState changed.
    ///
    /// # Arguments
    ///
    /// * `state` - The new `readyState` of the MediaSource.
    pub fn on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        WaspHlsPlayer::internal_on_media_source_state_change(self, state);
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
        WaspHlsPlayer::internal_on_source_buffer_update(self, source_buffer_id);
    }

    pub fn on_source_buffer_error(&mut self, source_buffer_id: SourceBufferId) {
        WaspHlsPlayer::internal_on_source_buffer_error(self, source_buffer_id);
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
            PlaybackTickReason::Seeking => WaspHlsPlayer::on_seek(self, position),
            _ => WaspHlsPlayer::on_regular_tick(self, position),
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

pub enum DataSource {
    /// When the Segment to push is available right now in memory.
    Raw(Vec<u8>),
    /// When the Segment to push is only accessible through JavaScript's memory.
    JsBlob(JsMemoryBlob),
}

impl From<Vec<u8>> for DataSource {
    fn from(v: Vec<u8>) -> DataSource {
        DataSource::Raw(v)
    }
}

impl From<JsMemoryBlob> for DataSource {
    fn from(b: JsMemoryBlob) -> DataSource {
        DataSource::JsBlob(b)
    }
}

pub struct JsMemoryBlob {
    id: u32,
}

/// Special structure to handle data
impl JsMemoryBlob {
    pub fn from_resource_id(id: u32) -> Self {
        Self { id }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }
}

impl Drop for JsMemoryBlob {
    fn drop(&mut self) {
        js_functions::jsFreeResource(self.id);
    }
}
