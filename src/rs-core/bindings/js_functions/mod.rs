#![allow(non_snake_case)]

use super::{
    AddSourceBufferErrorCode, AttachMediaSourceErrorCode, ContentCompatibilityErrorCode,
    EndOfStreamErrorCode, LogLevel, MediaPlaylistParsingErrorCode,
    MediaSourceDurationUpdateErrorCode, MediaType, MultivariantPlaylistParsingErrorCode,
    OtherErrorCode, PlaylistNature, PlaylistType, PushedSegmentErrorCode, RemoveBufferErrorCode,
    RemoveMediaSourceErrorCode, RequestErrorReason, SegmentParsingErrorCode,
    SourceBufferCreationErrorCode, TimerReason,
};
use crate::{
    media_element::{PushSegmentError, SegmentHints},
    parser::{
        MediaPlaylistParsingError, MediaPlaylistUpdateError, MultivariantPlaylistParsingError,
    },
};
use std::{fmt, slice};

// # js_functions
//
// This file lists all JavaScript functions that are callable from Rust as well as
// struct and enumeration used by those functions.

#[cfg(not(target_arch = "wasm32"))]
mod raw_host;
#[cfg(target_arch = "wasm32")]
mod raw_wasm;

#[cfg(not(target_arch = "wasm32"))]
use raw_host::*;
#[cfg(target_arch = "wasm32")]
use raw_wasm::*;

fn bool_to_raw(value: bool) -> u32 {
    if value {
        1
    } else {
        0
    }
}

fn raw_to_opt_u32(value: u32) -> Option<u32> {
    if value == u32::MAX {
        None
    } else {
        Some(value)
    }
}

fn opt_u32_to_raw(value: Option<u32>) -> u32 {
    value.unwrap_or(u32::MAX)
}

fn opt_f64_to_raw_ptr(value: Option<f64>) -> *const f64 {
    match value {
        Some(ref v) => v as *const f64,
        None => std::ptr::null(),
    }
}

fn owned_string_from_abi(ptr: u32, len: u32) -> String {
    let bytes = unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) };
    String::from_utf8(bytes).unwrap_or_default()
}

fn opt_owned_string_from_abi(ptr: u32, len: u32) -> Option<String> {
    if ptr == 0 {
        None
    } else {
        Some(owned_string_from_abi(ptr, len))
    }
}

#[derive(Default)]
struct JsErrorOut {
    code: u32,
    desc_ptr: u32,
    desc_len: u32,
}

fn take_js_error_out<E>(out: JsErrorOut, map: impl FnOnce(u32) -> E) -> (E, Option<String>) {
    (
        map(out.code),
        opt_owned_string_from_abi(out.desc_ptr, out.desc_len),
    )
}

pub fn jsLog(log_level: LogLevel, log: &str) {
    unsafe { __js_func__log(log_level as u32, log.as_ptr(), log.len() as u32) }
}

/// Starts a timer for the number of milliseconds indicated by the `duration` argument.
///
/// Once this timer has elapsed, and unless `jsClearTimer` has been called since with
/// the `TimerId` returned by this function, the `on_timer_ended` of this
/// `WaspHlsPlayer` will be called with both the corresponding `TimerId` and `reason`,
/// which you can use on your side to better categorize timer categories.
pub fn jsTimer(duration: f64, reason: TimerReason) -> TimerId {
    unsafe { __js_func__timer(duration, reason as u32) }
}

/// Clear a timer started with `jsTimer`.
pub fn jsClearTimer(id: TimerId) {
    unsafe { __js_func__clear_timer(id) }
}

/// Returns the data, as a vector of bytes of a resource behind a `ResourceId`.
///
/// Returns `None` if that `ResourceId` is not linked to any resource right now.
/// TODO also return ContentType of the requested data?
pub fn jsGetResourceData(id: ResourceId) -> Option<Vec<u8>> {
    let len = unsafe { __js_func__get_resource_len(id) };
    if len < 0 {
        return None;
    }
    let len = len as usize;
    let mut data = vec![0_u8; len];
    let copied = unsafe { __js_func__copy_resource_data(id, data.as_mut_ptr(), len as u32) };
    if copied == 0 {
        None
    } else {
        Some(data)
    }
}

