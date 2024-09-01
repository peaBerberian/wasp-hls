use crate::{
    media_element::PushSegmentError,
    parser::{
        MediaPlaylistParsingError, MediaPlaylistUpdateError, MultivariantPlaylistParsingError,
    },
    wasm_bindgen,
};
use std::fmt;

/// # js_functions
///
/// This file lists all JavaScript functions that are callable from Rust as well as
/// struct and enumeration used by those functions.

#[wasm_bindgen]
extern "C" {
    // Announces that a minor problem occured when trying to play the current
    // content.
    // pub fn jsWarning(warning: WarningCode);

    // Log the given text in the JavaScript console, with the log level given.
    pub fn jsLog(log_level: LogLevel, log: &str);

    // Starts a timer for the number of milliseconds indicated by the `duration` argument.
    //
    // Once this timer has elapsed, and unless `jsClearTimer` has been called since with
    // the `TimerId` returned by this function, the `on_timer_ended` of this
    // `WaspHlsPlayer` will be called with both the corresponding `TimerId` and `reason`,
    // which you can use on your side to better categorize timer categories.
    pub fn jsTimer(duration: f64, reason: TimerReason) -> TimerId;

    // Clear a timer started with `jsTimer`.
    pub fn jsClearTimer(id: TimerId);

    // Returns the data, as a vector of bytes of a resource behind a `ResourceId`.
    //
    // Returns `None` if that `ResourceId` is not linked to any resource right now.
    // TODO also return ContentType of the requested data?
    pub fn jsGetResourceData(id: ResourceId) -> Option<Vec<u8>>;

    // Fetch the given `url` from the network and await a response.
    // If at least the `range_base` argument is set, the request will be a range request from
    // `range_base` to `range_end` or to the end of the resource if `range_end` is set to `None`.
    //
    // A timeout in milliseconds may also be communicated to `jsFetch`, after which the request will
    // automatically be aborted and failure will be reported. To disable any timeout, you can set
    // the `timeout` argument to a negative value.
    //
    // If and when it finishes with success, the result will be emitted as a `resource_id` through
    // the `on_request_finished` method of this `WaspHlsPlayer`.
    //
    // If and when it fails, the error will be emitted through the `on_request_failed` method of this
    // `WaspHlsPlayer`.
    //
    // In both cases, those methods will always be called asynchronously after the `jsFetch` call.
    //
    // If the request has been aborted while pending through the `jsAbortRequest` function, none of
    // those methods will be called.
    //
    // The resource requested is actually kept in JavaScript's memory to avoid unnecesary copies of
    // larges amount of data (and to avoid stressing JavaScript's garbage collector in case where
    // the data would go back and forth between JavaScript and WASM).
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
    pub fn jsFetch(
        url: &str,
        range_base: Option<usize>,
        range_end: Option<usize>,
        timeout: f64,
    ) -> RequestId;

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
    // this `WaspHlsPlayer`.
    //
    // This function performs the MediaSource creation and attachment
    // synchronously. Yet the MediaSource is not usable right away (e.g. it is
    // not immediately possible to open SourceBuffers on it.
    // This `WaspHlsPlayer` instance will know when this MediaSource becomes usable or not
    // when its `on_media_source_state_change` method is called with the "Open"
    // `MediaSourceReadyState`.
    pub fn jsAttachMediaSource() -> AttachMediaSourceResult;

    // Remove MediaSource attached to the <video> element associated with
    // the `WaspHlsPlayer` if one, and free all its associated resources
    // (such as event listeners or created ObjectURL).
    //
    // This function performs all those operations synchronously.
    pub fn jsRemoveMediaSource() -> RemoveMediaSourceResult;

    // Update the duration in seconds of the MediaSource attached to this WaspHlsPlayer.
    pub fn jsSetMediaSourceDuration(duration: f64) -> MediaSourceDurationUpdateResult;

    // Add a SourceBuffer to the created MediaSource, allowing to push media
    // segment of a given type to a lower-level media buffer.
    //
    // This function performs this operation synchronously and may fail, see
    // `AddSourceBufferResult` for more details on the return value.
    pub fn jsAddSourceBuffer(media_type: MediaType, typ: &str) -> AddSourceBufferResult;

    pub fn jsIsTypeSupported(media_type: MediaType, typ: &str) -> Option<bool>;

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
        parse_time_information: bool,
    ) -> AppendBufferResult;

    // Remove media data from the given SourceBuffer.
    //
    // This process is asynchronous, meaning that the data might not be directly
    // considered after calling `jsRemoveBuffer`.
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
        end: f64,
    ) -> RemoveBufferResult;

    // Call the `MediaSource.prototype.endOfStream` API, allowing to signal that
    // all contents have been pushed to all of its buffer.
    //
    // Note that you should make sure that all of the buffers have an empty queue
    // of operations (no `jsAppendBuffer` or `jsRemoveBuffer` call not yet
    // validated through a `on_source_buffer_update` callback) before making the
    // `jsEndOfStream` call.
    pub fn jsEndOfStream() -> EndOfStreamResult;

    // After this method is called, this `WaspHlsPlayer` instance will regularly receive
    // `PlaybackObservation` objects, describing the current playback conditions through
    // its `on_playback_tick` method.
    // The first event will be sent right away, though asynchronously.
    //
    // You can stop receiving those observations by calling
    // `stopObservingPlayback` and restart it by calling `startObservingPlayback` a new
    // time.
    //
    // If this `WaspHlsPlayer` was already observing playback when that function
    // was called, this function does nothing.
    pub fn jsStartObservingPlayback();

    // If playback observations were being regularly sent to this
    // `WaspHlsPlayer` instance, stop emitting them until `startObservingPlayback` is
    // called again.
    pub fn jsStopObservingPlayback();

    // Free resource stored in JavaScript's memory kept alive for the current
    // `WaspHlsPlayer`.
    pub fn jsFreeResource(resource_id: ResourceId) -> bool;

    // Method called to change the playback rate (speed of playback).
    // This can be both in response to API input or to start/exit buffering by
    // example.
    pub fn jsSetPlaybackRate(playbackRate: f64);

    // Call the `HTMLMediaElement.prototype.seek` API, allowing to move the current
    // playback's playhead.
    pub fn jsSeek(position: f64);

    pub fn jsFlush();

    // Method called to indicate the offset to convert playlist time, as anounced in the
    // MediaPlaylist (and which should be preferred for a user interface) into media time,
    // which is the time actually present on the HTMLMediaElement.
    pub fn jsSetMediaOffset(media_offset: f64);

    pub fn jsUpdateContentInfo(
        minimum_position: Option<f64>,
        maximum_position: Option<f64>,
        playlist_nat: PlaylistNature,
    );

    pub fn jsAnnounceFetchedContent(variant_info: Vec<u32>, audio_tracks_info: Vec<u32>);

    pub fn jsAnnounceVariantUpdate(variant_id: Option<u32>);

    pub fn jsAnnounceTrackUpdate(
        media_type: MediaType,
        current_audio_track: Option<u32>,
        is_audio_track_selected: bool,
    );

    pub fn jsAnnounceVariantLockStatusChange(variant_id: Option<u32>);

    pub fn jsStartRebuffering();
    pub fn jsStopRebuffering();

    pub fn jsGetRandom() -> f64;

    // Errors

    /// Function to call to indicate that a segment HTTP request failure
    /// happened.
    pub fn jsSendSegmentRequestError(
        fatal: bool,
        url: &str,
        isInit: bool,
        timeInfo: Option<Vec<f64>>,
        mediaType: MediaType,
        reason: RequestErrorReason,
        status: Option<u32>,
    );

    /// Function to call to indicate that a Multivariant Playlist HTTP request failure happened.
    pub fn jsSendMultivariantPlaylistRequestError(
        fatal: bool,
        url: &str,
        reason: RequestErrorReason,
        status: Option<u32>,
    );

    /// Function to call to indicate that a Media Playlist HTTP request failure happened.
    pub fn jsSendMediaPlaylistRequestError(
        fatal: bool,
        url: &str,
        reason: RequestErrorReason,
        mediaType: MediaType,
        status: Option<u32>,
    );

    /// Function to call to indicate that an error arised on `SourceBuffer` creation.
    pub fn jsSendSourceBufferCreationError(
        fatal: bool,
        code: SourceBufferCreationErrorCode,
        media_type: MediaType,
        message: &str,
    );

    /// Function to call to indicate that an error arised when parsing the Multivariant Playlist.
    pub fn jsSendMultivariantPlaylistParsingError(
        fatal: bool,
        code: MultivariantPlaylistParsingErrorCode,
        message: &str,
    );

    /// Function to call to indicate that an error arised when parsing a Media Playlist.
    pub fn jsSendMediaPlaylistParsingError(
        fatal: bool,
        code: MediaPlaylistParsingErrorCode,
        media_type: MediaType,
        message: &str,
    );

    /// Function to call to indicate that an error arised when parsing a segment.
    pub fn jsSendSegmentParsingError(
        fatal: bool,
        code: SegmentParsingErrorCode,
        media_type: MediaType,
        message: &str,
    );

    /// Function to call to indicate that an error arised after pushing a segment to a
    /// `SourceBuffer`.
    pub fn jsSendPushedSegmentError(
        fatal: bool,
        code: PushedSegmentErrorCode,
        media_type: MediaType,
        message: &str,
    );

    /// Function to call to indicate that an error arised when removing data from a
    /// `SourceBuffer`.
    pub fn jsSendRemoveBufferError(fatal: bool, media_type: MediaType, message: &str);

    /// Function to call to indicate that an uncategorized error happened.
    pub fn jsSendOtherError(fatal: bool, code: OtherErrorCode, message: &str);
}

