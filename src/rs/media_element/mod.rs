use crate::bindings::{
    jsAddSourceBuffer,
    jsAppendBuffer,
    jsRemoveBuffer,
    jsSeek,
    MediaType,
    jsAttachMediaSource,
    jsEndOfStream,
    MediaObservation,
    JsMemoryBlob,
    AppendBufferErrorCode,
    JsResult, jsRemoveMediaSource, ParsedSegmentInfo,
};
use crate::dispatcher::MediaSourceReadyState;
use crate::Logger;

/// Structure linked to an HTMLMediaElement which allows to perform media-related actions on it,
/// such as:
///   - attaching a MediaSource and creating SourceBuffers
///   - adding or removing data from those SourceBuffers
///   - pausing playback and resuming it
///   - seeking
///   - etc.
pub(crate) struct MediaElementReference {
    /// Set when a seek operation awaits (e.g. when a seek has been asked but the HTMLMediaElement
    /// is not yet ready to perform it).
    awaiting_seek: Option<f64>,

    /// Stores the last `MediaObservation` received.
    last_observation: Option<MediaObservation>,

    /// Offset used to convert the media position on the HTMLMediaElement (ultimately linked to
    /// pushed segments and the browser's internal logic) to the playlist position as found in a
    /// MultiVariant Playlist, that the WaspHlsPlayer actually uses.
    ///
    /// This offset is only known once a segment is being pushed.
    media_offset: Option<f64>,

    /// Current state of the attached MediaSource.
    ///
    /// `None` if no MediaSource is attached for now.
    media_source_ready_state: Option<MediaSourceReadyState>,

    /// Video SourceBuffer currently created for video data.
    /// `None` if no SourceBuffer has been created for that type.
    video_buffer: Option<SourceBuffer>,

    /// Audio SourceBuffer currently created for audio data.
    /// `None` if no SourceBuffer has been created for that type.
    audio_buffer: Option<SourceBuffer>,
}

impl MediaElementReference {
    /// Create a new `MediaElementReference`.
    /// This has no effect on playback, you may then call `attach_media_source` to being
    /// attaching a MediaSource to the corresponding `HTMLMediaElement` or `reset` to remove
    /// a `MediaSource` already-attached to it.
    pub fn new() -> Self {
        Self {
            awaiting_seek: None,
            last_observation: None,
            media_source_ready_state: None,
            media_offset: None,
            video_buffer: None,
            audio_buffer: None,
        }
    }

    /// Dispose current MediaSource if one and completely reset this MediaElementReference
    /// instance to its initial default state.
    ///
    /// To call once you want to stop the content.
    pub fn reset(&mut self) {
        jsRemoveMediaSource();
        self.awaiting_seek = None;
        self.last_observation = None;
        self.media_source_ready_state = Some(MediaSourceReadyState::Closed);
        self.media_offset = None;
        self.video_buffer = None;
        self.audio_buffer = None;
    }

    /// Attach a new `MediaSource` to the media element linked to this `MediaElementReference`.
    ///
    /// This is a necessary step before creating media buffers on it.
    pub fn attach_media_source(&mut self) {
        self.reset();
        jsAttachMediaSource();
    }

    /// Returns the currently wanted playlist position.
    /// That is:
    ///   - If a seek has been asked for but could not be performed yet (for example,
    ///     because initialization is still pending), the position for that seek
    ///   - Else if no seek is pending, the last known media playhead position
    ///     converted to a playlist position.
    pub fn wanted_position(&self) -> f64 {
        match self.awaiting_seek {
            Some(awaiting_seek) => awaiting_seek,
            None => {
                let last_media_pos = self.last_observation
                    .as_ref()
                    .map(|o| o.current_time())
                    .unwrap_or(0.);
                self.from_media_position(last_media_pos)
                    .unwrap_or(last_media_pos)
            },
        }
    }