/// Fetch the given `url` from the network and await a response.
/// If at least the `range_base` argument is set, the request will be a range request from
/// `range_base` to `range_end` or to the end of the resource if `range_end` is set to `None`.
///
/// A timeout in milliseconds may also be communicated to `jsFetch`, after which the request will
/// automatically be aborted and failure will be reported. To disable any timeout, you can set
/// the `timeout` argument to a negative value.
///
/// If and when it finishes with success, the result will be emitted as a `resource_id` through
/// the `on_request_finished` method of this `WaspHlsPlayer`.
///
/// If and when it fails, the error will be emitted through the `on_request_failed` method of this
/// `WaspHlsPlayer`.
///
/// In both cases, those methods will always be called asynchronously after the `jsFetch` call.
///
/// If the request has been aborted while pending through the `jsAbortRequest` function, none of
/// those methods will be called.
///
/// The resource requested is actually kept in JavaScript's memory to avoid unnecesary copies of
/// larges amount of data (and to avoid stressing JavaScript's garbage collector in case where
/// the data would go back and forth between JavaScript and WASM).
///
/// To avoid memory leaks, it is __VERY__ important to call the `jsFreeResource` function with
/// that `ResourceId` once it is not needed anymore.
///
/// Bear in mind that the JavaScript-side bears the right to free the resource even if not
/// explicitely asked through a `JsFreeResource` call, in cases where a method from the current
/// `WaspHlsPlayer` unexpectedly throws. This is again to avoid leaking memory.
///
/// In that last scenario, you will receive a corresponding error when trying to use that
/// `ResourceId` in the JavaScript functions receiving it.
pub fn jsFetch(
    url: &str,
    range_base: Option<usize>,
    range_end: Option<usize>,
    timeout: f64,
) -> RequestId {
    unsafe {
        __js_func__fetch(
            url.as_ptr(),
            url.len() as u32,
            bool_to_raw(range_base.is_some()),
            range_base.unwrap_or(0),
            bool_to_raw(range_end.is_some()),
            range_end.unwrap_or(0),
            timeout,
        )
    }
}

/// Abort a request started with `jsFetch`` based on its
/// `request_id`.
///
/// After calling this function, you won't get any event linked to that
/// request ever again.
/// Note that this RequestId may now be re-used in the future for any other
/// future request.
///
/// Returns `true` if a pending request with the given RequestId was found and aborted,
/// `false` if no pending request was found with that RequestId.
pub fn jsAbortRequest(request_id: RequestId) -> bool {
    unsafe { __js_func__abort_request(request_id) != 0 }
}

/// Create MediaSource and attach it to the <video> element associated with
/// this `WaspHlsPlayer`.
///
/// This function performs the MediaSource creation and attachment
/// synchronously. Yet the MediaSource is not usable right away (e.g. it is
/// not immediately possible to open SourceBuffers on it.
/// This `WaspHlsPlayer` instance will know when this MediaSource becomes usable or not
/// when its `on_media_source_state_change` method is called with the "Open"
/// `MediaSourceReadyState`.
pub fn jsAttachMediaSource() -> Result<(), (AttachMediaSourceErrorCode, Option<String>)> {
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__attach_media_source(&mut out.code, &mut out.desc_ptr, &mut out.desc_len)
    };
    if success != 0 {
        Ok(())
    } else {
        Err(take_js_error_out(out, AttachMediaSourceErrorCode::from_raw))
    }
}

/// Remove MediaSource attached to the <video> element associated with
/// the `WaspHlsPlayer` if one, and free all its associated resources
/// (such as event listeners or created ObjectURL).
///
/// This function performs all those operations synchronously.
pub fn jsRemoveMediaSource() -> Result<(), (RemoveMediaSourceErrorCode, Option<String>)> {
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__remove_media_source(&mut out.code, &mut out.desc_ptr, &mut out.desc_len)
    };
    if success != 0 {
        Ok(())
    } else {
        Err(take_js_error_out(out, RemoveMediaSourceErrorCode::from_raw))
    }
}