#[wasm_bindgen]
pub enum PlaylistType {
    MultivariantPlaylist,
    MediaPlaylist,
}

#[wasm_bindgen]
pub enum OtherErrorCode {
    NoSupportedVariant,
    UnfoundLockedVariant,
    MediaSourceAttachmentError,
    Unknown,
}

#[wasm_bindgen]
pub enum SourceBufferCreationErrorCode {
    AlreadyCreatedWithSameType,
    CantPlayType,
    EmptyMimeType,
    MediaSourceIsClosed,
    NoMediaSourceAttached,
    QuotaExceededError,
    Unknown,
}

#[wasm_bindgen]
pub enum RequestErrorReason {
    Timeout,
    Status,
    Error,
    Other,
}

// TODO some macro-based metaprogramming, instead of just repeating the same boilerplate for each
// result type, would be welcome

/// Errors that can arise when attempting to remove a MediaSource previously attached
/// to a media element.
#[wasm_bindgen]
pub enum RemoveMediaSourceErrorCode {
    /// Could not remove MediaSource from the media element because this `WaspHlsPlayer`
    /// had no MediaSource attached to its media element.
    NoMediaSourceAttached,

    /// Could not remove MediaSource from the media element because of an unknown error.
    UnknownError,
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
        Self {
            error: Some((err, desc)),
        }
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

/// Trait allowing to convert "JavaScript Results" as exposed by the JavaScript functions into
/// `Result` structs more idiomatic to Rust.
pub(crate) trait JsResult<T, E> {
    fn result(self) -> Result<T, (E, Option<String>)>;
}

/// Errors that can arise when attempting to update the duration of a MediaSource.
/// TODO defined errors when the MediaSource is closed and so on?
#[wasm_bindgen]
pub enum MediaSourceDurationUpdateErrorCode {
    /// The `WaspHlsPlayer` had no MediaSource attached to its media element.
    NoMediaSourceAttached,

