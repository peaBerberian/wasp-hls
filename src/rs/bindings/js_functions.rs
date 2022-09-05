use std::fmt;
use crate::wasm_bindgen;

/// # js_functions
///
/// This file lists all JavaScript functions that are callable from Rust as well as
/// struct and enumeration used by those functions.

#[wasm_bindgen]
extern "C" {
    // Log the given text in the JavaScript console, with the log level given.
    pub fn jsLog(log_level: LogLevel, log: &str);

    // Fetch the given `url` from the network and await a u8 slice response.
    //
    // If and when it finishes with success, the result will be emitted as a
    // u8 slice through the `on_u8_request_finished` method of the
    // `WaspHlsPlayer` which has the given `PlayerId`.
    //
    // If and when it fails, the error will be emitted through the
    // `on_u8_request_failed` method of the `WaspHlsPlayer` which has the given
    // `PlayerId`.
    //
    // In both cases, those methods will always be called asynchronously after the `jsFetchU8`
    // call.
    //
    // If the request has been aborted while pending through the `jsAbortRequest`
    // function, no method will be called.
    pub fn jsFetchU8(player_id: PlayerId, url: &str) -> RequestId;

    // Variant of `jsFetchU8` where the resource requested is actually kept in
    // JavaScript's memory to avoid unnecesary copies of larges amount of data (and to avoid
    // stressing JavaScript's garbage collector in case where the data would go back and forth
    // between JavaScript and WASM).
    //
    // This variant follows the exact same rules than `jsFetchU8` except that a `ResourceId`
    // will be communicated on success through the `on_u8_no_copy_request_finished` method of the
    // `WaspHlsPlayer` which has the given `PlayerId`.
    // To avoid memory leaks, it is __VERY__ important to call the `jsFreeResource` function with
    // that `ResourceId` once it is not needed anymore.
    //
    // Bear in mind that the JavaScript-side bears the right to free the resource even if not
    // explicitely asked through a `JsFreeResource` call, in cases where a method from the current
    // `WaspHlsPlayer` unexpectedly throws. This is again to avoid leaking memory.
    //
    // In that last scenario, you will receive a corresponding error when trying to use that
    // `ResourceId` in the JavaScript functions receiving it.
    pub fn jsFetchU8NoCopy(player_id: PlayerId, url: &str) -> RequestId;

    // Abort a request started with `jsFetchU8` or `jsFetchU8NoCopy` based on its
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
    pub fn jsAttachMediaSource(player_id: PlayerId);

    // Remove MediaSource attached to the <video> element associated with
    // the `WaspHlsPlayer` if one, and free all its associated resources
    // (such as event listeners or created ObjectURL).
    //
    // This function performs all those operations synchronously.
    pub fn jsRemoveMediaSource(player_id: PlayerId);

    pub fn jsSetMediaSourceDuration(player_id: PlayerId, duration: f64);

    // Add a SourceBuffer to the created MediaSource, allowing to push media
    // segment of a given type to a lower-level media buffer.
    //
    // This function performs this operation synchronously and may fail, see
    // `AddSourceBufferResult` for more details on the return value.
    pub fn jsAddSourceBuffer(
        player_id: PlayerId,
        media_type: MediaType,
        typ: &str
    ) -> SourceBufferId;

    // Append media data to the given SourceBuffer.
    //
    // This process is asynchronous, meaning that the data might not be directly
    // considered directly after calling `jsAppendBuffer`.
    //
    // It is also forbidden to perform either append or remove operations on that SourceBuffer,
    // respectively by calling `jsAppendBuffer` or `jsRemoveBuffer` with the same
    // `SourceBufferId`, while that preceeding call did not either indicate success or failure
    // (respectively by calling either the `on_source_buffer_update` method or the
    // `on_source_buffer_error` method of the `WaspHlsPlayer` linked to the given `PlayerId`).
    pub fn jsAppendBuffer(
        player_id: PlayerId,
        source_buffer_id: SourceBufferId,
        data: &[u8]
    );

    // Variant of `jsAppendBuffer` where the data to append actually resides in JavaScript's
    // memory.
    //
    // Here, the data to puch is derived from its given `ResourceId`.
    //
    // This function relies on the exact same rules than `jsAppendBuffer`.
    pub fn jsAppendBufferJsBlob(
        player_id: PlayerId,
        source_buffer_id: SourceBufferId,
        segment_id: ResourceId
    );

    // Remove media data from the given SourceBuffer.
    //
    // This process is asynchronous, meaning that the data might not be directly
    // considered directly after calling `jsAppendBuffer`.
    //
    // It is also forbidden to perform either append or remove operations on that SourceBuffer,
    // respectively by calling `jsAppendBuffer` or `jsRemoveBuffer` with the same
    // `SourceBufferId`, while that preceeding call did not either indicate success or failure
    // (respectively by calling either the `on_source_buffer_update` method or the
    // `on_source_buffer_error` method of the `WaspHlsPlayer` linked to the given `PlayerId`).
    pub fn jsRemoveBuffer(
        source_buffer_id: SourceBufferId,
        start: f64,
        end: f64
    );

    pub fn jsEndOfStream(player_id: PlayerId);

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
    pub fn jsStartObservingPlayback(player_id: PlayerId);

    // If playback observations were being regularly sent to the
    // `WaspHlsPlayer` instance with the given `player_id`, stop emitting them
    // until `startObservingPlayback` is called again.
    pub fn jsStopObservingPlayback(player_id: PlayerId);

    // Free resource stored in JavaScript's memory kept alive for the current
    // `WaspHlsPlayer`.
    pub fn jsFreeResource(resource_id: ResourceId) -> bool;

    pub fn jsSeek(player_id: PlayerId, position: f64);
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
    //        player_id: PlayerId,
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

/// Errors that can arise when calling the either the `jsAppendBuffer` or the
/// `jsAppendBufferJsBlob` JavaScript functions.
#[wasm_bindgen]
pub enum AppendBufferErrorCode {
    /// The operation failed because the `WaspHlsPlayer` linked to the given `PlayerId`
    /// and/or the SourceBuffer instance linked to the given `SourceBufferId` was not found.
    PlayerOrSourceBufferInstanceNotFound = 1,

    /// The operation failed because the resource to append was not found.
    ///
    /// This error is only returned for cases where the data to push resides in JavaScript's
    /// memory (as opposed to given by the WebAssembly code).
    GivenResourceNotFound = 2,

    /// The operation failed because of an unknown error.
    UnknownError = 3,
}

/// Result of calling either the `jsAppendBuffer` or the `jsAppendBufferJsBlob`
/// JavaScript functions.
///
/// Creation of an `AppendBufferResult` should only be performed by the JavaScript side
/// through the exposed static constructors.
#[wasm_bindgen]
pub struct AppendBufferResult {
    error: Option<(AppendBufferErrorCode, Option<String>)>,
}

#[wasm_bindgen]
impl AppendBufferResult {
    /// Creates an `AppendBufferResult` indicating success, with the corresponding
    /// `SourceBufferId`.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn success() -> Self {
        Self { error: None }
    }

    /// Creates an `AppendBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AppendBufferErrorCode, desc: Option<String>) -> Self {
        Self { error: Some((err, desc)) }
    }
}

