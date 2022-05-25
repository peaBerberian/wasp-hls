use crate::wasm_bindgen;

/// # External
///
/// This file lists all JavaScript functions that are callable from Rust as well as
/// struct and enumeration used by those functions.

#[wasm_bindgen]
extern "C" {
    /// Log the given text in the JavaScript console, with the log level given.
    pub fn jsLog(log_level: LogLevel, log: &str);

    /// Fetch the given `url` from the network and await a u8 slice response.
    ///
    /// If and when it finishes with success, the result will be emitted as a
    /// u8 slice through the `on_u8_request_finished` method of the
    /// `WaspHlsPlayer` which has the given `PlayerId`.
    ///
    /// If and when it fails, the error will be emitted through the
    /// `on_u8_request_failed` method of the `WaspHlsPlayer` which has the given
    /// `PlayerId`.
    ///
    /// If the request has been aborted while pending through the `jsAbortRequest`
    /// function, no method will be called.
    pub fn jsFetchU8(player_id: PlayerId, url: &str) -> RequestId;

    /// Abort a request started with `jsFetchStr` or `jsFetchU8` based on its `request_id`.
    ///
    /// After calling this function, you won't get any event linked to that
    /// request ever again.
    /// Note that this RequestId may now be re-used in the future for any other
    /// future request.
    pub fn jsAbortRequest(request_id: RequestId);

    /// Create MediaSource and attach it to the <video> element associated with
    /// the `WaspHlsPlayer` linked to the given `player_id`.
    ///
    /// This function performs the MediaSource creation and attachment
    /// synchronously. Yet the MediaSource is not usable right away (e.g. it is
    /// not immediately possible to open SourceBuffers on it.
    /// The `WaspHlsPlayer` instance linked to the given `player_id` will know
    /// when this MediaSource becomes usable or not when its
    /// `on_media_source_state_change` method is called.
    pub fn jsAttachMediaSource(player_id: PlayerId);

    /// Remove MediaSource attached to the <video> element associated with
    /// the `WaspHlsPlayer` if one, and free all its associated resources
    /// (such as event listeners or created ObjectURL).
    ///
    /// This function performs all those operations synchronously and never
    /// fails.
    pub fn jsRemoveMediaSource(player_id: PlayerId);

    /// Add a SourceBuffer to the created MediaSource, allowing to push media
    /// segment of a given type to a lower-level media buffer.
    ///
    /// This function performs this operation synchronously and may fail.
    ///
    /// The returned value is an i32.
    ///
    /// `0` or any positive value indicate a success and will be the
    /// corresponding `SourceBufferId` with which you will be able to call
    /// functions such as `jsAppendBuffer` and `jsRemoveBuffer`.
    ///
    /// Any negative value indicates an error:
    ///  - `-1`: Error returned when the MediaSource is not yet attached to
    ///    the media element. You should only call `jsAddSourceBuffer` when the
    ///    MediaSource has the "open" media state.
    ///  - `-2`: Error returned when the MediaSource has the `Closed`
    ///    `MediaSourceReadyState`. You should only call `jsAddSourceBuffer`
    ///    when the MediaSource has the "open" readyState.
    ///  - `-3`: Error returned when SourceBuffers can no longer be attached to
    ///    the MediaSource. This is probably because data already have been
    ///    pushed to another SourceBuffers.
    ///  - `-4`: Error returned when the given `typ` is not supported.
    ///  - `-5`: Thrown if the value specified for mimeType is an empty string
    ///    rather than a valid MIME type.
    ///  - `-6`: Any other error.
    ///
    /// If the call succeed, the created SourceBuffer will be considered open
    /// until either `jsRemoveMediaSource` is called.
    pub fn jsAddSourceBuffer(player_id: PlayerId, typ: &str) -> i32;

    /// Append media data to the given SourceBuffer.
    ///
    /// This process is asynchronous, meaning that the data might not be directly
    /// considered directly after calling `jsAppendBuffer`.
    ///
    /// It is also forbidden to perform either append or remove operations on that SourceBuffer,
    /// respectively by calling `jsAppendBuffer` or `jsRemoveBuffer` with the same
    /// `SourceBufferId`, while that preceeding call did not either indicate success or failure
    /// (respectively by calling either the `on_source_buffer_update` method or the
    /// `on_source_buffer_error` method of the `WaspHlsPlayer` linked to the given `PlayerId`).
    pub fn jsAppendBuffer(player_id: PlayerId, source_buffer_id: SourceBufferId, data: &[u8]);

    /// Remove media data from the given SourceBuffer.
    ///
    /// This process is asynchronous, meaning that the data might not be directly
    /// considered directly after calling `jsAppendBuffer`.
    ///
    /// It is also forbidden to perform either append or remove operations on that SourceBuffer,
    /// respectively by calling `jsAppendBuffer` or `jsRemoveBuffer` with the same
    /// `SourceBufferId`, while that preceeding call did not either indicate success or failure
    /// (respectively by calling either the `on_source_buffer_update` method or the
    /// `on_source_buffer_error` method of the `WaspHlsPlayer` linked to the given `PlayerId`).
    pub fn jsRemoveBuffer(source_buffer_id: SourceBufferId, start: f64, end: f64);

    /// Get the content of what has been buffered by the SourceBuffer, in terms of contiguous
    /// time ranges, in seconds.
    /// The returned vectors should always have an even length as it is organized by couples
    /// of f64: the first of which is the start of the contiguous range in seconds and the
    /// second the end.
    pub fn jsGetSourceBufferBuffered(player_id: PlayerId, source_buffer_id: SourceBufferId) -> Vec<f64>;

    /// After this method is called, the `WaspHlsPlayer` instance associated
    /// with the given `PlayerId` will regularly receive `PlaybackObservation`
    /// objects, describing the current playback conditions through its
    /// `on_playback_tick` method.
    /// The first event will be sent "almost" synchronously (queued as a
    /// JavaScript microtask).
    ///
    /// You can stop receiving those observations by calling
    /// `stopObservingPlayback` with the same `player_id` and restart it by
    /// calling `startObservingPlayback` a new time.
    ///
    /// If the `WaspHlsPlayer` was already observing playback when that function
    /// was called, this function does nothing.
    pub fn jsStartObservingPlayback(player_id: PlayerId);

    /// If playback observations were being regularly sent to the
    /// `WaspHlsPlayer` instance with the given `player_id`, stop emitting them
    /// until `startObservingPlayback` is called again.
    pub fn jsStopObservingPlayback(player_id: PlayerId);

    pub fn jsIsTypeSupported(typ: &str) -> bool;

//    /// Fetch the given `url` from the network and await a String response.
//    ///
//    /// If and when it finishes with success, the result will be emitted as a
//    /// String through the `on_str_request_finished` method of the
//    /// `WaspHlsPlayer` which has the given `PlayerId`.
//    ///
//    /// If and when it fails, the error will be emitted through the
//    /// `on_str_request_failed` method of the `WaspHlsPlayer` which has the given
//    /// `PlayerId`.
//    ///
//    /// If the request has been aborted while pending through the `jsAbortRequest`
//    /// function, no method will be called.
//    pub fn jsFetchStr(player_id: PlayerId, url: &str) -> RequestId;
}

#[wasm_bindgen]
// TODO
pub enum PlaybackObservationReason {
    Init,
    Seeked,
    Seeking,
    ReadyStateChanged,
    RegularInterval,
    Error,
}

#[wasm_bindgen]
pub struct PlaybackObservation {
    reason : PlaybackObservationReason,
    current_time: f64,
    playback_rate: f64,
    duration: f64,
}

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

pub type PlayerId = u32;
pub type RequestId = u32;
pub type SourceBufferId = u32;
