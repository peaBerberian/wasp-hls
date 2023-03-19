use std::collections::VecDeque;

use crate::bindings::{
    jsAddSourceBuffer, jsAppendBuffer, jsFlush, jsRemoveBuffer, AddSourceBufferErrorCode,
    AppendBufferErrorCode, JsResult, MediaType, ParsedSegmentInfo, SourceBufferId,
};
use crate::dispatcher::JsMemoryBlob;
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

    next_segment_id: u64,
}

impl SourceBuffer {
    /// Create a new `SourceBuffer` for the given `MediaType` and the mime-type indicated by `typ`.
    pub(super) fn new(media_type: MediaType, typ: String) -> Result<Self, AddSourceBufferError> {
        Logger::info(&format!("Creating new {} SourceBuffer", media_type));
        match jsAddSourceBuffer(media_type, &typ).result() {
            Ok(x) => Ok(Self {
                next_segment_id: 0,
                id: x,
                typ,
                queue: VecDeque::new(),
                was_used: false,
                needs_reflush: false,
                last_segment_pushed: false,
                media_type,
            }),
            Err(err) => Err(AddSourceBufferError::from_add_source_buffer_error(
                err, &typ,
            )),
        }
    }

    /// Get the `SourceBufferId`, needed to refer to that SourceBuffer when interacting with
    /// JavaScript.
    pub(super) fn id(&self) -> SourceBufferId {
        self.id
    }

    /// Returns `true` if there is at least one pending buffer operation that
    /// isn't finished yet.
    pub(super) fn has_operations_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Push a new segment to the SourceBuffer.
    ///
    /// If the `parse_time_info` bool in argument is set to `true`, the segment might be parsed to
    /// recuperate its time information which will be returned if found.
    pub(super) fn append_buffer(
        &mut self,
        metadata: PushMetadata,
        parse_time_info: bool,
    ) -> Result<AppendBufferResponse, PushSegmentError> {
        self.last_segment_pushed = false;
        self.was_used = true;
        let segment_data = metadata.segment_data.id();
        let segment_id = self.next_segment_id;
        self.next_segment_id += 1;
        self.queue.push_back(SourceBufferQueueElement::Push((
            metadata,
            segment_id,
        )));
        Logger::debug(&format!("Buffer {} ({}): Pushing", self.id, self.typ));
        match jsAppendBuffer(self.id, segment_data, parse_time_info).result() {
            Err(err) => Err(PushSegmentError::from_append_buffer_error(
                self.media_type,
                err,
            )),
            Ok(x) => Ok(AppendBufferResponse {
                parsed: x,
                id: segment_id,
            }),
        }
    }

    /// Remove media data from this `SourceBuffer`, based on a `start` and `end` time in seconds.
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

    /// Indicate to this `SourceBuffer` that the last chronological segment has been pushed.
    pub(super) fn announce_last_segment_pushed(&mut self) {
        self.last_segment_pushed = true;
    }

    /// Returns `true` if the last chronological segment is known to have been pushed.
    pub(super) fn is_last_segment_pushed(&self) -> bool {
        self.last_segment_pushed
    }

    /// To call once a `SourceBuffer` operation, either created through `append_buffer` or
    /// `remove_buffer`, has been finished by the underlying MSE SourceBuffer.
    pub(super) fn on_operation_end(&mut self) -> Option<SourceBufferQueueElement> {
        let queue_elt = self.queue.pop_front();
        match queue_elt {
            Some(SourceBufferQueueElement::Emptying) => {
                self.needs_reflush = true;
                jsFlush();
            }
            Some(SourceBufferQueueElement::Push { .. }) => {
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

/// Structure describing a segment that should be pushed to the SourceBuffer.
pub(crate) struct PushMetadata {
    /// Raw data of the segment to push.
    segment_data: JsMemoryBlob,

    /// Time information, as a tuple of its start time and end time in seconds as deduced from the
    /// media playlist.
    ///
    /// This should always be defined, unless the segment contains no media data (like for
    /// initialization segments).
    time_info: Option<(f64, f64)>,
}

impl PushMetadata {
    /// Creates a new `PushMetadata`.
    pub(crate) fn new(segment_data: JsMemoryBlob, time_info: Option<(f64, f64)>) -> Self {
        Self {
            segment_data,
            time_info,
        }
    }

    pub(crate) fn time_info(&self) -> Option<&(f64, f64)> {
        self.time_info.as_ref()
    }

    pub(crate) fn start(&self) -> Option<f64> {
        self.time_info.map(|t| t.0)
    }

    pub(crate) fn end(&self) -> Option<f64> {
        self.time_info.map(|t| t.1)
    }
}

pub(crate) struct AppendBufferResponse {
    parsed: Option<ParsedSegmentInfo>,
    id: u64,
}

impl AppendBufferResponse {
    pub(crate) fn segment_id(&self) -> u64 {
        self.id
    }

    pub(crate) fn media_start(&self) -> Option<f64> {
        self.parsed.as_ref().and_then(|p| p.start)
    }

    pub(crate) fn media_duration(&self) -> Option<f64> {
        self.parsed.as_ref().and_then(|p| p.duration)
    }
}

/// Enum listing possible operations awaiting to be performed on a `SourceBuffer`.
pub(crate) enum SourceBufferQueueElement {
    /// A new chunk of media data needs to be pushed.
    Push((PushMetadata, u64)),

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
    NoMediaSourceAttached { message: String },
    MediaSourceIsClosed,
    QuotaExceededError { message: String },
    TypeNotSupportedError { mime_type: String, message: String },
    EmptyMimeType,
    UnknownError { message: String },
}

impl AddSourceBufferError {
    fn from_add_source_buffer_error(
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
    /// Creates a new `PushSegmentError` based on a given `MediaType` and the original error as
    /// returned by the `jsAppendBuffer` binding.
    fn from_append_buffer_error(
        media_type: MediaType,
        err: (AppendBufferErrorCode, Option<String>),
    ) -> Self {
        match err.0 {
            AppendBufferErrorCode::NoSourceBuffer => PushSegmentError::NoSourceBuffer(media_type),
            AppendBufferErrorCode::NoResource => PushSegmentError::NoResource(media_type),
            AppendBufferErrorCode::TransmuxerError => PushSegmentError::TransmuxerError(
                media_type,
                err.1
                    .unwrap_or_else(|| "Unknown transmuxing error.".to_owned()),
            ),
            AppendBufferErrorCode::UnknownError => PushSegmentError::UnknownError(
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
