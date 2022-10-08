use std::fmt;
use crate::wasm_bindgen;

/// # js_functions
///
/// This file lists all JavaScript functions that are callable from Rust as well as
/// struct and enumeration used by those functions.

// XXX TODO remove PlayerId mentions

#[wasm_bindgen]
extern "C" {
    // Log the given text in the JavaScript console, with the log level given.
    pub fn jsLog(log_level: LogLevel, log: &str);

    pub fn jsTimer(duration: f64, reason: TimerReason) -> TimerId;

    pub fn jsClearTimer(id: TimerId);

    pub fn jsGetResourceData(id: ResourceId) -> Option<Vec<u8>>;

    // Fetch the given `url` from the network and await a response.
    //
    // If and when it finishes with success, the result will be emitted as a
    // `resource_id` through the `on_request_finished` method of this
    // `WaspHlsPlayer`.
    //
    // If and when it fails, the error will be emitted through the
    // `on_request_failed` method of this `WaspHlsPlayer`.
    //
    // In both cases, those methods will always be called asynchronously after the `jsFetch`
    // call.
    //
    // If the request has been aborted while pending through the `jsAbortRequest`
    // function, none of those methods will be called.
    //
    // The resource requested is actually kept in JavaScript's memory to avoid unnecesary copies
    // of larges amount of data (and to avoid stressing JavaScript's garbage collector in case
    // where the data would go back and forth between JavaScript and WASM).
    //
    // To avoid memory leaks, it is __VERY__ important to call the `jsFreeResource` function with
    // that `ResourceId` once it is not needed anymore.
    //
    // Bear in mind that the JavaScript-side bears the right to free the resource even if not
    // explicitely asked through a `JsFreeResource` call, in cases where a method from the current
    // `WaspHlsPlayer` unexpectedly throws. This is again to avoid leaking memory.
    //
    // In that last scenario, you will receive a corresponding error when trying to use that
    // `ResourceId` in the JavaScript functions receiving it.
    pub fn jsFetch(url: &str) -> RequestId;

    // Abort a request started with `jsFetch`` based on its
    // `request_id`.
    //
    // After calling this function, you won't get any event linked to that
    // request ever again.
    // Note that this RequestId may now be re-used in the future for any other
    // future request.
    //
    // Returns `true` if a pending request with the given RequestId was found and aborted,
    // `false` if no pending request was found with that RequestId.
    pub fn jsAbortRequest(request_id: RequestId) -> bool;

    // Create MediaSource and attach it to the <video> element associated with
    // the `WaspHlsPlayer` linked to the given `player_id`.
    //
    // This function performs the MediaSource creation and attachment
    // synchronously. Yet the MediaSource is not usable right away (e.g. it is
    // not immediately possible to open SourceBuffers on it.
    // The `WaspHlsPlayer` instance linked to the given `player_id` will know
    // when this MediaSource becomes usable or not when its
    // `on_media_source_state_change` method is called with the "Open"
    // `MediaSourceReadyState`.
    pub fn jsAttachMediaSource();

    // Remove MediaSource attached to the <video> element associated with
    // the `WaspHlsPlayer` if one, and free all its associated resources
    // (such as event listeners or created ObjectURL).
    //
    // This function performs all those operations synchronously.
    pub fn jsRemoveMediaSource();

    pub fn jsSetMediaSourceDuration(duration: f64);

    // Add a SourceBuffer to the created MediaSource, allowing to push media
    // segment of a given type to a lower-level media buffer.
    //
    // This function performs this operation synchronously and may fail, see
    // `AddSourceBufferResult` for more details on the return value.
    pub fn jsAddSourceBuffer(
        media_type: MediaType,
        typ: &str
    ) -> SourceBufferId;

    // Append media data to the given SourceBuffer.
    //
    // This process is asynchronous, meaning that the data might not be appended
    // directly after calling `jsAppendBuffer`.
    //
    // Append and remove operations performed on that SourceBuffer, respectively
    // through the `jsAppendBuffer` and `jsRemoveBuffer` functions, are all
    // pushed to an internal queue of operations which will be executed in the
    // same order than their calls have been made.
    // You will be notified once each single one of these operations have
    // succeeded when the `on_source_buffer_update` function is called on this
    // `WaspHlsPlayer` instance, with the same `source_buffer_id`.
    //
    // If the `on_source_buffer_error` method of this `WaspHlsPlayer` instance,
    // with the same `source_buffer_id`, it means that the currently scheduled
    // operation (the first one in the queue) failed. In that case, the
    // SourceBuffer is not usable anymore.
    pub fn jsAppendBuffer(
        source_buffer_id: SourceBufferId,
        segment_id: ResourceId,
        parse_time_information: bool
    ) -> AppendBufferResult;

    // Remove media data from the given SourceBuffer.
    //
    // This process is asynchronous, meaning that the data might not be directly
    // considered directly after calling `jsAppendBuffer`.
    //
    // Append and remove operations performed on that SourceBuffer, respectively
    // through the `jsAppendBuffer` and `jsRemoveBuffer` functions, are all
    // pushed to an internal queue of operations which will be executed in the
    // same order than their calls have been made.
    // You will be notified once each single one of these operations have
    // succeeded when the `on_source_buffer_update` function is called on this
    // `WaspHlsPlayer` instance, with the same `source_buffer_id`.
    //
    // If the `on_source_buffer_error` method of this `WaspHlsPlayer` instance,
    // with the same `source_buffer_id`, it means that the currently scheduled
    // operation (the first one in the queue) failed. In that case, the
    // SourceBuffer is not usable anymore.
    pub fn jsRemoveBuffer(
        source_buffer_id: SourceBufferId,
        start: f64,
        end: f64
    );

    pub fn jsEndOfStream();

    // After this method is called, the `WaspHlsPlayer` instance associated
    // with the given `PlayerId` will regularly receive `PlaybackObservation`
    // objects, describing the current playback conditions through its
    // `on_playback_tick` method.
    // The first event will be sent "almost" synchronously (queued as a
    // JavaScript microtask).
    //
    // You can stop receiving those observations by calling
    // `stopObservingPlayback` with the same `player_id` and restart it by
    // calling `startObservingPlayback` a new time.
    //
    // If the `WaspHlsPlayer` was already observing playback when that function
    // was called, this function does nothing.
    pub fn jsStartObservingPlayback();

    // If playback observations were being regularly sent to the
    // `WaspHlsPlayer` instance with the given `player_id`, stop emitting them
    // until `startObservingPlayback` is called again.
    pub fn jsStopObservingPlayback();

    // Free resource stored in JavaScript's memory kept alive for the current
    // `WaspHlsPlayer`.
    pub fn jsFreeResource(resource_id: ResourceId) -> bool;

    pub fn jsSeek(position: f64);
    //    // Check if the given mime-type and codecs are supported for playback.
    //    //
    //    // Returns `true` if that is the case, false if it isn't
    //    pub fn jsIsTypeSupported(typ: &str) -> bool;

    //    // Get the content of what has been buffered by the SourceBuffer, in terms of contiguous
    //    // time ranges, in seconds.
    //    // The returned vectors should always have an even length as it is organized by couples
    //    // of f64: the first of which is the start of the contiguous range in seconds and the
    //    // second the end.
    //    //
    //    // TODO this API might error depending on the underlying media element or MediaSource's
    //    // state.
    //    pub fn jsGetSourceBufferBuffered(
    //        ,
    //        source_buffer_id: SourceBufferId
    //    ) -> Vec<f64>;

}