impl JsResult<(), AppendBufferErrorCode> for AppendBufferResult {
    /// Basically unwrap and consume the `AppendBufferResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<(), (AppendBufferErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(())
        }
    }
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
    source_buffer_id: u32,
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
    pub fn success(val : u32) -> Self {
        Self { source_buffer_id: val, error: None }
    }

    /// Creates an `AddSourceBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AddSourceBufferErrorCode, desc: Option<String>) -> Self {
        Self { source_buffer_id: 0, error: Some((err, desc)) }
    }
}

impl JsResult<u32, AddSourceBufferErrorCode> for AddSourceBufferResult {
    /// Basically unwrap and consume the `AddSourceBufferResult`, converting it into a
    /// Result enum.
    fn result(self) -> Result<u32, (AddSourceBufferErrorCode, Option<String>)> {
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

#[wasm_bindgen]
// TODO
pub struct PlaybackObservation {
    reason : PlaybackObservationReason,
    current_time: f64,
    playback_rate: f64,
    duration: f64,
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
pub type ResourceId = u32;

/// Identify a `WaspHlsPlayer`.
///
/// Multiple JavaScript-side API rely on that identifier.
pub type PlayerId = u32;

/// Identify a pending request.
pub type RequestId = u32;

/// Identify a SourceBuffer.
pub type SourceBufferId = u32;

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
