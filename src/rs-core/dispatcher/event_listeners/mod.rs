use std::{iter::Map, ops::Index, slice::Chunks};

use crate::{
    bindings::{
        f64_vec_from_abi, jsFreeResource, jsGetResourceData, AddSourceBufferErrorCode,
        MediaSourceReadyState, PlaybackTickReason, PushedSegmentErrorCode, RequestId, ResourceId,
        SourceBufferId, TimerId, TimerReason,
    },
    dispatcher::{Dispatcher, UNKNOWN_REQUEST_SIZE},
    utils::{logger::*, url::Url},
};

/// Methods triggered on JavaScript events by the JavaScript code.
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
        duration_ms: f64,
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
    pub fn on_request_failed(
        &mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>,
    ) {
        self.on_request_failed_core(request_id, has_timeouted, status);
    }

    /// The JS code should call this method whenever progress is made on an HTTP(S) request
    /// started with `jsFetch`.
    pub fn on_request_progress(
        &mut self,
        request_id: RequestId,
        bytes_loaded: u32,
        bytes_total: Option<u32>,
        duration_ms: f64,
    ) {
        self.on_request_progress_core(request_id, bytes_loaded, bytes_total, duration_ms);
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
    pub fn on_source_buffer_update(
        &mut self,
        source_buffer_id: SourceBufferId,
        buffered: JsTimeRanges,
    ) {
        self.on_source_buffer_update_core(source_buffer_id, buffered);
    }

    /// The JS code should call this method when a `SourceBuffer`'s creation
    /// asynchronously fails
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier generated when the
    ///   SourceBuffer was created through the `jsAddSourceBuffer` call.
    ///   This allows the `Dispatcher` to identify which `SourceBuffer`
    ///   we're talking about.
    ///
    /// * `code` - The `AddSourceBufferErrorCode` linked to the error.
    ///
    /// * `message` - An human-readable message describing the error.
    pub fn on_source_buffer_creation_error(
        &mut self,
        source_buffer_id: SourceBufferId,
        code: AddSourceBufferErrorCode,
        msg: String,
    ) {
        self.on_source_buffer_creation_error_core(source_buffer_id, (code, Some(msg)));
    }

    /// The JS code should call this method when a SourceBuffer emits an `error`
    /// event while processing an `appendBuffer` call
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given generated when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    ///
    /// * `code` - A coe identifying the type of problem encountered.
    pub fn on_append_buffer_error(
        &mut self,
        source_buffer_id: SourceBufferId,
        code: PushedSegmentErrorCode,
        buffered: JsTimeRanges,
    ) {
        self.on_append_buffer_error_core(source_buffer_id, code, buffered);
    }

    /// The JS code should call this method when a SourceBuffer emits an `error`
    /// event while processing a `remove` call
    ///
    /// # Arguments
    ///
    /// * `source_buffer_id` - The identifier given generated when the
    ///   SourceBuffer was created. This allows the `Dispatcher` to identify
    ///   which SourceBuffer actually emitted this event.
    pub fn on_remove_buffer_error(
        &mut self,
        source_buffer_id: SourceBufferId,
        buffered: JsTimeRanges,
    ) {
        self.on_remove_buffer_error_core(source_buffer_id, buffered);
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
            TimerReason::MediaPlaylistRefresh => self.on_playlist_refresh_timer_ended(id),
            TimerReason::RetryRequest => self.on_retry_request(id),
        }
    }

    pub fn on_codecs_support_update(&mut self) {
        self.on_codecs_support_update_core();
    }
}

/// Special structure to handle data that is only present in JavaScript's
/// memory.
///
/// The data is identified through a unique `ResourceId` identifier.
///
/// The idea behind this struct is to prevent memory leaks by implementing the
/// Drop trait on it, so the resource is freed when no ownership of it is left.
#[derive(Debug)]
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
    pub fn id(&self) -> ResourceId {
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

pub struct JsTimeRanges {
    buffered: Vec<f64>,
}

impl JsTimeRanges {
    pub fn new(buffered: Vec<f64>) -> Self {
        if !buffered.len().is_multiple_of(2) {
            log_error!("Incorrect JsTimeRanges object");
            Self { buffered: vec![] }
        } else {
            Self { buffered }
        }
    }

    pub fn len(&self) -> usize {
        self.buffered.len() / 2
    }

    pub fn start(&self, idx: usize) -> Option<f64> {
        self.buffered.get(idx * 2).copied()
    }

    pub unsafe fn start_unchecked(&self, idx: usize) -> f64 {
        self.buffered[idx * 2]
    }

    pub fn end(&self, idx: usize) -> Option<f64> {
        self.buffered.get((idx * 2) + 1).copied()
    }

    pub unsafe fn end_unchecked(&self, idx: usize) -> f64 {
        self.buffered[idx * 2 + 1]
    }
}

impl Index<usize> for JsTimeRanges {
    type Output = [f64; 2];
    fn index(&self, index: usize) -> &Self::Output {
        self.buffered.as_slice()[index..index + 1]
            .try_into()
            .unwrap()
    }
}

impl JsTimeRanges {
    pub(crate) fn range(&self, idx: usize) -> Option<(f64, f64)> {
        self.buffered
            .get(idx * 2)
            .and_then(|s| self.buffered.get((idx * 2) + 1).map(|e| (*s, *e)))
    }

    pub(crate) fn range_unchecked(&self, idx: usize) -> (f64, f64) {
        (self.buffered[idx * 2], self.buffered[(idx * 2) + 1])
    }

    pub(crate) fn range_for(&self, pos: f64) -> Option<(f64, f64)> {
        for range in self.into_iter() {
            if pos < range.1 {
                return if pos >= range.0 { Some(range) } else { None };
            }
        }
        None
    }

    pub(crate) fn buffer_gap(&self, pos: f64) -> Option<f64> {
        self.range_for(pos).map(|r| r.1 - pos)
    }
}

impl<'a> IntoIterator for &'a JsTimeRanges {
    type Item = (f64, f64);

    // Yep, not easy to look at. Maybe future Rust feature can simplify that mess
    type IntoIter = Map<Chunks<'a, f64>, fn(&'a [f64]) -> (f64, f64)>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffered.chunks(2).map(|vals| (vals[0], vals[1]))
    }
}