/// Update the duration in seconds of the MediaSource attached to this WaspHlsPlayer.
pub fn jsSetMediaSourceDuration(
    duration: f64,
) -> Result<(), (MediaSourceDurationUpdateErrorCode, Option<String>)> {
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__set_media_source_duration(
            duration,
            &mut out.code,
            &mut out.desc_ptr,
            &mut out.desc_len,
        )
    };
    if success != 0 {
        Ok(())
    } else {
        Err(take_js_error_out(
            out,
            MediaSourceDurationUpdateErrorCode::from_raw,
        ))
    }
}

/// Add a SourceBuffer to the created MediaSource, allowing to push media
/// segment of a given type to a lower-level media buffer.
///
/// This function performs this operation synchronously and may fail, see
/// `AddSourceBufferResult` for more details on the return value.
pub fn jsAddSourceBuffer(
    media_type: MediaType,
    typ: &str,
) -> Result<SourceBufferId, (AddSourceBufferErrorCode, Option<String>)> {
    let mut source_buffer_id = 0;
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__add_source_buffer(
            media_type as u32,
            typ.as_ptr(),
            typ.len() as u32,
            &mut source_buffer_id,
            &mut out.code,
            &mut out.desc_ptr,
            &mut out.desc_len,
        )
    };
    if success != 0 {
        Ok(source_buffer_id)
    } else {
        Err(take_js_error_out(out, AddSourceBufferErrorCode::from_raw))
    }
}

pub fn jsIsTypeSupported(media_type: MediaType, typ: &str) -> Option<bool> {
    match unsafe { __js_func__is_type_supported(media_type as u32, typ.as_ptr(), typ.len() as u32) }
    {
        -1 => None,
        0 => Some(false),
        _ => Some(true),
    }
}

/// Recuperate more information on a segment, identified by its `ResourceId`.
///
/// This can generally be relied on to e.g. obtain the `codec` and mime-type
/// directly from segment metadata.
///
/// # Arguments
///
/// * `segment_id` - `ResourceId` of the segment you want more metadata from
///
/// # Returns
///
/// A result:
///
/// - When `Ok`, transports the parsed metadata from the segment
/// - When `Err`, transports both the error code that prevented the inspection,
///   and an optional description as a `String`.
pub fn jsInspectSegment(
    segment_id: ResourceId,
) -> Result<InspectedSegmentMetadata, (SegmentParsingErrorCode, Option<String>)> {
    let mut media_type = 0;
    let mut parsed_mime_type_ptr = 0;
    let mut parsed_mime_type_len = 0;
    let mut codec_ptr = 0;
    let mut codec_len = 0;
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__inspect_segment(
            segment_id,
            &mut media_type,
            &mut parsed_mime_type_ptr,
            &mut parsed_mime_type_len,
            &mut codec_ptr,
            &mut codec_len,
            &mut out.code,
            &mut out.desc_ptr,
            &mut out.desc_len,
        )
    };
    if success == 0 {
        return Err(take_js_error_out(out, SegmentParsingErrorCode::from_raw));
    }
    Ok(InspectedSegmentMetadata {
        media_type: MediaType::from_raw(media_type),
        mime_type: owned_string_from_abi(parsed_mime_type_ptr, parsed_mime_type_len),
        codec: owned_string_from_abi(codec_ptr, codec_len),
    })
}

/// Append media data to the given SourceBuffer.
///
/// This process is asynchronous, meaning that the data might not be appended
/// directly after calling `jsAppendBuffer`.
///
/// Append and remove operations performed on that SourceBuffer, respectively
/// through the `jsAppendBuffer` and `jsRemoveBuffer` functions, are all
/// pushed to an internal queue of operations which will be executed in the
/// same order than their calls have been made.
/// You will be notified once each single one of these operations have
/// succeeded when the `on_source_buffer_update` function is called on this
/// `WaspHlsPlayer` instance, with the same `source_buffer_id`.
///
/// If the `on_source_buffer_error` method of this `WaspHlsPlayer` instance,
/// with the same `source_buffer_id`, it means that the currently scheduled
/// operation (the first one in the queue) failed. In that case, the
/// SourceBuffer is not usable anymore.
pub fn jsAppendBuffer(
    source_buffer_id: SourceBufferId,
    segment_id: ResourceId,
    segment_hints: &SegmentHints,
) -> Result<Option<ParsedSegmentInfo>, (SegmentParsingErrorCode, Option<String>)> {
    let mut has_start = 0;
    let mut start_value_hi = 0;
    let mut start_value_lo = 0;

    let mut has_end = 0;
    let mut end_value_hi = 0;
    let mut end_value_lo = 0;

    let mut timescale = 1;

    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__append_buffer(
            source_buffer_id,
            segment_id,
            (segment_hints.base_decode_time_start() >> 32) as u32,
            segment_hints.base_decode_time_start() as u32,
            segment_hints.base_decode_time_start_timescale(),
            segment_hints.reset_transmuxer_state() as u32,
            &mut has_start,
            &mut start_value_hi,
            &mut start_value_lo,
            &mut has_end,
            &mut end_value_hi,
            &mut end_value_lo,
            &mut timescale,
            &mut out.code,
            &mut out.desc_ptr,
            &mut out.desc_len,
        )
    };
    if success == 0 {
        return Err(take_js_error_out(out, SegmentParsingErrorCode::from_raw));
    }
    Ok(Some(ParsedSegmentInfo {
        start: if has_start != 0 {
            Some(((start_value_hi as u64) << 32) | start_value_lo as u64)
        } else {
            None
        },
        end: if has_end != 0 {
            Some(((end_value_hi as u64) << 32) | end_value_lo as u64)
        } else {
            None
        },
        timescale,
    }))
}