    /// Perform a seek, that is, move the current position to another one.
    ///
    /// Note that depending on that `MediaElementReference`'s state, seeks might
    /// not be able to be performed right now, and might in that case be postponed
    /// until the right conditions are reached.
    ///
    /// The boolean returned indicates if the seek was able to be performed
    /// synchronously.
    pub fn seek(&mut self, position: f64) -> bool {
        match &self.last_observation {
            Some(obs) if obs.ready_state() >= 1 => {
                match self.to_media_position(position) {
                    Some(media_pos) => {
                        self.awaiting_seek = None;
                        jsSeek(media_pos);
                        true
                    },
                    None => {
                        self.awaiting_seek = Some(position);
                        false
                    },
                }
            },
            _ => {
                self.awaiting_seek = Some(position);
                false
            },
        }
    }

    /// Method to call once a `MediaObservation` has been received.
    pub fn on_observation(&mut self, observation: MediaObservation) {
        self.last_observation = Some(observation);
self.check_awaiting_seek();
    }

    /// Returns the last communicated `readyState` of the `MediaSource` attached
    /// to this `MediaElementReference`.
    ///
    /// This `readyState` is linked to the last "attached" (through the
    /// `attach_new` method) `MediaSource`.
    /// The return value should be equal to `None` when no `MediaSource`
    /// is currently attached.
    ///
    /// Note that you can (and should) communicate about new `readyState` by calling
    /// `update_media_source_ready_state` first.
    pub(crate) fn media_source_ready_state(&self) -> Option<MediaSourceReadyState> {
        self.media_source_ready_state
    }

    /// Update the current `readyState` of the `MediaSource`.
    /// TODO trigger MediaObservation when MediaSourceReadyState' changes?
    pub(crate) fn update_media_source_ready_state(&mut self, ready_state: MediaSourceReadyState) {
        self.media_source_ready_state = Some(ready_state);
        self.check_end_of_stream();
    }

