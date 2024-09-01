use std::collections::VecDeque;

use crate::bindings::{
    jsAddSourceBuffer, jsAppendBuffer, jsFlush, jsRemoveBuffer, AddSourceBufferErrorCode, JsResult,
    MediaType, ParsedSegmentInfo, ResourceId, SegmentParsingErrorCode, SourceBufferId,
};
use crate::dispatcher::JsMemoryBlob;
use crate::parser::SegmentTimeInfo;
use crate::Logger;

/// Abstraction over the Media Source Extension's `SourceBuffer` concept.
///
/// This is the interface allowing to interact with lower-level media buffers.
pub(super) struct SourceBuffer {
    /// The `SourceBufferId` given on SourceBuffer creation, used to identify
    /// this `SourceBuffer` in the current dispatcher instance.
    id: SourceBufferId,

    /// The current queue of operations being performed on the `SourceBuffer`.
    ///
    /// From the most imminent to the least.
    queue: VecDeque<SourceBufferQueueElement>,

    /// The Content-Type currently linked to the SourceBuffer
    typ: String,

    /// Set to `true` as soon as the first operation is being performed, at
    /// which point, some actions cannot be taken anymore (like creating other
    /// `SourceBuffer` instances).
    was_used: bool,

    /// If `true` the chronologically last possible media chunk has been
    /// scheduled to be pushed.
    /// This allows for example to properly "end" the `SourceBuffer`.
    last_segment_pushed: bool,

    /// The MediaType associated to this `SourceBuffer`.
    media_type: MediaType,

    /// If `true`, the `SourceBuffer` was very recently emptied.
    ///
    /// In that situation, various decoding issues may occur after new data is pushed to the
    /// buffer, so special considerations, such as calling the `jsFlush` function might need to be
    /// taken on buffer updates.
    needs_reflush: bool,
}

impl SourceBuffer {
    /// Create a new `SourceBuffer` for the given `MediaType` and the mime-type indicated by `typ`.
    ///
    /// # Arguments
    ///
    /// * `media_type` - The `MediaType` that will handle this `SourceBuffer`. A `SourceBuffer`
    ///   handling multiple times at once is expected to:
    ///     1. At least contain video content
    ///     2. Be set to `MediaType::Video`, even if it also contains audio for example
    ///
    /// * `mime_type` - Mime-type to use when creating this `SourceBuffer` on the JavaScript-side.
    pub(super) fn new(media_type: MediaType, typ: String) -> Result<Self, AddSourceBufferError> {
        Logger::info(&format!("Creating new {} SourceBuffer", media_type));
        match jsAddSourceBuffer(media_type, &typ).result() {
            Ok(x) => Ok(Self {
                id: x,
                typ,
                queue: VecDeque::new(),
                was_used: false,
                needs_reflush: false,
                last_segment_pushed: false,
                media_type,
            }),
            Err(err) => Err(AddSourceBufferError::from_js_add_source_buffer_error(
                err, &typ,
            )),
        }
    }

    /// Returns the `SourceBufferId` needed to refer to that SourceBuffer when interacting with
    /// JavaScript.
    pub(super) fn id(&self) -> SourceBufferId {
        self.id
    }

    /// Returns the `MediaType` linked to that SourceBuffer.
    pub(super) fn media_type(&self) -> MediaType {
        self.media_type
    }

    /// Returns the mime-type linked to that SourceBuffer.
    pub(super) fn mime_type(&self) -> &str {
        &self.typ
    }