/// Remove media data from the given SourceBuffer.
///
/// This process is asynchronous, meaning that the data might not be directly
/// considered after calling `jsRemoveBuffer`.
///
/// Append and remove operations performed on that SourceBuffer, respectively
/// through the `jsAppendBuffer` and `jsRemoveBuffer` functions, are all
/// pushed to an internal queue of operations which will be executed in the
/// same order than their calls have been made.
/// You will be notified once each single one of these operations have
/// succeeded when the `on_source_buffer_update` function is called on this
/// `WaspHlsPlayer` instance, with the same `source_buffer_id`.
///
/// If the `on_source_buffer_error` method of this `WaspHlsPlayer` instance,
/// with the same `source_buffer_id`, it means that the currently scheduled
/// operation (the first one in the queue) failed. In that case, the
/// SourceBuffer is not usable anymore.
pub fn jsRemoveBuffer(
    source_buffer_id: SourceBufferId,
    start: f64,
    end: f64,
) -> Result<(), (RemoveBufferErrorCode, Option<String>)> {
    let mut out = JsErrorOut::default();
    let success = unsafe {
        __js_func__remove_buffer(
            source_buffer_id,
            start,
            end,
            &mut out.code,
            &mut out.desc_ptr,
            &mut out.desc_len,
        )
    };
    if success != 0 {
        Ok(())
    } else {
        Err(take_js_error_out(out, RemoveBufferErrorCode::from_raw))
    }
}

/// Call the `MediaSource.prototype.endOfStream` API, allowing to signal that
/// all contents have been pushed to all of its buffer.
///
/// Note that you should make sure that all of the buffers have an empty queue
/// of operations (no `jsAppendBuffer` or `jsRemoveBuffer` call not yet
/// validated through a `on_source_buffer_update` callback) before making the
/// `jsEndOfStream` call.
pub fn jsEndOfStream() -> Result<(), (EndOfStreamErrorCode, Option<String>)> {
    let mut out = JsErrorOut::default();
    let success =
        unsafe { __js_func__end_of_stream(&mut out.code, &mut out.desc_ptr, &mut out.desc_len) };
    if success != 0 {
        Ok(())
    } else {
        Err(take_js_error_out(out, EndOfStreamErrorCode::from_raw))
    }
}

/// After this method is called, this `WaspHlsPlayer` instance will regularly receive
/// `PlaybackObservation` objects, describing the current playback conditions through
/// its `on_playback_tick` method.
/// The first event will be sent right away, though asynchronously.
///
/// You can stop receiving those observations by calling
/// `stopObservingPlayback` and restart it by calling `startObservingPlayback` a new
/// time.
///
/// If this `WaspHlsPlayer` was already observing playback when that function
/// was called, this function does nothing.
pub fn jsStartObservingPlayback() {
    unsafe { __js_func__start_observing_playback() }
}

/// If playback observations were being regularly sent to this
/// `WaspHlsPlayer` instance, stop emitting them until `startObservingPlayback` is
/// called again.
pub fn jsStopObservingPlayback() {
    unsafe { __js_func__stop_observing_playback() }
}

