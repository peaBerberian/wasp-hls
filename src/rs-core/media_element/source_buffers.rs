use crate::bindings::{
    jsAddSourceBuffer,
    jsAppendBuffer,
    jsRemoveBuffer,
    MediaType,
    JsMemoryBlob,
    AppendBufferErrorCode,
    JsResult,
    ParsedSegmentInfo,
    AddSourceBufferErrorCode,
};
use crate::Logger;

/// Identify a unique JavaScript `SourceBuffer`
pub(crate) type SourceBufferId = f64;

/// Abstraction over the Media Source Extension's `SourceBuffer` concept.
///
/// This is the interface allowing to interact with lower-level media buffers.
pub(super) struct SourceBuffer {
    /// The `SourceBufferId` given on SourceBuffer creation, used to identify
    /// this `SourceBuffer` in the current dispatcher instance.
    id: SourceBufferId,

    /// The current queue of operations being performed on the `SourceBuffer`.
    queue: Vec<SourceBufferQueueElement>,

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
}

impl SourceBuffer {
    pub(super) fn new(media_type: MediaType, typ: String) -> Result<Self, AddSourceBufferError> {
        Logger::info(&format!("Creating new {} SourceBuffer", media_type));
        match jsAddSourceBuffer(media_type, &typ,).result() {
            Ok(x) => {
                Ok(Self {
                    id: x,
                    typ,
                    queue: vec![],
                    was_used: false,
                    last_segment_pushed: false,
                    media_type,
                })
            },
            Err(err) => Err(AddSourceBufferError::from_add_source_buffer_error(err, &typ)),
        }
    }

    pub(super) fn id(&self) -> SourceBufferId {
        self.id
    }

    pub(super) fn get_segment_queue(&self) -> &[SourceBufferQueueElement] {
        &self.queue
    }

    pub(super) fn append_buffer(
        &mut self,
        metadata: PushMetadata,
        parse_time_info: bool
    ) -> Result<Option<ParsedSegmentInfo>, PushSegmentError> {
        self.was_used = true;
        let segment_id = metadata.segment_data.get_id();
        self.queue.push(SourceBufferQueueElement::Push(metadata));
        Logger::debug(&format!("Buffer {} ({}): Pushing", self.id, self.typ));
        match jsAppendBuffer(self.id, segment_id, parse_time_info).result() {
            Err(err) =>
                Err(PushSegmentError::from_append_buffer_error(self.media_type, err)),
            Ok(x) => Ok(x),
        }
    }

    pub(super) fn anounced_last_segment_pushed(&mut self) {
        self.last_segment_pushed = true;
    }

    pub(super) fn is_last_segment_pushed(&self) -> bool {
        self.last_segment_pushed
    }

    pub(super) fn remove_buffer(&mut self, start: f64, end: f64) {
        self.was_used = true;
        self.queue.push(SourceBufferQueueElement::Remove { start, end });
        Logger::debug(&format!("Buffer {} ({}): Removing {} {}",
            self.id, self.typ, start, end));
        jsRemoveBuffer(self.id, start, end);
    }

    pub(super) fn on_operation_end(&mut self) {
        self.queue.remove(0);
    }

//     pub(super) fn buffered(&self) -> Vec<(f64, f64)> {
//         let buffered = jsGetSourceBufferBuffered(self.id);
//         let og_len = buffered.len();
//         if og_len % 2 != 0 {
//             panic!("Unexpected buffered value: not even.");
//         }
//         let mut ret : Vec<(f64, f64)> = Vec::with_capacity(og_len / 2);
//         for i in 0..og_len / 2 {
//             ret.push((buffered[i], buffered[i+1]));
//         }
//         ret
//     }
}

pub(crate) struct PushMetadata {
    segment_data: JsMemoryBlob,
    pub(crate) time_info: Option<(f64, f64)>
}

impl PushMetadata {
    pub(crate) fn new(
        segment_data: JsMemoryBlob,
        time_info: Option<(f64, f64)>
    ) -> Self {
        Self { segment_data, time_info }
    }
}

/// Enum listing possible operations awaiting to be performed on a `SourceBuffer`.
pub(crate) enum SourceBufferQueueElement {
    /// A new chunk of media data needs to be pushed.
    Push(PushMetadata),

    /// Some already-buffered needs to be removed, `start` and `end` giving the
    /// time range of the data to remove, in seconds.
    Remove { start: f64, end: f64 },
}

use thiserror::Error;

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
        mime_type: &str
    ) -> Self {
        match err.0 {
            AddSourceBufferErrorCode::NoMediaSourceAttached =>
                AddSourceBufferError::NoMediaSourceAttached {
                    message: err.1.unwrap_or("MediaSource instance not found.".to_owned())
                },
            AddSourceBufferErrorCode::MediaSourceIsClosed =>
                AddSourceBufferError::MediaSourceIsClosed,
            AddSourceBufferErrorCode::QuotaExceededError =>
                AddSourceBufferError::QuotaExceededError {
                    message: err.1.unwrap_or("Unknown QuotaExceededError error".to_owned())
                },
            AddSourceBufferErrorCode::TypeNotSupportedError =>
                AddSourceBufferError::TypeNotSupportedError {
                    mime_type: mime_type.to_string(),
                    message: err.1.unwrap_or("Unknown NotSupportedError error".to_owned()),
                },
            AddSourceBufferErrorCode::EmptyMimeType => AddSourceBufferError::EmptyMimeType,
            AddSourceBufferErrorCode::UnknownError =>
                AddSourceBufferError::UnknownError {
                    message: err.1.unwrap_or("Unknown error.".to_owned())
                },
        }
    }
}

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
    fn from_append_buffer_error(
        media_type: MediaType,
        err: (AppendBufferErrorCode, Option<String>)
    ) -> Self {
        match err.0 {
            AppendBufferErrorCode::NoSourceBuffer =>
                PushSegmentError::NoSourceBuffer(media_type),
            AppendBufferErrorCode::NoResource =>
                PushSegmentError::NoResource(media_type),
            AppendBufferErrorCode::TransmuxerError =>
                PushSegmentError::TransmuxerError(
                    media_type,
                    err.1.unwrap_or("Unknown transmuxing error.".to_owned())
                ),
            AppendBufferErrorCode::UnknownError =>
                PushSegmentError::UnknownError(
                    media_type,
                    err.1.unwrap_or("Unknown error.".to_owned())
                ),
        }
    }
}

#[derive(Error, Debug)]
pub(crate) enum RemoveDataError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
}