    /// Returns `true` if there is at least one pending buffer operation that
    /// isn't finished yet.
    pub(super) fn has_operations_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Pushes a new initialization segment to the underlying `SourceBuffer.
    ///
    /// # Arguments
    ///
    /// * `segment_data` - Actual initialization segment's data.
    pub(super) fn push_init_segment(
        &mut self,
        segment_data: JsMemoryBlob,
    ) -> Result<AppendBufferResponse, PushSegmentError> {
        self.was_used = true;
        self.queue
            .push_back(SourceBufferQueueElement::PushInit(segment_data.id()));
        Logger::debug(&format!(
            "Buffer {} ({}): Pushing initialization segment",
            self.id, self.typ
        ));
        match jsAppendBuffer(self.id, segment_data.id(), false).result() {
            Err(err) => Err(PushSegmentError::from_js_append_buffer_error(
                self.media_type,
                err,
            )),
            Ok(x) => Ok(AppendBufferResponse { parsed: x }),
        }
    }

    /// Pushes a new media segment to the SourceBuffer.
    ///
    /// If the `parse_time_info` bool in argument is set to `true`, the segment might be parsed to
    /// recuperate its time information which will be returned if found.
    ///
    /// # Arguments
    ///
    /// * `data` - Actual data AND metadata on the segment you want to push. See
    /// `MediaSegmentPushData` documentation for more information.
    ///
    /// * `parse_time_info` - If set to `true`, the segment's data will be read before pushing it
    ///   to try recuperate its timing information. If it has been parsed with success, it will
    ///   be contained in the `AppendBufferResponse` returned by this method.
    pub(super) fn push_media_segment(
        &mut self,
        data: MediaSegmentPushData,
        parse_time_info: bool,
    ) -> Result<AppendBufferResponse, PushSegmentError> {
        self.last_segment_pushed = false;
        self.was_used = true;
        let segment_data = data.segment_data.id();
        let id = data.id;
        self.queue
            .push_back(SourceBufferQueueElement::PushMedia((data, id)));
        Logger::debug(&format!("Buffer {} ({}): Pushing", self.id, self.typ));
        match jsAppendBuffer(self.id, segment_data, parse_time_info).result() {
            Err(err) => Err(PushSegmentError::from_js_append_buffer_error(
                self.media_type,
                err,
            )),
            Ok(x) => Ok(AppendBufferResponse { parsed: x }),
        }
    }

    /// Remove media data from this `SourceBuffer`, based js_on a `start` and `end` time in seconds.
    ///
    /// # Arguments
    ///
    /// * `start` - Start time, in seconds, of the range of time which should be removed from the
    /// `SourceBuffer`.
    ///
    /// * `end` - End time, in seconds, of the range of time which should be removed from the
    /// `SourceBuffer`.
    pub(super) fn remove_buffer(&mut self, start: f64, end: f64) {
        self.was_used = true;
        self.queue
            .push_back(SourceBufferQueueElement::Remove { start, end });
        Logger::debug(&format!(
            "Buffer {} ({}): Removing {} {}",
            self.id, self.typ, start, end
        ));
        jsRemoveBuffer(self.id, start, end);
    }

    /// Empty media data from this `SourceBuffer`.
    ///
    /// There's special considerations too take care of here as we'll remove data corresponding to
    /// the current position. As such a seek will have to be performed once the remove is done
    pub(super) fn flush_buffer(&mut self) {
        self.was_used = true;
        self.queue.push_back(SourceBufferQueueElement::Emptying);
        Logger::debug(&format!("Buffer {} ({}): emptying", self.id, self.typ));
        jsRemoveBuffer(self.id, 0., f64::INFINITY);
    }

    /// SourceBuffers maintain a queue of planned operations such as push and remove to media
    /// buffers.
    ///
    /// In some rare scenarios, we could be left in a situation where all previously scheduled
    /// operations are cancelled, such as when one of them fails.
    /// This method allows to empty that SourceBuffer's queue in such situations.
    pub(super) fn clear_queue(&mut self) {
        Logger::info(&format!(
            "Buffer {} ({}): clearing queue.",
            self.id, self.typ
        ));
        self.queue.clear();
    }

    /// Indicate to this `SourceBuffer` that the last chronological segment has been pushed.
    pub(super) fn announce_last_segment_pushed(&mut self) {
        self.last_segment_pushed = true;
    }

    /// Returns `true` if the last chronological segment is known to have been pushed.
    pub(super) fn is_last_segment_pushed(&self) -> bool {
        self.last_segment_pushed
    }

    /// To call once a `SourceBuffer` operation, either created through `append_buffer`,
    /// `remove_buffer` or `flush_buffer` has been finished by the underlying MSE SourceBuffer.
    pub(super) fn on_operation_end(&mut self) -> Option<SourceBufferQueueElement> {
        let queue_elt = self.queue.pop_front();
        match queue_elt {
            Some(SourceBufferQueueElement::Emptying) => {
                self.needs_reflush = true;
                jsFlush();
            }
            Some(SourceBufferQueueElement::PushMedia { .. }) => {
                if self.needs_reflush {
                    self.needs_reflush = false;
                    jsFlush();
                }
            }
            _ => {}
        }
        queue_elt
    }
}

/// Structure describing a media segment that should be pushed to the SourceBuffer.
pub(crate) struct MediaSegmentPushData {
    /// Identifier used to identify the pushed segment in question.
    ///
    /// It can be useful for example to easily detect which segment has succesfully been pushed or
    /// caused an issue.
    id: u64,

    /// Raw data of the segment to push.
    segment_data: JsMemoryBlob,

    /// Time information, as a tuple of its start time and end time in seconds as deduced from the
    /// media playlist.
    time_info: SegmentTimeInfo,
}

impl MediaSegmentPushData {
    /// Creates a new `SegmentPushData` object linked to the given data and time information.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier for the `MediaSegmentPushData`
    ///
    /// * `segment_data` - The segment's actual data.
    ///
    /// * `time_info` - The playlist-originated time information on that segment.
    pub(super) fn new(id: u64, segment_data: JsMemoryBlob, time_info: SegmentTimeInfo) -> Self {
        Self {
            id,
            segment_data,
            time_info,
        }
    }

    /// Get time information linked to this segment as a reference to its `SegmentTimeInfo` object.
    pub(crate) fn time_info(&self) -> &SegmentTimeInfo {
        &self.time_info
    }

    /// Returns start, in seconds, at which the segment starts.
    pub(crate) fn start(&self) -> f64 {
        self.time_info.start()
    }

    /// Returns end, in seconds, at which the segment ends.
    pub(crate) fn end(&self) -> f64 {
        self.time_info.end()
    }
}

/// Represents a successful response from the `append_buffer` SourceBuffer's method.
pub(crate) struct AppendBufferResponse {
    /// Time information optionally parsed from the segment itself.
    parsed: Option<ParsedSegmentInfo>,
}

impl AppendBufferResponse {
    /// Returns the optionally parsed start time, in seconds, found when parsing the segment's
    /// internals.
    ///
    /// If set it is generally closer to the real segment's start time, once pushed to the browser,
    /// than what the Media Playlist told us.
    pub(crate) fn media_start(&self) -> Option<f64> {
        self.parsed.as_ref().and_then(|p| p.start)
    }

    /// Returns the optionally parsed duration, in seconds, found when parsing the segment's
    /// internals.
    ///
    /// If set it is generally closer to the real segment's duration, once pushed to the browser,
    /// than what the Media Playlist told us.
    pub(crate) fn media_duration(&self) -> Option<f64> {
        self.parsed.as_ref().and_then(|p| p.duration)
    }
}

/// Enum listing possible operations awaiting to be performed on a `SourceBuffer`.
pub(crate) enum SourceBufferQueueElement {
    /// A new initialization segment needs to be pushed.
    PushInit(ResourceId),

    /// A new chunk of media data needs to be pushed.
    /// The `u64` is the corresponding `id` of the given `MediaSegmentPushData` when the
    /// `push_media_segment` method was called.
    PushMedia((MediaSegmentPushData, u64)),

    /// Some already-buffered needs to be removed, `start` and `end` giving the
    /// time range of the data to remove, in seconds.
    Remove { start: f64, end: f64 },

    /// The buffer is being completely emptied.
    Emptying,
}

use thiserror::Error;

/// Formatted error when the creation of a MSE SourceBuffer fails.
///
/// This is almost a 1:1 to error codes returned by `AddSourceBufferErrorCode`.
///
/// Note that `thiserror` is not used here because this error will really be formatted at a later
/// time.
#[derive(Debug)]
pub(super) enum AddSourceBufferError {
    /// No MediaSource was found to attach that `SourceBuffer`.
    NoMediaSourceAttached { message: String },

    /// The current `MediaSource` instance is in a "closed" state.
    /// As such it is not possible to attach a `SourceBuffer` to it anymore.
    MediaSourceIsClosed,

    /// A `QuotaExceededError` was received while trying to add the `SourceBuffer`
    ///
    /// Such errors are often encountered when another SourceBuffer attached to the same
    /// MediaSource instance was already updated through a buffer operation.
    QuotaExceededError { message: String },

    /// The given mime-type is not supported
    TypeNotSupportedError { mime_type: String, message: String },

    /// The given mime-type was an empty string
    EmptyMimeType,

    /// An unknown error happened.
    UnknownError { message: String },
}

impl AddSourceBufferError {
    /// Translate `SegmentParsingErrorCode` and its optional accompanying message, as returned by the
    /// `jsAppendBuffer` JavaScript function, into the corresponding `AddSourceBufferError`.
    ///
    /// # Arguments
    ///
    /// * `err` - The error received from the `jsAppendBuffer` JavaScript function.
    ///
    /// * `mime_type` - Mime-type linked to this SourceBuffer.
    pub(super) fn from_js_add_source_buffer_error(
        err: (AddSourceBufferErrorCode, Option<String>),
        mime_type: &str,
    ) -> Self {
        match err.0 {
            AddSourceBufferErrorCode::NoMediaSourceAttached => {
                AddSourceBufferError::NoMediaSourceAttached {
                    message: err
                        .1
                        .unwrap_or_else(|| "MediaSource instance not found.".to_owned()),
                }
            }
            AddSourceBufferErrorCode::MediaSourceIsClosed => {
                AddSourceBufferError::MediaSourceIsClosed
            }
            AddSourceBufferErrorCode::QuotaExceededError => {
                AddSourceBufferError::QuotaExceededError {
                    message: err
                        .1
                        .unwrap_or_else(|| "Unknown QuotaExceededError error".to_owned()),
                }
            }
            AddSourceBufferErrorCode::TypeNotSupportedError => {
                AddSourceBufferError::TypeNotSupportedError {
                    mime_type: mime_type.to_string(),
                    message: err
                        .1
                        .unwrap_or_else(|| "Unknown NotSupportedError error".to_owned()),
                }
            }
            AddSourceBufferErrorCode::EmptyMimeType => AddSourceBufferError::EmptyMimeType,
            AddSourceBufferErrorCode::UnknownError => AddSourceBufferError::UnknownError {
                message: err.1.unwrap_or_else(|| "Unknown error.".to_owned()),
            },
        }
    }
}

/// Error encountered synchronously after trying to push a segment to a `SourceBuffer`.
#[derive(Error, Debug)]
pub(crate) enum PushSegmentError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
    #[error("The {0} resource appended did not exist.")]
    NoResource(MediaType),
    #[error("Could not transmux {0} resource: {1}.")]
    TransmuxerError(MediaType, String),
    #[error("Uncategorized Error with {0} buffer: {1}")]
    UnknownError(MediaType, String),
}

impl PushSegmentError {
    /// Returns the `MediaType` associated to the `PushSegmentError`.
    pub(crate) fn media_type(&self) -> MediaType {
        match self {
            PushSegmentError::NoResource(m) => *m,
            PushSegmentError::NoSourceBuffer(m) => *m,
            PushSegmentError::TransmuxerError(m, _) => *m,
            PushSegmentError::UnknownError(m, _) => *m,
        }
    }

    /// Creates a new `PushSegmentError` based on a given `MediaType` and the original error as
    /// returned by the `jsAppendBuffer` binding.
    ///
    /// # Arguments
    ///
    /// * `media_type` - The `MediaType` linked to the corresponding `SourceBuffer`.
    ///
    /// * `err` - The error received from the `jsAppendBuffer` JavaScript function.
    fn from_js_append_buffer_error(
        media_type: MediaType,
        err: (SegmentParsingErrorCode, Option<String>),
    ) -> Self {
        match err.0 {
            SegmentParsingErrorCode::NoSourceBuffer => PushSegmentError::NoSourceBuffer(media_type),
            SegmentParsingErrorCode::NoResource => PushSegmentError::NoResource(media_type),
            SegmentParsingErrorCode::TransmuxerError => PushSegmentError::TransmuxerError(
                media_type,
                err.1
                    .unwrap_or_else(|| "Unknown transmuxing error.".to_owned()),
            ),
            SegmentParsingErrorCode::UnknownError => PushSegmentError::UnknownError(
                media_type,
                err.1.unwrap_or_else(|| "Unknown error.".to_owned()),
            ),
        }
    }
}

/// Error encountered synchronously after trying to remove media data from a `SourceBuffer`.
#[derive(Error, Debug)]
pub(crate) enum RemoveDataError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
}