/// Free resource stored in JavaScript's memory kept alive for the current
/// `WaspHlsPlayer`.
pub fn jsFreeResource(resource_id: ResourceId) -> bool {
    unsafe { __js_func__free_resource(resource_id) != 0 }
}

/// Method called to change the playback rate (speed of playback).
/// This can be both in response to API input or to start/exit buffering by
/// example.
pub fn jsSetPlaybackRate(playback_rate: f64) {
    unsafe { __js_func__set_playback_rate(playback_rate) }
}

/// Call the `HTMLMediaElement.prototype.seek` API, allowing to move the current
/// playback's playhead.
pub fn jsSeek(position: f64) {
    unsafe { __js_func__seek(position) }
}

pub fn jsFlush() {
    unsafe { __js_func__flush() }
}

/// Method called to indicate the offset to convert playlist time, as anounced in the
/// MediaPlaylist (and which should be preferred for a user interface) into media time,
/// which is the time actually present on the HTMLMediaElement.
pub fn jsSetMediaOffset(media_offset: f64) {
    unsafe { __js_func__set_media_offset(media_offset) }
}

pub fn jsUpdateContentInfo(
    minimum_position: Option<f64>,
    maximum_position: Option<f64>,
    playlist_nat: PlaylistNature,
    is_finalized: bool,
    uses_program_date_time: bool,
) {
    unsafe {
        __js_func__update_content_info(
            bool_to_raw(minimum_position.is_some()),
            minimum_position.unwrap_or(0.),
            bool_to_raw(maximum_position.is_some()),
            maximum_position.unwrap_or(0.),
            playlist_nat as u32,
            bool_to_raw(is_finalized),
            bool_to_raw(uses_program_date_time),
        )
    }
}

pub fn jsAnnounceFetchedContent(
    playlist_type: PlaylistType,
    variant_info: Vec<u32>,
    audio_tracks_info: Vec<u32>,
) {
    unsafe {
        __js_func__announce_fetched_content(
            playlist_type as u32,
            variant_info.as_ptr(),
            variant_info.len() as u32,
            audio_tracks_info.as_ptr(),
            audio_tracks_info.len() as u32,
        )
    }
}

pub fn jsAnnounceVariantUpdate(variant_id: Option<u32>) {
    unsafe { __js_func__announce_variant_update(opt_u32_to_raw(variant_id)) }
}

pub fn jsAnnounceTrackUpdate(media_type: MediaType, track_id: Option<u32>, is_fixed: bool) {
    unsafe {
        __js_func__announce_track_update(
            media_type as u32,
            opt_u32_to_raw(track_id),
            bool_to_raw(is_fixed),
        )
    }
}

pub fn jsAnnounceVariantLockStatusChange(variant_id: Option<u32>) {
    unsafe { __js_func__announce_variant_lock_status_change(opt_u32_to_raw(variant_id)) }
}

pub fn jsStartRebuffering() {
    unsafe { __js_func__start_rebuffering() }
}

pub fn jsStopRebuffering() {
    unsafe { __js_func__stop_rebuffering() }
}

pub fn jsGetRandom() -> f64 {
    unsafe { __js_func__get_random() }
}

/// Function to call to indicate that a segment HTTP request failure
/// happened.
pub fn jsSendSegmentRequestError(
    fatal: bool,
    url: &str,
    is_init: bool,
    time_info: Option<(f64, f64)>,
    media_type: Option<MediaType>,
    reason: RequestErrorReason,
    status: Option<u32>,
) {
    let start = time_info.map(|t| t.0);
    let duration = time_info.map(|t| t.1);
    unsafe {
        __js_func__send_segment_request_error(
            bool_to_raw(fatal),
            url.as_ptr(),
            url.len() as u32,
            bool_to_raw(is_init),
            opt_u32_to_raw(media_type.map(|m| m as u32)),
            opt_f64_to_raw_ptr(start),
            opt_f64_to_raw_ptr(duration),
            reason as u32,
            opt_u32_to_raw(status),
        )
    }
}