    /// Create a new `SourceBuffer` instance linked to this
    /// `MediaElementReference`.
    ///
    /// A `MediaSource` first need to be attached for a `SourceBuffer` to be
    /// created (see `attach_new` method).
    pub fn create_source_buffer(
        &mut self,
        media_type: MediaType,
        mime_type: &str,
        codec: &str
    ) -> Result<(), SourceBufferCreationError> {
        match self.media_source_ready_state {
            Some(MediaSourceReadyState::Closed) => {
                return Err(SourceBufferCreationError::MediaSourceIsClosed);
            },
            None => {
                return Err(SourceBufferCreationError::NoMediaSourceAttached);
            },
            _ => {},
        }
        let sb_codec = format!("{};codecs=\"{}\"", mime_type, codec).to_owned();
        match media_type {
            MediaType::Audio => {
                if self.audio_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType(media_type))
                } else {
                    self.audio_buffer = Some(SourceBuffer::new(media_type, sb_codec));
                    Ok(())
                }
            }
            MediaType::Video => {
                if self.video_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType(media_type))
                } else {
                    self.video_buffer = Some(SourceBuffer::new(media_type, sb_codec));
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn push_segment(
        &mut self,
        media_type: MediaType,
        metadata: PushMetadata
    ) -> Result<(), PushSegmentError> {
        let has_media_offset = self.media_offset.is_some();
        match self.get_buffer_mut(media_type) {
            None => Err(PushSegmentError::NoSourceBuffer(media_type)),
            Some(sb) => {
                let do_time_parsing = !has_media_offset &&
                    (media_type == MediaType::Audio ||
                     media_type == MediaType::Video) &&
                    metadata.time_info.is_some();
                let start = metadata.time_info.map(|t| t.0);
                let parsed = sb.append_buffer(metadata, do_time_parsing)?;
                let media_start = parsed.map_or(None, |p| p.start);
                if let (Some(segment_start), Some(media_start)) = (start, media_start) {
                    Logger::info(&format!("Setting media offset: {}", media_start - segment_start));
                    self.media_offset = Some(media_start - segment_start);
                    self.check_awaiting_seek();
                }
                Ok(())
            },
        }
    }

    pub(crate) fn remove_data(
        &mut self,
        media_type: MediaType,
        start: f64,
        end: f64
    ) -> Result<(), RemoveDataError> {
        match self.get_buffer_mut(media_type) {
            None => Err(RemoveDataError::NoSourceBuffer(media_type)),
            Some(sb) => {
                sb.remove_buffer(start, end);
                Ok(())
            },
        }
    }

    /// Callback that should be called once one of the `SourceBuffer` linked to this
    /// `MediaElementReference` has "updated" (meaning: one of its operation has ended).
    pub(crate) fn on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        if let Some(ref mut sb) = self.audio_buffer {
            if sb.id == source_buffer_id {
                sb.on_operation_end();
            }
        }
        if let Some(ref mut sb) = self.video_buffer {
            if sb.id == source_buffer_id {
                sb.on_operation_end();
            }
        }
        self.check_end_of_stream();
    }

    /// Returns `true` if a `SourceBuffer` of the given `MediaType` is currently
    /// linked to this `MediaElementReference`.
    pub(crate) fn has_buffer(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.audio_buffer.is_some(),
            MediaType::Video => self.video_buffer.is_some(),
        }
    }


    pub fn end_buffer(&mut self, media_type: MediaType) {
        match self.get_buffer_mut(media_type) {
            None => {
                Logger::warn(&format!("Asked to end a non existent {} buffer", media_type))
            },
            Some(sb) => {
                sb.last_segment_pushed = true;
            }
        }
        self.check_end_of_stream();
    }

    fn get_buffer(&self, media_type: MediaType) -> Option<&SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio_buffer.as_ref(),
            MediaType::Video => self.video_buffer.as_ref()
        }
    }

    fn get_buffer_mut(&mut self, media_type: MediaType) -> Option<&mut SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio_buffer.as_mut(),
            MediaType::Video => self.video_buffer.as_mut()
        }
    }

    fn check_end_of_stream(&self) {
        if self.video_buffer.is_none() && self.audio_buffer.is_none() {
            return;
        }
        if self.is_buffer_ended(MediaType::Audio) &&
            self.is_buffer_ended(MediaType::Video) &&
            self.media_source_ready_state != Some(MediaSourceReadyState::Closed)
        {
            jsEndOfStream();
        }
    }

    fn is_buffer_ended(&self, media_type: MediaType) -> bool {
        match self.get_buffer(media_type) {
            None => true,
            Some(sb) => sb.last_segment_pushed && !sb.has_remaining_operation,
        }
    }

    fn check_awaiting_seek(&mut self) {
        if self.awaiting_seek.is_some() && self.last_observation.as_ref().unwrap().ready_state() >= 1 {
            let awaiting_seek = self.awaiting_seek.unwrap();
            if let Some(media_pos) = self.to_media_position(awaiting_seek) {
                Logger::info(&format!("Perform awaited seek to {} ({})", awaiting_seek, media_pos));
                jsSeek(media_pos);
                self.awaiting_seek = None;
            }
        }
    }

    /// Convert a media position, which is the position as played on the
    /// media element, to a playlist position, which is the position actually
    /// used in this player.
    ///
    /// None if the `MediaElementReference` has not enough information yet to
    /// make that conversion.
    fn from_media_position(&self, pos: f64) -> Option<f64> {
        Some(pos - self.media_offset?)
    }

    /// Convert a playlist position, which is the position used in this player,
    /// to a media position, which is the position as played on the media
    /// element.
    ///
    /// None if the `MediaElementReference` has not enough information yet to
    /// make that conversion.
    fn to_media_position(&self, pos: f64) -> Option<f64> {
        Some(pos + self.media_offset?)
    }
}

/// Identify a unique JavaScript `SourceBuffer`
pub type SourceBufferId = f64;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PushSegmentError {
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
pub enum RemoveDataError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
}

#[derive(Error, Debug)]
pub enum SourceBufferCreationError {
    #[error("A SourceBuffer was already created for the {0} type.")]
    AlreadyCreatedWithSameType(MediaType),