pub struct MediaObservation {
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
    ) -> Self {
        Self {
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

fn dispatcher_mut<'a>(dispatcher_ptr: u32) -> &'a mut Dispatcher {
    unsafe { &mut *(dispatcher_ptr as *mut Dispatcher) }
}

fn string_from_abi(ptr: *const u8, len: u32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn maybe_time_ranges(ptr: *const f64, len: u32) -> Option<JsTimeRanges> {
    if len == u32::MAX {
        None
    } else {
        Some(JsTimeRanges::new(f64_vec_from_abi(ptr, len as usize)))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__request_finished(
    dispatcher_ptr: u32,
    request_id: RequestId,
    resource_id: ResourceId,
    resource_size: u32,
    final_url_ptr: *const u8,
    final_url_len: u32,
    duration_ms: f64,
) {
    dispatcher_mut(dispatcher_ptr).on_request_finished(
        request_id,
        resource_id,
        resource_size,
        string_from_abi(final_url_ptr, final_url_len),
        duration_ms,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__request_failed(
    dispatcher_ptr: u32,
    request_id: RequestId,
    has_timeouted: u32,
    status: u32,
) {
    dispatcher_mut(dispatcher_ptr).on_request_failed(
        request_id,
        has_timeouted != 0,
        if status == u32::MAX {
            None
        } else {
            Some(status)
        },
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__request_progress(
    dispatcher_ptr: u32,
    request_id: RequestId,
    bytes_loaded: u32,
    bytes_total: u32,
    duration_ms: f64,
) {
    dispatcher_mut(dispatcher_ptr).on_request_progress(
        request_id,
        bytes_loaded,
        if bytes_total == UNKNOWN_REQUEST_SIZE {
            None
        } else {
            Some(bytes_total)
        },
        duration_ms,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__media_source_state_change(dispatcher_ptr: u32, state: u32) {
    dispatcher_mut(dispatcher_ptr)
        .on_media_source_state_change(MediaSourceReadyState::from_raw(state));
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__source_buffer_update(
    dispatcher_ptr: u32,
    source_buffer_id: SourceBufferId,
    buffered_ptr: *const f64,
    buffered_len: u32,
) {
    dispatcher_mut(dispatcher_ptr).on_source_buffer_update(
        source_buffer_id,
        JsTimeRanges::new(f64_vec_from_abi(buffered_ptr, buffered_len as usize)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__source_buffer_creation_error(
    dispatcher_ptr: u32,
    source_buffer_id: SourceBufferId,
    code: u32,
    msg_ptr: *const u8,
    msg_len: u32,
) {
    dispatcher_mut(dispatcher_ptr).on_source_buffer_creation_error(
        source_buffer_id,
        AddSourceBufferErrorCode::from_raw(code),
        string_from_abi(msg_ptr, msg_len),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__append_buffer_error(
    dispatcher_ptr: u32,
    source_buffer_id: SourceBufferId,
    code: u32,
    buffered_ptr: *const f64,
    buffered_len: u32,
) {
    dispatcher_mut(dispatcher_ptr).on_append_buffer_error(
        source_buffer_id,
        match code {
            0 => PushedSegmentErrorCode::BufferFull,
            _ => PushedSegmentErrorCode::UnknownError,
        },
        JsTimeRanges::new(f64_vec_from_abi(buffered_ptr, buffered_len as usize)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__remove_buffer_error(
    dispatcher_ptr: u32,
    source_buffer_id: SourceBufferId,
    buffered_ptr: *const f64,
    buffered_len: u32,
) {
    dispatcher_mut(dispatcher_ptr).on_remove_buffer_error(
        source_buffer_id,
        JsTimeRanges::new(f64_vec_from_abi(buffered_ptr, buffered_len as usize)),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__playback_tick(
    dispatcher_ptr: u32,
    reason: u32,
    current_time: f64,
    ready_state: u32,
    buffered_ptr: *const f64,
    buffered_len: u32,
    paused: u32,
    seeking: u32,
    ended: u32,
    duration: f64,
    audio_buffered_ptr: *const f64,
    audio_buffered_len: u32,
    video_buffered_ptr: *const f64,
    video_buffered_len: u32,
) {
    let observation = MediaObservation::new(
        PlaybackTickReason::from_raw(reason),
        current_time,
        ready_state as u8,
        JsTimeRanges::new(f64_vec_from_abi(buffered_ptr, buffered_len as usize)),
        paused != 0,
        seeking != 0,
        ended != 0,
        duration,
        maybe_time_ranges(audio_buffered_ptr, audio_buffered_len),
        maybe_time_ranges(video_buffered_ptr, video_buffered_len),
    );
    dispatcher_mut(dispatcher_ptr).on_playback_tick(observation);
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__timer_ended(dispatcher_ptr: u32, id: TimerId, reason: u32) {
    dispatcher_mut(dispatcher_ptr).on_timer_ended(id, TimerReason::from_raw(reason));
}

#[unsafe(no_mangle)]
pub extern "C" fn __web_event__codecs_support_update(dispatcher_ptr: u32) {
    dispatcher_mut(dispatcher_ptr).on_codecs_support_update();
}