/// Function to call to indicate that a Multivariant Playlist HTTP request failure happened.
pub fn jsSendMultivariantPlaylistRequestError(
    fatal: bool,
    url: &str,
    reason: RequestErrorReason,
    status: Option<u32>,
) {
    unsafe {
        __js_func__send_multivariant_playlist_request_error(
            bool_to_raw(fatal),
            url.as_ptr(),
            url.len() as u32,
            reason as u32,
            opt_u32_to_raw(status),
        )
    }
}

/// Function to call to indicate that a Media Playlist HTTP request failure happened.
pub fn jsSendMediaPlaylistRequestError(
    fatal: bool,
    url: &str,
    reason: RequestErrorReason,
    media_type: Option<MediaType>,
    status: Option<u32>,
) {
    unsafe {
        __js_func__send_media_playlist_request_error(
            bool_to_raw(fatal),
            url.as_ptr(),
            url.len() as u32,
            reason as u32,
            opt_u32_to_raw(media_type.map(|m| m as u32)),
            opt_u32_to_raw(status),
        )
    }
}

/// Function to call to indicate that an error arised on `SourceBuffer` creation.
pub fn jsSendSourceBufferCreationError(
    fatal: bool,
    code: SourceBufferCreationErrorCode,
    media_type: MediaType,
    message: &str,
) {
    unsafe {
        __js_func__send_source_buffer_creation_error(
            bool_to_raw(fatal),
            code as u32,
            media_type as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an error arised when parsing the Multivariant Playlist.
pub fn jsSendMultivariantPlaylistParsingError(
    fatal: bool,
    code: MultivariantPlaylistParsingErrorCode,
    message: &str,
) {
    unsafe {
        __js_func__send_multivariant_playlist_parsing_error(
            bool_to_raw(fatal),
            code as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an error arised when parsing a Media Playlist.
pub fn jsSendMediaPlaylistParsingError(
    fatal: bool,
    code: MediaPlaylistParsingErrorCode,
    media_type: Option<MediaType>,
    message: &str,
) {
    unsafe {
        __js_func__send_media_playlist_parsing_error(
            bool_to_raw(fatal),
            code as u32,
            opt_u32_to_raw(media_type.map(|m| m as u32)),
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an error arised when parsing a segment.
pub fn jsSendSegmentParsingError(
    fatal: bool,
    code: SegmentParsingErrorCode,
    media_type: Option<MediaType>,
    message: &str,
) {
    unsafe {
        __js_func__send_segment_parsing_error(
            bool_to_raw(fatal),
            code as u32,
            opt_u32_to_raw(media_type.map(|m| m as u32)),
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an error arised after pushing a segment to a
/// `SourceBuffer`.
pub fn jsSendPushedSegmentError(
    fatal: bool,
    code: PushedSegmentErrorCode,
    media_type: MediaType,
    message: &str,
) {
    unsafe {
        __js_func__send_pushed_segment_error(
            bool_to_raw(fatal),
            code as u32,
            media_type as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an error arised when removing data from a
/// `SourceBuffer`.
pub fn jsSendRemoveBufferError(fatal: bool, media_type: MediaType, message: &str) {
    unsafe {
        __js_func__send_remove_buffer_error(
            bool_to_raw(fatal),
            media_type as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that an uncategorized error happened.
pub fn jsSendOtherError(fatal: bool, code: OtherErrorCode, message: &str) {
    unsafe {
        __js_func__send_other_error(
            bool_to_raw(fatal),
            code as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}

/// Function to call to indicate that the content is incompatible with the current environment.
pub fn jsSendContentCompatibilityError(
    fatal: bool,
    code: ContentCompatibilityErrorCode,
    message: &str,
) {
    unsafe {
        __js_func__send_content_compatibility_error(
            bool_to_raw(fatal),
            code as u32,
            message.as_ptr(),
            message.len() as u32,
        )
    }
}
impl From<MultivariantPlaylistParsingError> for MultivariantPlaylistParsingErrorCode {
    fn from(value: MultivariantPlaylistParsingError) -> Self {
        match value {
            MultivariantPlaylistParsingError::InvalidDecimalInteger => {
                MultivariantPlaylistParsingErrorCode::InvalidValue
            }
            MultivariantPlaylistParsingError::MediaTagMissingType
            | MultivariantPlaylistParsingError::MediaTagMissingName
            | MultivariantPlaylistParsingError::MediaTagMissingGroupId => {
                MultivariantPlaylistParsingErrorCode::MissingRequiredAttribute
            }
            MultivariantPlaylistParsingError::VariantMissingBandwidth => {
                MultivariantPlaylistParsingErrorCode::VariantMissingBandwidth
            }
            MultivariantPlaylistParsingError::DuplicateTag => {
                MultivariantPlaylistParsingErrorCode::DuplicateTag
            }
            MultivariantPlaylistParsingError::ConflictingPlaylistTagTypes => {
                MultivariantPlaylistParsingErrorCode::ConflictingPlaylistTagTypes
            }
            MultivariantPlaylistParsingError::MissingUriLineAfterVariant => {
                MultivariantPlaylistParsingErrorCode::MissingUriLineAfterVariant
            }
            MultivariantPlaylistParsingError::UnableToReadVariantUri
            | MultivariantPlaylistParsingError::UnableToReadLine
            | MultivariantPlaylistParsingError::Unknown => {
                MultivariantPlaylistParsingErrorCode::Unknown
            }
            MultivariantPlaylistParsingError::VariableDefinition(_) => {
                MultivariantPlaylistParsingErrorCode::VariableDefinitionError
            }
        }
    }
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
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::DuplicateTag) => {
                MediaPlaylistParsingErrorCode::DuplicateTag
            }
            MediaPlaylistUpdateError::ParsingError(
                MediaPlaylistParsingError::ConflictingPlaylistTagTypes,
            ) => MediaPlaylistParsingErrorCode::ConflictingPlaylistTagTypes,
            MediaPlaylistUpdateError::ParsingError(
                MediaPlaylistParsingError::VariableDefinition(_),
            ) => MediaPlaylistParsingErrorCode::VariableDefinitionError,
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::UnableToReadLine) => {
                MediaPlaylistParsingErrorCode::Unknown
            }
            MediaPlaylistUpdateError::NotFound => MediaPlaylistParsingErrorCode::Unknown,
        }
    }
}

impl From<MediaPlaylistParsingError> for MediaPlaylistParsingErrorCode {
    fn from(value: MediaPlaylistParsingError) -> Self {
        MediaPlaylistUpdateError::ParsingError(value).into()
    }
}

/// Return value of the `jsAppendBuffer` call, corresponding to the obtained
/// data after both potentially transmuxing and inspecting a media segment.
pub struct ParsedSegmentInfo {
    /// Its precise start time if known, in the original timescale
    start: Option<u64>,
    /// Its precise end time if known, in the original timescale
    end: Option<u64>,
    /// Timescale associated to `start` and `end`.
    timescale: u32,
}

impl ParsedSegmentInfo {
    /// Obtain precise start time with its original timescale
    pub(crate) fn start(&self) -> Option<u64> {
        self.start
    }
    /// Obtain precise end time with its original timescale
    pub(crate) fn end(&self) -> Option<u64> {
        self.end
    }
    /// Obtain the timescale associated to the parsed timing values.
    pub(crate) fn timescale(&self) -> u32 {
        self.timescale
    }
}

/// Precise timestamp value represented as an integer and its associated timescale.
#[derive(Clone, Copy, Debug)]
pub struct TimescaledTimestamp {
    value: u64,
    timescale: u32,
}

impl TimescaledTimestamp {
    pub(crate) fn new(value: u64, timescale: u32) -> Self {
        Self { value, timescale }
    }

    pub(crate) fn value(self) -> u64 {
        self.value
    }

    pub(crate) fn timescale(self) -> u32 {
        self.timescale
    }

    pub(crate) fn to_f64_seconds(self) -> f64 {
        self.value as f64 / self.timescale as f64
    }
}

/// Result of a segment inspection (e.g. when calling `jsInspectSegment`)
pub struct InspectedSegmentMetadata {
    /// The container-level mime-type inferred from the segment's metadata itself.
    pub mime_type: String,
    /// The media type associated to the inspected segment.
    pub media_type: MediaType,
    /// The identified codec string from the segment's metadata itself.
    /// e.g. `mp4a.40.2` or `avc1.4D401F` etc.
    pub codec: String,
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

impl fmt::Display for MediaType {
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

pub(crate) fn f64_vec_from_abi(ptr: *const f64, len: usize) -> Vec<f64> {
    if len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(ptr, len).to_vec() }
    }
}