// TODO some macro-based metaprogramming, instead of just repeating the same boilerplate for each
// result type, would be welcome

/// Errors that can arise when attempting to remove a MediaSource previously attached
/// to a media element.
#[wasm_bindgen]
pub enum RemoveMediaSourceErrorCode {
    /// Could not remove MediaSource from the media element because the `WaspHlsPlayer`
    /// linked to the given `PlayerId` was not known by the JavaScript-side.
    PlayerInstanceNotFound = 1,

    /// Could not remove MediaSource from the media element because the `WaspHlsPlayer`
    /// linked to the given `PlayerId` had no MediaSource attached to its media element.
    NoMediaSourceAttached = 2,

    /// Could not remove MediaSource from the media element because of an unknown error.
    UnknownError = 3,
}

/// Result of calling the `jsRemoveMediaSource` JavaScript function.
///
/// Creation of an `RemoveMediaSourceResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct RemoveMediaSourceResult {
    error: Option<(RemoveMediaSourceErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl RemoveMediaSourceResult {
    /// Creates an `RemoveMediaSourceResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `RemoveMediaSourceResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: RemoveMediaSourceErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), RemoveMediaSourceErrorCode> for RemoveMediaSourceResult {
    /// Basically unwrap and consume the `RemoveMediaSourceResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (RemoveMediaSourceErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

pub(crate) trait JsResult<T, E> {
    fn result(self)  -> Result<T, (E, Option<String>)>;
}

/// Errors that can arise when attempting to update the duration of a MediaSource.
/// TODO defined errors when the MediaSource is closed and so on?
#[wasm_bindgen]
pub enum MediaSourceDurationUpdateErrorCode {
    /// The `WaspHlsPlayer` linked to the given `PlayerId` was not known by the JavaScript-side.
    PlayerInstanceNotFound = 1,

    /// The `WaspHlsPlayer` linked to the given `PlayerId` had no MediaSource attached to its media
    /// element.
    NoMediaSourceAttached = 2,

    /// An unknown error arised
    UnknownError = 3,
}

/// Result of calling the `jsSetMediaSourceDuration` JavaScript function.
///
/// Creation of an `MediaSourceDurationUpdateResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct MediaSourceDurationUpdateResult {
    error: Option<(MediaSourceDurationUpdateErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl MediaSourceDurationUpdateResult {
    /// Creates an `MediaSourceDurationUpdateResult` indicating success.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `MediaSourceDurationUpdateResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: MediaSourceDurationUpdateErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), MediaSourceDurationUpdateErrorCode> for MediaSourceDurationUpdateResult {
    /// Basically unwrap and consume the `MediaSourceDurationUpdateResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (MediaSourceDurationUpdateErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

/// Errors (or success) that can arise when attempting to "attach" a MediaSource to a media
/// element.
#[wasm_bindgen]
pub enum AttachMediaSourceErrorCode {
    /// Could not attach MediaSource to the media element because the `WaspHlsPlayer`
    /// linked to the given `PlayerId` was not known by the JavaScript-side.
    PlayerInstanceNotFound = 1,

    /// Could not attach MediaSource to the media element because of an unknown error.
    UnknownError = 2,
}

/// Errors that can arise when calling the `jsRemoveBuffer` JavaScript function.
#[wasm_bindgen]
pub enum RemoveBufferErrorCode {
    /// The operation failed because the `WaspHlsPlayer` linked to the given `PlayerId`
    /// and/or the SourceBuffer instance linked to the given `SourceBufferId` was not found.
    PlayerOrSourceBufferInstanceNotFound = 1,

    /// The operation failed because of an unknown error.
    UnknownError = 2,
}

/// Result of calling the `jsRemoveBuffer` JavaScript function.
///
/// Creation of an `RemoveBufferResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct RemoveBufferResult {
    error: Option<(RemoveBufferErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl RemoveBufferResult {
    /// Creates an `RemoveBufferResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `RemoveBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: RemoveBufferErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), RemoveBufferErrorCode> for RemoveBufferResult {
    /// Basically unwrap and consume the `RemoveBufferResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (RemoveBufferErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

/// Errors that can arise when calling the `jsEndOfStream` JavaScript function.
#[wasm_bindgen]
pub enum EndOfStreamErrorCode {
    /// The operation failed because the `WaspHlsPlayer` linked to the given `PlayerId`
    /// was not found.
    PlayerInstanceNotFound = 1,

    /// The operation failed because of an unknown error.
    UnknownError = 2,
}

/// Result of calling the `jsEndOfStream` JavaScript function.
///
/// Creation of an `EndOfStreamResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct EndOfStreamResult {
    error: Option<(EndOfStreamErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl EndOfStreamResult {
    /// Creates an `EndOfStreamResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `EndOfStreamResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: EndOfStreamErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), EndOfStreamErrorCode> for EndOfStreamResult {
    /// Basically unwrap and consume the `EndOfStreamResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (EndOfStreamErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

/// Result of calling the `jsAddSourceBuffer` JavaScript function.
///
/// Creation of an `AddSourceBufferResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct AddSourceBufferResult {
    source_buffer_id: SourceBufferId,
    error: Option<(AddSourceBufferErrorCode, Option<String>)>,
}

/// Error that might arise when adding a SourceBuffer through a MediaSource instance.
#[wasm_bindgen]
pub enum AddSourceBufferErrorCode {
    /// The `WaspHlsPlayer` linked to the given `PlayerId` was not known by the JavaScript-side.
    PlayerInstanceNotFound,

    /// The `WaspHlsPlayer` linked to the given `PlayerId` had no MediaSource attached to its media
    /// element.
    NoMediaSourceAttached,

    /// The `MediaSource` instance linked to the `WaspHlsPlayer` (itself linked to the given
    /// `PlayerId`) is in a "closed" state.
    MediaSourceIsClosed,

    /// A `QuotaExceededError` was received while trying to add the `SourceBuffer`
    ///
    /// Such errors are often encountered when another SourceBuffer attached to the same
    /// MediaSource instance was already updated through a buffer operation.
    QuotaExceededError,

    /// The given mime-type and codec  combination is not supported
    TypeNotSupportedError,

    /// The given mime-type and codec combination is empty
    EmptyMimeType,

    /// An unknown error happened.
    UnknownError,
}

/// `AddSourceBufferResult` methods exposed to JavaScript.
#[wasm_bindgen]
impl AddSourceBufferResult {
    /// Creates an `AddSourceBufferResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success(val : SourceBufferId) -> Self {
        Self { source_buffer_id: val, error: None }
    }

    /// Creates an `AddSourceBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AddSourceBufferErrorCode, desc: Option<String>) -> Self {
        Self { source_buffer_id: 0., error: Some((err, desc)) }
    }
}

impl JsResult<SourceBufferId, AddSourceBufferErrorCode> for AddSourceBufferResult {
    /// Basically unwrap and consume the `AddSourceBufferResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<SourceBufferId, (AddSourceBufferErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(self.source_buffer_id)
        }
    }
}

/// Result of calling the `jsAttachMediaSource` JavaScript function.
///
/// Creation of an `AttachMediaSourceResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct AttachMediaSourceResult {
    error: Option<(AttachMediaSourceErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl AttachMediaSourceResult {
    /// Creates an `AttachMediaSourceResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `AttachMediaSourceResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AttachMediaSourceErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), AttachMediaSourceErrorCode> for AttachMediaSourceResult {
    /// Basically unwrap and consume the `AttachMediaSourceResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (AttachMediaSourceErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

/// Result of calling the `jsPrepareSegmentForBuffer` JavaScript function.
///
/// Creation of an `AppendBufferResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct AppendBufferResult {
    success: Option<ParsedSegmentInfo>,
    error: Option<(AppendBufferErrorCode, Option<String>)>,
}

pub struct ParsedSegmentInfo {
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

#[wasm_bindgen]
impl AppendBufferResult {
    /// Creates an `AppendBufferResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success(
        start: Option<f64>,
        duration: Option<f64>
    ) -> Self {
        Self {
            success: Some(ParsedSegmentInfo { start, duration }),
            error: None,
        }
    }

    /// Creates an `AppendBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AppendBufferErrorCode, desc: Option<String>) -> Self {
        Self { success: None, error: Some((err, desc)) }
    }
}

impl JsResult<
    Option<ParsedSegmentInfo>,
    AppendBufferErrorCode
> for AppendBufferResult {
    /// Basically unwrap and consume the `AppendBufferResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<
        Option<ParsedSegmentInfo>,
        (AppendBufferErrorCode, Option<String>)
    > {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(self.success)
        }
    }
}

/// Errors that can arise when calling the `jsAppendBuffer` JavaScript function.
#[wasm_bindgen]
pub enum AppendBufferErrorCode {
    /// The operation failed because the resource to append was not found.
    ///
    /// This error is only returned for cases where the data to push resides in JavaScript's
    /// memory (as opposed to given by the WebAssembly code).
    NoResource,
    /// The operation failed because the SourceBuffer instance linked to the
    /// given `SourceBufferId` was not found.
    NoSourceBuffer,
    /// The operation failed at the transmuxing stage.
    TransmuxerError,
    /// The operation failed because of an unknown error.
    UnknownError,
}

/// Current playback information associated to the `HTMLMediaElement` displayed
/// on the page.
/// `PlaybackObservation` should be regularly sent on various events.
#[wasm_bindgen]
pub struct PlaybackObservation {
    /// The reason that triggered the `PlaybackObservation` struct to be
    /// created.
    reason : PlaybackObservationReason,
    /// The value of the `currentTime` attribute of the HTMLMediaElement.
    current_time: f64,
    /// The value of the `playbackRate` attribute of the HTMLMediaElement.
    playback_rate: f64,
    /// The value of the `duration` attribute of the HTMLMediaElement.
    duration: f64,
}

/// Reason that triggered a `PlaybackObservation`
#[wasm_bindgen]
pub enum PlaybackObservationReason {
    /// First observation given initially, not linked to any particular event.
    Init,
    /// Observation sent right after a "seeked" event has been received on the
    /// HTMLMediaElement.
    Seeked,
    /// Observation sent right after a "seeking" event has been received on the
    /// HTMLMediaElement.
    Seeking,
    /// Observation sent right after an "ended" event has been received on the
    /// HTMLMediaElement.
    Ended,
    /// Observation sent right after a the HTMLMediaElement's `readyState`
    /// attribute changed
    ReadyStateChanged,
    /// Observation sent regularly, at a set interval without any event.
    RegularInterval,
    /// Observation sent as an error has been received on the HTMLMediaElement.
    Error,
}

/// "Reason" associated to a timer started by the WaspHlsPlayer.
///
/// This can then help to identify what the timer was for once resolved.
#[wasm_bindgen]
pub enum TimerReason {
    /// The timer is linked to the MediaPlaylistRefresh's mechanism, meaning
    /// that the wanted MediaPlaylist may have to be reloaded.
    MediaPlaylistRefresh = 0,
}

/// Levels with which a log can be emitted.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum LogLevel {
    /// Log level reserved for very important errors and highly unexpected events.
    Error = 0,

    /// Log level reserved for less important errors and unexpected events.
    Warn = 1,

    /// Log level reserved for important events
    Info = 2,

    /// Log level used when debugging. Small-ish yet impactful events should be logged with it.
    Debug = 3,
}

/// Identify a resource allocated on the JavaScript side and kept alive until either
/// `jsFreeResource` is called with it or the `WaspHlsPlayer` that requested it unexpectedly
/// throws.
///
/// Special care of those id should be taken to avoid memory leaks: you should always call
/// `jsFreeResource` as soon as the resource is not needed anymore.
pub type ResourceId = f64;

/// Identify a pending request.
pub type RequestId = f64;

/// Identify a pending timer.
pub type TimerId = f64;

/// Identify a SourceBuffer.
pub type SourceBufferId = f64;

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaType {
    Audio = 0,
    Video = 1,
}

impl fmt::Display for MediaType {
    /// When wanting to display the value, just format Audio as "audio" and
    /// Video as "video"
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            MediaType::Audio => "audio",
            MediaType::Video => "video",
        })
    }
}