    /// An unknown error arised
    UnknownError,
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
        Self {
            error: Some((err, desc)),
        }
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
    /// Could not attach MediaSource to the media element because of an unknown error.
    UnknownError,

    /// Could not attach MediaSource to the media element because no content is currently
    /// loaded.
    NoContentLoaded,
}

/// Errors that can arise when calling the `jsRemoveBuffer` JavaScript function.
#[wasm_bindgen]
pub enum RemoveBufferErrorCode {
    /// The operation failed because the SourceBuffer instance linked to the given
    /// `SourceBufferId` was not found.
    SourceBufferNotFound,

    /// The operation failed because of an unknown error.
    UnknownError,
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
        Self {
            error: Some((err, desc)),
        }
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
    /// The `WaspHlsPlayer` linked had no MediaSource attached to its media
    /// element.
    NoMediaSourceAttached,

    /// The operation failed because of an unknown error.
    UnknownError,
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
        Self {
            error: Some((err, desc)),
        }
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

/// Error that might arise when parsing the Multivariant Playlist.
#[wasm_bindgen]
pub enum MultivariantPlaylistParsingErrorCode {
    /// The first line of the MultivariantPlaylist is not #EXTM3U.
    ///
    /// Are you sure this is a Multivariant Playlist?
    MissingExtM3uHeader,
    /// The Multivariant Playlist has no variant.
    ///
    /// Are you sure this is a Multivariant Playlist and not a Media Playlist?
    MultivariantPlaylistWithoutVariant,
    /// An `EXT-X-STREAM-INF` tag announced in the Multivariant Playlist,
    /// describing an HLS variant, had no URI associated to it. It should be
    /// mandatory.
    MissingUriLineAfterVariant,
    /// An `EXT-X-STREAM-INF` tag announced in the Multivariant Playlist,
    /// describing an HLS variant, had an unreadable URI associated to it.
    UnableToReadVariantUri,
    /// An `EXT-X-STREAM-INF` tag announced in the Multivariant Playlist,
    /// describing an HLS variant, had no `BANDWIDTH` attribute associated to it.
    /// It should be mandatory.
    VariantMissingBandwidth,
    /// A value in the Multivariant Playlist was in an invalid format.
    InvalidValue,
    /// An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
    /// an HLS variant, had no `TYPE` attribute associated to it. It should be
    /// mandatory.
    MediaTagMissingType,
    /// An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
    /// an HLS variant, had no `NAME` attribute associated to it. It should be
    /// mandatory.
    MediaTagMissingName,
    /// An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
    /// an HLS variant, had no `GROUP-ID` attribute associated to it. It should be
    /// mandatory.
    MediaTagMissingGroupId,
    /// A line could not be read.
    UnableToReadLine,
    /// Another, uncategorized, error arised.
    Unknown,
}

impl From<MultivariantPlaylistParsingError> for MultivariantPlaylistParsingErrorCode {
    fn from(value: MultivariantPlaylistParsingError) -> Self {
        match value {
            MultivariantPlaylistParsingError::MissingExtM3uHeader => {
                MultivariantPlaylistParsingErrorCode::MissingExtM3uHeader
            }
            MultivariantPlaylistParsingError::InvalidDecimalInteger => {
                MultivariantPlaylistParsingErrorCode::InvalidValue
            }
            MultivariantPlaylistParsingError::MediaTagMissingType => {
                MultivariantPlaylistParsingErrorCode::MediaTagMissingType
            }
            MultivariantPlaylistParsingError::MediaTagMissingName => {
                MultivariantPlaylistParsingErrorCode::MediaTagMissingName
            }
            MultivariantPlaylistParsingError::MediaTagMissingGroupId => {
                MultivariantPlaylistParsingErrorCode::MediaTagMissingGroupId
            }
            MultivariantPlaylistParsingError::VariantMissingBandwidth => {
                MultivariantPlaylistParsingErrorCode::VariantMissingBandwidth
            }
            MultivariantPlaylistParsingError::MissingUriLineAfterVariant => {
                MultivariantPlaylistParsingErrorCode::MissingUriLineAfterVariant
            }
            MultivariantPlaylistParsingError::UnableToReadVariantUri
            | MultivariantPlaylistParsingError::UnableToReadLine
            | MultivariantPlaylistParsingError::Unknown => {
                MultivariantPlaylistParsingErrorCode::Unknown
            }
        }
    }
}

/// Error that might arise when parsing the Media Playlist.
#[wasm_bindgen]
pub enum MediaPlaylistParsingErrorCode {
    /// An `#EXTINF` tag announced in the Media Playlist was not in the right
    /// format.
    UnparsableExtInf,
    /// An `#EXT-X-MAP` tag in the Media Playlist didn't its mandatory `URI`
    /// attribute.
    UriMissingInMap,
    /// There was no `#EXT-X-TARGETDURATION` tag in the Media Playlist.
    MissingTargetDuration,
    /// One of the URI found in the Media Playlist wasn't associated to any
    /// `#EXTINF` tag.
    UriWithoutExtInf,
    /// A `#EXT-X-BYTERANGE` tag or a `BYTERANGE` attribute in the Media Playlist
    /// was not in the right format.
    UnparsableByteRange,
    /// Another, uncategorized, error arised.
    Unknown,
}

impl From<MediaPlaylistUpdateError> for MediaPlaylistParsingErrorCode {
    fn from(value: MediaPlaylistUpdateError) -> Self {
        match value {
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::UnparsableExtInf) => {
                MediaPlaylistParsingErrorCode::UnparsableExtInf
            }
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::UriMissingInMap) => {
                MediaPlaylistParsingErrorCode::UriMissingInMap
            }
            MediaPlaylistUpdateError::ParsingError(
                MediaPlaylistParsingError::MissingTargetDuration,
            ) => MediaPlaylistParsingErrorCode::MissingTargetDuration,
            MediaPlaylistUpdateError::ParsingError(
                MediaPlaylistParsingError::UnparsableByteRange,
            ) => MediaPlaylistParsingErrorCode::UnparsableByteRange,
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::UriWithoutExtInf) => {
                MediaPlaylistParsingErrorCode::UriWithoutExtInf
            }
            MediaPlaylistUpdateError::NotFound => MediaPlaylistParsingErrorCode::Unknown,
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
    /// The `WaspHlsPlayer` linked to it had no MediaSource attached to its media
    /// element.
    NoMediaSourceAttached,

