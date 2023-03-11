use crate::{
    wasm_bindgen,
    dispatcher::{MediaSourceReadyState, Dispatcher}, utils::url::Url,
};

use super::{js_functions::{self, RequestId, SourceBufferId}, TimerReason, TimerId, jsGetResourceData, ResourceId};

/// Methods triggered on JavaScript events by the JavaScript code
///
/// Those functions are voluntarly written a certain way to put in evidence that
/// those should just be bindings converting to the right types without directly
/// interacting with the `Dispatcher`'s state (e.g. methods are called with
/// an explicit `Dispatcher` reference).
#[wasm_bindgen]
impl Dispatcher {
    /// Called by the JavaScript code each time an HTTP(S) request started with
    /// `jsFetch` finished with success.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by the JavaScript when the request
    ///   was started. This allows the `Dispatcher` to identify which request
    ///   is actually finished
    ///
    /// * `resource_id` - Id refering to the resource on the JavaScript-side.
    pub fn on_request_finished(&mut self,
        request_id: RequestId,
        resource_id: ResourceId,
        resource_size: u32,
        final_url: String,
        duration_ms: f64 // TODO Rust-side?
    ) {
        let resource_handle = JsMemoryBlob::from_resource_id(resource_id);
        Dispatcher::on_request_succeeded(self, request_id,
            resource_handle,
            Url::new(final_url),
            resource_size,
            duration_ms);
    }

    /// Called by the JavaScript code each time an HTTP(S) request started with
    /// `jsFetch` finished with an error.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by the JavaScript when the request
    ///   was started. This allows the `Dispatcher` to identify which request
    ///   is actually finished
    pub fn on_request_failed(&mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>
    ) {
        Dispatcher::on_request_failed_inner(self, request_id, has_timeouted, status);
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

    /// Called by the JavaScript code when a SourceBuffer emits an `error`
    /// event.
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given by the JavaScript when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_source_buffer_error(&mut self, source_buffer_id: SourceBufferId) {
        Dispatcher::internal_on_source_buffer_error(self, source_buffer_id);
    }

    /// Called by the JavaScript code once regular playback "tick" are enabled
    /// after the `jsStartObservingPlayback` function has been called.
    ///
    /// This function will be continuously called at each important media events
    /// (seek operations, end of the streams, known stalls etc.) until
    /// `jsStopObservingPlayback` is called.
    pub fn on_playback_tick(&mut self, observation: MediaObservation) {
        Dispatcher::on_observation(self, observation);
    }

    /// Called by the JavaScript code each time a timer started with the `jsTimer`
    /// function finished.
    ///
    /// # Arguments
    ///
    /// * `id` - The `TimerId` given by the JavaScript when the timer was
    ///   started. This allows the `Dispatcher` to identify which timer
    ///   actually finished.
    ///
    /// * `reason` - The `TimerReason` given by the Rust code when that timer
    ///   was started. Using this supplementary attribute allows to better
    ///   discriminate between timers used for different purposes and thus
    ///   to simplify the logic handling a resolved timer.
    pub fn on_timer_ended(&mut self, id: TimerId, reason: TimerReason) {
        match reason {
            TimerReason::MediaPlaylistRefresh =>
                Dispatcher::on_playlist_refresh_timer_ended(self, id),
            TimerReason::RetryRequest =>
                Dispatcher::on_retry_request(self, id),
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

pub struct JsMemoryBlob {
    id: ResourceId,
}

/// Special structure to handle data
impl JsMemoryBlob {
    pub fn from_resource_id(id: ResourceId) -> Self {
        Self { id }
    }

    pub fn get_id(&self) -> ResourceId {
        self.id
    }

    pub fn obtain(self) -> Vec<u8> {
        jsGetResourceData(self.id).unwrap()
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
    ended: bool,
    duration: f64,
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
        ended: bool,
        duration: f64,
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
            ended,
            duration,
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

    #[inline(always)]
    pub fn ended(&self) -> bool {
        self.ended
    }

    #[inline(always)]
    pub fn duration(&self) -> f64 {
        self.duration
    }
}
