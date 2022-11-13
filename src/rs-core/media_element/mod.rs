use crate::bindings::{
    jsSeek,
    MediaType,
    jsAttachMediaSource,
    jsEndOfStream,
    MediaObservation,
    jsRemoveMediaSource,
    jsSetMediaOffset,
};
use crate::dispatcher::MediaSourceReadyState;
use crate::Logger;

mod source_buffers;

use source_buffers::{
    SourceBufferId,
    PushSegmentError,
    RemoveDataError,
};

pub(crate) use source_buffers::PushMetadata;

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
    video_buffer: Option<source_buffers::SourceBuffer>,

    /// Audio SourceBuffer currently created for audio data.
    /// `None` if no SourceBuffer has been created for that type.
    audio_buffer: Option<source_buffers::SourceBuffer>,
}

impl MediaElementReference {
    /// Create a new `MediaElementReference`.
    /// This has no effect on playback, you may then call `attach_media_source` to being
    /// attaching a MediaSource to the corresponding `HTMLMediaElement` or `reset` to remove
    /// a `MediaSource` already-attached to it.
    pub(crate) fn new() -> Self {
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
    pub(crate) fn reset(&mut self) {
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
    pub(crate) fn attach_media_source(&mut self) {
        self.reset();
        // TODO handle result
        jsAttachMediaSource();
    }

    /// Returns the currently wanted playlist position.
    /// That is:
    ///   - If a seek has been asked for but could not be performed yet (for example,
    ///     because initialization is still pending), the position for that seek
    ///   - Else if no seek is pending, the last known media playhead position
    ///     converted to a playlist position.
    pub(crate) fn wanted_position(&self) -> f64 {
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
    pub(crate) fn seek(&mut self, position: f64) -> bool {
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
    pub(crate) fn on_observation(&mut self, observation: MediaObservation) {
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
    pub(crate) fn create_source_buffer(
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
                return Err(
                    SourceBufferCreationError::NoMediaSourceAttached {
                        message: "The MediaSource does not seem to be attached".to_string()
                    }
                );
            },
            _ => {},
        }
        let sb_codec = format!("{};codecs=\"{}\"", mime_type, codec).to_owned();
        match media_type {
            MediaType::Audio => {
                if self.audio_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType { media_type })
                } else {
                    self.audio_buffer = Some(source_buffers::SourceBuffer::new(media_type, sb_codec)?);
                    Ok(())
                }
            }
            MediaType::Video => {
                if self.video_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType { media_type })
                } else {
                    self.video_buffer = Some(source_buffers::SourceBuffer::new(media_type, sb_codec)?);
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
                    let media_offset = media_start - segment_start;
                    Logger::info(&format!("Setting media offset: {}", media_start - segment_start));
                    self.media_offset = Some(media_offset);
                    jsSetMediaOffset(media_offset);
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
            if sb.id() == source_buffer_id {
                sb.on_operation_end();
            }
        }
        if let Some(ref mut sb) = self.video_buffer {
            if sb.id() == source_buffer_id {
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


    pub(crate) fn end_buffer(&mut self, media_type: MediaType) {
        match self.get_buffer_mut(media_type) {
            None => {
                Logger::warn(&format!("Asked to end a non existent {} buffer", media_type))
            },
            Some(sb) => {
                sb.anounced_last_segment_pushed();
            }
        }
        self.check_end_of_stream();
    }

    fn get_buffer(&self, media_type: MediaType) -> Option<&source_buffers::SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio_buffer.as_ref(),
            MediaType::Video => self.video_buffer.as_ref()
        }
    }

    fn get_buffer_mut(&mut self, media_type: MediaType) -> Option<&mut source_buffers::SourceBuffer> {
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
            Some(sb) => sb.is_last_segment_pushed() && sb.get_segment_queue().is_empty(),
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

use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum SourceBufferCreationError {
    #[error("SourceBuffer initialization impossible: {message}")]
    NoMediaSourceAttached { message: String },
    #[error("Could not create SourceBuffer because the MediaSource instance was closed.")]
    MediaSourceIsClosed,
    #[error("QuotaExceededError received when trying to create SourceBuffer: {message}")]
    QuotaExceededError { message: String },
    #[error("Could not create SourceBuffer due to unsupported `{mime_type}` mime-type: {message}")]
    CantPlayType { mime_type: String, message: String },
    #[error("Could not create SourceBuffer because no mime-type was defined.")]
    EmptyMimeType,
    #[error("A SourceBuffer was already created for the {media_type} type.")]
    AlreadyCreatedWithSameType { media_type: MediaType },
    #[error("Uncategorized Error when creating SourceBuffer: {message}")]
    UnknownError { message: String },
}

use source_buffers::AddSourceBufferError;

impl From<AddSourceBufferError> for SourceBufferCreationError {
  fn from(src: AddSourceBufferError) -> Self {
    match src {
        AddSourceBufferError::NoMediaSourceAttached { message } =>
            SourceBufferCreationError::NoMediaSourceAttached { message },
        AddSourceBufferError::MediaSourceIsClosed =>
            SourceBufferCreationError::MediaSourceIsClosed,
        AddSourceBufferError::QuotaExceededError { message } =>
            SourceBufferCreationError::QuotaExceededError { message },
        AddSourceBufferError::TypeNotSupportedError { mime_type, message } =>
            SourceBufferCreationError::CantPlayType { mime_type, message },
        AddSourceBufferError::EmptyMimeType =>
            SourceBufferCreationError::EmptyMimeType,
        AddSourceBufferError::UnknownError { message } =>
            SourceBufferCreationError::UnknownError { message },

    }
  }
}