    /// The `MediaSource` instance linked to this `WaspHlsPlayer` is in a "closed" state.
    MediaSourceIsClosed,

    /// A `QuotaExceededError` was received while trying to add the `SourceBuffer`
    ///
    /// Such errors are often encountered when another SourceBuffer attached to the same
    /// MediaSource instance was already updated through a buffer operation.
    QuotaExceededError,

    /// The given mime-type and codec combination is not supported
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
    pub fn success(val: SourceBufferId) -> Self {
        Self {
            source_buffer_id: val,
            error: None,
        }
    }

    /// Creates an `AddSourceBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: AddSourceBufferErrorCode, desc: Option<String>) -> Self {
        Self {
            source_buffer_id: 0,
            error: Some((err, desc)),
        }
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
        Self {
            error: Some((err, desc)),
        }
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
    error: Option<(SegmentParsingErrorCode, Option<String>)>,
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
    pub fn success(start: Option<f64>, duration: Option<f64>) -> Self {
        Self {
            success: Some(ParsedSegmentInfo { start, duration }),
            error: None,
        }
    }

    /// Creates an `AppendBufferResult` indicating failure, with the corresponding
    /// error.
    ///
    /// This function should only be called by the JavaScript-side.
    pub fn error(err: SegmentParsingErrorCode, desc: Option<String>) -> Self {
        Self {
            success: None,
            error: Some((err, desc)),
        }
    }
}

impl JsResult<Option<ParsedSegmentInfo>, SegmentParsingErrorCode> for AppendBufferResult {
    /// Basically unwrap and consume the `AppendBufferResult`, converting it into a
    /// Result enum.
    fn result(
        self,
    ) -> Result<Option<ParsedSegmentInfo>, (SegmentParsingErrorCode, Option<String>)> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(self.success)
        }
    }
}