    #[error("No JavaScript MediaSource is currently attached to the media element")]
    NoMediaSourceAttached,

    #[error("The `MediaSource` has a \"closed\" state.")]
    MediaSourceIsClosed,

    /// The `SourceBuffer` could not have been created because a `QuotaExceededError`
    /// JavaScript error has been received while doing so. This often happens when
    /// a SourceBuffer is created after another one already had data pushed to it.
    #[error("QuotaExceededError: {0}")]
    QuotaExceededError(String),

    #[error("The given `{0}` mime-type and codecs couple is not supported by the current device.")]
    CantPlayType(String),

    #[error("Unknown error: `{0}`")]
    UnknownError(String),
}

/// Abstraction over the Media Source Extension's `SourceBuffer` concept.
///
/// This is the interface allowing to interact with lower-level media buffers.
pub struct SourceBuffer {
    /// The `SourceBufferId` given on SourceBuffer creation, used to identify
    /// this `SourceBuffer` in the current dispatcher instance.
    id: SourceBufferId,

    /// The current queue of operations being performed on the `SourceBuffer`.
    queue: Vec<SourceBufferQueueElement>,

    /// The Content-Type currently linked to the SourceBuffer
    typ: String,

    /// If `true`, at least one operation is still pending on the underlying MSE's
    /// `SourceBuffer`.
    has_remaining_operation: bool,

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

/// Enum listing possible operations awaiting to be performed on a `SourceBuffer`.
pub enum SourceBufferQueueElement {
    /// A new chunk of media data needs to be pushed.
    Push(PushMetadata),

    /// Some already-buffered needs to be removed, `start` and `end` giving the
    /// time range of the data to remove, in seconds.
    Remove { start: f64, end: f64 },
}

impl SourceBuffer {
    fn new(media_type: MediaType, typ: String) -> Self {
        Logger::info(&format!("Creating new {} SourceBuffer", media_type));
        let x = jsAddSourceBuffer(media_type, &typ,);
        Self {
            id: x,
            typ,
            has_remaining_operation: false,
            queue: vec![],
            was_used: false,
            last_segment_pushed: false,
            media_type,
        }
    }

    pub fn get_segment_queue(&self) -> &[SourceBufferQueueElement] {
        &self.queue
    }

    pub fn append_buffer(
        &mut self,
        metadata: PushMetadata,
        parse_time_info: bool
    ) -> Result<Option<ParsedSegmentInfo>, PushSegmentError> {
        self.was_used = true;
        self.has_remaining_operation = true;
        let segment_id = metadata.segment_data.get_id();
        self.queue.push(SourceBufferQueueElement::Push(metadata));
        Logger::debug(&format!("Buffer {} ({}): Pushing", self.id, self.typ));
        match jsAppendBuffer(self.id, segment_id, parse_time_info).result() {
            Err(err) =>
                Err(PushSegmentError::from_append_buffer_error(self.media_type, err)),
            Ok(x) => Ok(x),
        }
    }

    pub fn remove_buffer(&mut self, start: f64, end: f64) {
        self.was_used = true;
        self.has_remaining_operation = true;
        self.queue.push(SourceBufferQueueElement::Remove { start, end });
        Logger::debug(&format!("Buffer {} ({}): Removing {} {}",
            self.id, self.typ, start, end));
        jsRemoveBuffer(self.id, start, end);
    }

    pub fn on_operation_end(&mut self) {
        self.queue.remove(0);
        if self.queue.is_empty() {
            self.has_remaining_operation = false;
        }
    }

//     pub fn buffered(&self) -> Vec<(f64, f64)> {
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

pub struct PushMetadata {
    segment_data: JsMemoryBlob,
    time_info: Option<(f64, f64)>
}

impl PushMetadata {
    pub fn new(
        segment_data: JsMemoryBlob,
        time_info: Option<(f64, f64)>
    ) -> Self {
        Self { segment_data, time_info }
    }
}
