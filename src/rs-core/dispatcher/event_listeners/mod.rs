use crate::{
    bindings::{
        f64_vec_from_abi, AddSourceBufferErrorCode, JsMemoryBlob, JsTimeRanges,
        MediaSourceReadyState, PlaybackTickReason, PushedSegmentErrorCode, RequestId, ResourceId,
        SourceBufferId, TimerId, TimerReason,
    },
    dispatcher::Dispatcher,
    media_element::MediaObservation,
    utils::url::Url,
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
    last_media_command_number: f64,
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
        last_media_command_number as u64,
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
