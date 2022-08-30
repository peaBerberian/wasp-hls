use crate::{
    wasm_bindgen,
    dispatcher::{MediaSourceReadyState, Dispatcher}, utils::url::Url,
};

use super::js_functions::{self, RequestId, SourceBufferId};

/// Methods triggered on JavaScript events by the JavaScript code
///
/// Those functions are voluntarly written a certain way to put in evidence that
/// those should just be bindings converting to the right types without directly
/// interacting with the `Dispatcher`'s state (e.g. methods are called with
/// an explicit `Dispatcher` reference).
#[wasm_bindgen]
impl Dispatcher {
    /// Called by the JavaScript code each time an HTTP(S) request started with
    /// `jsFetchU8` finished with success.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by the JavaScript when the request
    ///   was started. This allows the `Dispatcher` to identify which request
    ///   is actually finished
    ///
    /// * `result` - The data returned.
    pub fn on_u8_request_finished(&mut self,
        request_id: RequestId,
        result: Vec<u8>,
        final_url: String,
        duration_ms: f64 // TODO Rust-side?
    ) {
        let resource_size = result.len() as u32;
        Dispatcher::on_request_succeeded(self,
            request_id,
            DataSource::Raw(result),
            Url::new(final_url),
            resource_size,
            duration_ms);
    }

    pub fn on_u8_no_copy_request_finished(&mut self,
        request_id: RequestId,
        resource_id: u32,
        resource_size: u32,
        final_url: String,
        duration_ms: f64 // TODO Rust-side?
    ) {
        let resource_handle = JsMemoryBlob::from_resource_id(resource_id);
        Dispatcher::on_request_succeeded(self, request_id,
            DataSource::JsBlob(resource_handle),
            Url::new(final_url),
            resource_size,
            duration_ms);
    }

    pub fn on_u8_request_failed(&mut self, request_id: RequestId) {
        Dispatcher::on_request_failed(self, request_id);
    }

    /// Called by the JavaScript code when the MediaSource's readyState changed.
    ///
    /// # Arguments
    ///
    /// * `state` - The new `readyState` of the MediaSource.
    pub fn on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        Dispatcher::internal_on_media_source_state_change(self, state);
    }

    /// Called by the JavaScript code when a SourceBuffer emits an `updateend`
    /// event.
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given by the JavaScript when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        Dispatcher::internal_on_source_buffer_update(self, source_buffer_id);
    }

    pub fn on_source_buffer_error(&mut self, source_buffer_id: SourceBufferId) {
        Dispatcher::internal_on_source_buffer_error(self, source_buffer_id);
    }

    /// Called by the JavaScript code once regular playback "tick" are enabled
    /// after the `jsStartObservingPlayback` function has been called.
    /// This function will be continuously called at each important media events
    /// (seek operations, end of the streams, known stalls etc.) until
    /// `jsStopObservingPlayback` is called.
    pub fn on_playback_tick(&mut self, observation: MediaObservation) {
        match observation.reason {
            PlaybackTickReason::Seeking => Dispatcher::on_seek(self, observation),
            _ => Dispatcher::on_regular_tick(self, observation),
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackTickReason {
    Init,
    Seeking,
    Seeked,
    RegularInterval,
    LoadedData,
    LoadedMetadata,
    CanPlay,
    CanPlayThrough,
    Ended,
    Pause,
    Play,
    RateChange,
    Stalled,
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

#[wasm_bindgen]
pub struct MediaObservation {
    reason: PlaybackTickReason,
    current_time: f64,
    ready_state: u8,
    buffered: Vec<(f64, f64)>,
    paused: bool,
    seeking: bool,
}


#[wasm_bindgen]
impl MediaObservation {
    #[wasm_bindgen(constructor)]
    pub fn new(
        reason: PlaybackTickReason,
        current_time: f64,
        ready_state: u8,
        buffered: &[f64],
        paused: bool,
        seeking: bool,
    ) -> Self {
        assert!(buffered.len() % 2 == 0);
        let mut new_buf = Vec::with_capacity(buffered.len() / 2);
        for i in 0..buffered.len() / 2 {
            let offset = i * 2;
            new_buf.push((buffered[offset], buffered[offset + 1]));
        }
        Self {
            reason,
            current_time,
            ready_state,
            buffered: new_buf,
            paused,
            seeking,
        }
    }
}

impl MediaObservation {
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
    pub fn buffered(&self) -> &[(f64, f64)] {
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
}
