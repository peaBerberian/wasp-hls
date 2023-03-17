use crate::{
    dispatcher::{Dispatcher, MediaSourceReadyState},
    utils::url::Url,
    wasm_bindgen, bindings::{jsGetResourceData, RequestId, ResourceId, SourceBufferId, TimerReason, TimerId, jsFreeResource},
};

/// Methods triggered on JavaScript events by the JavaScript code.
#[wasm_bindgen]
impl Dispatcher {
    /// The JS code should call this method each time an HTTP(S) request started with
    /// `jsFetch` finished with success.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by `jsFetch` when the request
    ///   was started. This allows the `Dispatcher` to identify which request
    ///   is actually finished
    ///
    /// * `resource_id` - Id refering to the resource on the JavaScript-side.
    ///
    /// * `resource_size` - Size of the fetched resource (uncompressed, in
    ///   bytes)
    ///
    /// * `final_url` - Actual url of the content, which may be different from
    ///   the original resource if an HTTP redirect occured
    ///
    /// * `duration_ms` - Number of millisceconds taken to perform the request
    ///   from start to finish.
    pub fn on_request_finished(
        &mut self,
        request_id: RequestId,
        resource_id: ResourceId,
        resource_size: u32,
        final_url: String,
        duration_ms: f64, // TODO Rust-side?
    ) {
        let resource_handle = JsMemoryBlob::from_resource_id(resource_id);
        self.on_request_succeeded(
            request_id,
            resource_handle,
            Url::new(final_url),
            resource_size,
            duration_ms,
        );
    }

    /// The JS code should call this method each time an HTTP(S) request started with
    /// `jsFetch` finished with an error.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The identifier given by `jsFetch` when the request
    ///   was started. This allows the `Dispatcher` to identify which request
    ///   is actually finished
    ///
    /// * `has_timeouted` - If `true`, the issue was due to the request timeouting
    ///   with the current request configuration.
    ///
    /// * `has_timeouted` - If set, the issue was due to a non-satisfying HTTP
    ///   status being received.
    ///   TODO actually categorize that in Rust?
    pub fn on_request_failed(
        &mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>,
    ) {
        self.on_request_failed_core(request_id, has_timeouted, status);
    }

    /// The JS code should call this method when the MediaSource's readyState changed.
    ///
    /// # Arguments
    ///
    /// * `state` - The new `readyState` of the MediaSource.
    pub fn on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        self.on_media_source_state_change_core(state);
    }

    /// The JS code should call this method when a SourceBuffer emits an `updateend`
    /// event.
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier generated when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        self.on_source_buffer_update_core(source_buffer_id);
    }

    /// The JS code should call this method when a SourceBuffer emits an `error`
    /// event.
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given generated when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_source_buffer_error(&mut self, source_buffer_id: SourceBufferId) {
        self.on_source_buffer_error_core(source_buffer_id);
    }

    /// The JS code should call this method once regular playback "tick" are enabled
    /// after the `jsStartObservingPlayback` function has been called.
    ///
    /// This function will be continuously called at each important media events
    /// (seek operations, end of the streams, known stalls etc.) until
    /// `jsStopObservingPlayback` is called.
    pub fn on_playback_tick(&mut self, observation: MediaObservation) {
        self.on_observation(observation);
    }

    /// The JS code should call this method each time a timer started with the `jsTimer`
    /// function finished.
    ///
    /// # Arguments
    ///
    /// * `id` - The `TimerId` given by `jsTimer` when the timer was
    ///   started. This allows the `Dispatcher` to identify which timer
    ///   actually finished.
    ///
    /// * `reason` - The `TimerReason` given by the Rust code when that timer
    ///   was started. Using this supplementary attribute allows to better
    ///   discriminate between timers used for different purposes and thus
    ///   to simplify the logic handling a resolved timer.
    pub fn on_timer_ended(&mut self, id: TimerId, reason: TimerReason) {
        match reason {
            TimerReason::MediaPlaylistRefresh => {
                self.on_playlist_refresh_timer_ended(id)
            }
            TimerReason::RetryRequest => self.on_retry_request(id),
        }
    }

    pub fn on_codecs_support_update(&mut self) {
        self.on_codecs_support_update_core();
    }
}

/// Identify the event that lead to the `MediaObservation` being sent.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackTickReason {
    /// This is the initial observation emitted, right after it was started.
    Init,
    /// This designates MediaObservation sent after an interval without any
    /// of the other events.
    RegularInterval,
    /// The HTMLMediaElement's "seeking" event has just been triggered
    Seeking,
    /// The HTMLMediaElement's "seeked" event has just been triggered
    Seeked,
    /// The HTMLMediaElement's "loadeddata" event has just been triggered
    LoadedData,
    /// The HTMLMediaElement's "loadedmetadata" event has just been triggered
    LoadedMetadata,
    /// The HTMLMediaElement's "canplay" event has just been triggered
    CanPlay,
    /// The HTMLMediaElement's "canplaythrough" event has just been triggered
    CanPlayThrough,
    /// The HTMLMediaElement's "ended" event has just been triggered
    Ended,
    /// The HTMLMediaElement's "pause" event has just been triggered
    Pause,
    /// The HTMLMediaElement's "play" event has just been triggered
    Play,
    /// The HTMLMediaElement's "ratechange" event has just been triggered
    RateChange,
    /// The HTMLMediaElement's "stalled" event has just been triggered
    Stalled,
}

/// Special structure to handle data that is only present in JavaScript's
/// memory.
///
/// The data is identified through a unique `ResourceId` identifier.
///
/// The idea behind this struct is to prevent memory leaks by implementing the
/// Drop trait on it, so the resource is freed when no ownership of it is left.
pub struct JsMemoryBlob {
    /// Its unique identifier
    id: ResourceId,
}

impl JsMemoryBlob {
    /// Create a `JsMemoryBlob` object from the `ResourceId` given by the
    /// JavaScript-side
    pub fn from_resource_id(id: ResourceId) -> Self {
        Self { id }
    }

    /// Recuperates the ResourceId behind this `JsMemoryBlob`.
    pub fn get_id(&self) -> ResourceId {
        self.id
    }

    /// Actually obtain the data behind this `JsMemoryBlob`, as a Vec of bytes.
    pub fn obtain(self) -> Vec<u8> {
        jsGetResourceData(self.id).unwrap()
    }
}

impl Drop for JsMemoryBlob {
    fn drop(&mut self) {
        jsFreeResource(self.id);
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
    #[allow(clippy::too_many_arguments)]
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