/// Errors that can arise when calling the `jsAppendBuffer` JavaScript function.
#[wasm_bindgen]
pub enum SegmentParsingErrorCode {
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

impl From<PushSegmentError> for SegmentParsingErrorCode {
    fn from(value: PushSegmentError) -> Self {
        match value {
            PushSegmentError::NoResource(_) => SegmentParsingErrorCode::NoResource,
            PushSegmentError::NoSourceBuffer(_) => SegmentParsingErrorCode::NoSourceBuffer,
            PushSegmentError::TransmuxerError(_, _) => SegmentParsingErrorCode::TransmuxerError,
            PushSegmentError::UnknownError(_, _) => SegmentParsingErrorCode::UnknownError,
        }
    }
}

/// Errors that can arise after a SourceBuffer's `appendBuffer` call.
#[wasm_bindgen]
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum PushedSegmentErrorCode {
    /// We could not push the segment because the `SourceBuffer`'s buffer seems full.
    BufferFull,
    /// We could not push the segment because of another, unknown error.
    UnknownError,
}

/// Current playback information associated to the `HTMLMediaElement` displayed
/// on the page.
/// `PlaybackObservation` should be regularly sent on various events.
#[wasm_bindgen]
pub struct PlaybackObservation {
    /// The reason that triggered the `PlaybackObservation` struct to be
    /// created.
    reason: PlaybackObservationReason,
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

    /// The timer is linked to a failed request that has to be retried.
    RetryRequest = 1,
}

/// Levels with which a log can be emitted.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
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

/// Identify a pending request.
pub type RequestId = u32;

/// Identify a pending timer.
pub type TimerId = f64;

/// Identify a SourceBuffer.
pub type SourceBufferId = u32;

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    Audio = 0,
    Video = 1,
}

impl fmt::Display for MediaType {
    /// When wanting to display the value, just format Audio as "audio" and
    /// Video as "video"
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MediaType::Audio => "audio",
                MediaType::Video => "video",
            }
        )
    }
}

/// Values for the "Playlist Type" as specified by the HLS specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[wasm_bindgen]
pub enum PlaylistNature {
    Event,
    VoD,
    Live,
    Unknown,
}
