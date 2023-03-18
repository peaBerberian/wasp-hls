use crate::bindings::{
    jsAttachMediaSource, jsEndOfStream, jsRemoveMediaSource, jsSeek, jsSetMediaOffset,
    jsSetPlaybackRate, jsStartRebuffering, jsStopRebuffering, AttachMediaSourceErrorCode, JsResult,
    MediaType, SourceBufferId,
};
use crate::dispatcher::{MediaSourceReadyState, MediaObservation};
use crate::Logger;

// mod segment_inventory;
mod source_buffers;

use source_buffers::{PushSegmentError, RemoveDataError};

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

    is_rebuffering: bool,

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

    /// The wanted playback rate:
    /// 1. == playback at "normal" speed
    /// 2. == playback at two times the normal speed
    /// -1. == playback in reverse direction but at normal speed
    /// and so on
    wanted_speed: f64,
}

impl MediaElementReference {
    /// Create a new `MediaElementReference`.
    ///
    /// This has no effect on playback, you may then call `attach_media_source` to being
    /// attaching a MediaSource to the corresponding `HTMLMediaElement` or `reset` to remove
    /// a `MediaSource` already-attached to it.
    pub(crate) fn new() -> Self {
        Self {
            awaiting_seek: None,
            is_rebuffering: false,
            last_observation: None,
            media_source_ready_state: None,
            media_offset: None,
            video_buffer: None,
            audio_buffer: None,
            wanted_speed: 1.,
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

    /// Returns the wanted playback rate
    pub(crate) fn wanted_speed(&self) -> f64 {
        self.wanted_speed
    }

    /// Updates the wanted playback rate
    /// Note that playback effects will only happen asynchronously
    pub(crate) fn update_wanted_speed(&mut self, new_speed: f64) {
        self.wanted_speed = new_speed;
        jsSetPlaybackRate(new_speed);
    }

    /// Attach a new `MediaSource` to the media element linked to this `MediaElementReference`.
    ///
    /// This is a necessary step before creating media buffers on it.
    pub(crate) fn attach_media_source(&mut self) -> Result<(), AttachMediaSourceError> {
        self.reset();
        Ok(jsAttachMediaSource().result()?)
    }

    /// Returns the currently wanted playlist position.
    ///
    /// That is:
    ///
    ///   - If a seek has been asked for but could not be performed yet (for example,
    ///     because initialization is still pending), the position for that seek
    ///
    ///   - Else if no seek is pending, the last known media playhead position
    ///     converted to a playlist position.
    pub(crate) fn wanted_position(&self) -> f64 {
        match self.awaiting_seek {
            Some(awaiting_seek) => awaiting_seek,
            None => {
                let last_media_pos = self
                    .last_observation
                    .as_ref()
                    .map(|o| o.current_time())
                    .unwrap_or(0.);
                self.media_pos_to_playlist_pos(last_media_pos)
                    .unwrap_or(last_media_pos)
            }
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
            Some(obs) if obs.ready_state() >= 1 => match self.playlist_pos_to_media_pos(position) {
                Some(media_pos) => {
                    self.awaiting_seek = None;
                    jsSeek(media_pos);
                    true
                }
                None => {
                    self.awaiting_seek = Some(position);
                    false
                }
            },
            _ => {
                self.awaiting_seek = Some(position);
                false
            }
        }
    }

    /// Returns the last communicated `readyState` of the `MediaSource` attached
    /// to this `MediaElementReference`.
    ///
    /// This `readyState` is linked to the last "attached" (through the
    /// `attach_media_source` method) `MediaSource`.
    /// The return value should be equal to `None` when no `MediaSource`
    /// is currently attached.
    ///
    /// Note that you can (and should) communicate about new `readyState` by calling
    /// `update_media_source_ready_state` first.
    pub(crate) fn media_source_ready_state(&self) -> Option<MediaSourceReadyState> {
        self.media_source_ready_state
    }

    /// Create a new `SourceBuffer` instance linked to this
    /// `MediaElementReference`.
    ///
    /// A `MediaSource` first need to be attached for a `SourceBuffer` to be
    /// created (see `attach_media_source` method).
    pub(crate) fn create_source_buffer(
        &mut self,
        media_type: MediaType,
        mime_type: &str,
        codec: &str,
    ) -> Result<(), SourceBufferCreationError> {
        match self.media_source_ready_state {
            Some(MediaSourceReadyState::Closed) => {
                return Err(SourceBufferCreationError::MediaSourceIsClosed);
            }
            None => {
                return Err(SourceBufferCreationError::NoMediaSourceAttached {
                    message: "The MediaSource does not seem to be attached".to_string(),
                });
            }
            _ => {}
        }
        let sb_codec = format!("{};codecs=\"{}\"", mime_type, codec);
        match media_type {
            MediaType::Audio => {
                if self.audio_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType { media_type })
                } else {
                    self.audio_buffer =
                        Some(source_buffers::SourceBuffer::new(media_type, sb_codec)?);
                    Ok(())
                }
            }
            MediaType::Video => {
                if self.video_buffer.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType { media_type })
                } else {
                    self.video_buffer =
                        Some(source_buffers::SourceBuffer::new(media_type, sb_codec)?);
                    Ok(())
                }
            }
        }
    }

    /// Push a segment to the SourceBuffer of the media type given.
    ///
    /// You should have created a SourceBuffer of the corresponding type with
    /// `create_source_buffer` before calling this method. If you did not this method will return a
    /// `NoSourceBuffer` error.
    pub(crate) fn push_segment(
        &mut self,
        media_type: MediaType,
        metadata: PushMetadata,
    ) -> Result<(), PushSegmentError> {
        let has_media_offset = self.media_offset.is_some();
        match self.get_buffer_mut(media_type) {
            None => Err(PushSegmentError::NoSourceBuffer(media_type)),
            Some(sb) => {
                let do_time_parsing = !has_media_offset
                    && (media_type == MediaType::Audio || media_type == MediaType::Video)
                    && metadata.time_info.is_some();
                let start = metadata.time_info.map(|t| t.0);
                let parsed = sb.append_buffer(metadata, do_time_parsing)?;
                let media_start = parsed.and_then(|p| p.start);
                if let (Some(segment_start), Some(media_start)) = (start, media_start) {
                    let media_offset = media_start - segment_start;
                    Logger::info(&format!(
                        "Setting media offset: {}",
                        media_start - segment_start
                    ));
                    self.media_offset = Some(media_offset);
                    jsSetMediaOffset(media_offset);
                    self.check_awaiting_seek();
                }
                Ok(())
            }
        }
    }

    /// Remove media data, based on a `start` and `end` time in seconds.
    ///
    /// You should have created a SourceBuffer of the corresponding type with
    /// `create_source_buffer` before calling this method. If you did not this method will return a
    /// `NoSourceBuffer` error.
    pub(crate) fn remove_data(
        &mut self,
        media_type: MediaType,
        start: f64,
        end: f64,
    ) -> Result<(), RemoveDataError> {
        match self.get_buffer_mut(media_type) {
            None => Err(RemoveDataError::NoSourceBuffer(media_type)),
            Some(sb) => {
                sb.remove_buffer(start, end);
                Ok(())
            }
        }
    }

    pub(crate) fn flush(&mut self, media_type: MediaType) -> Result<(), RemoveDataError> {
        match self.get_buffer_mut(media_type) {
            None => Err(RemoveDataError::NoSourceBuffer(media_type)),
            Some(sb) => {
                sb.flush_buffer();
                Ok(())
            }
        }
    }

    /// Method to call once a `MediaObservation` has been received.
    pub(crate) fn on_observation(&mut self, observation: MediaObservation) {
        self.last_observation = Some(observation);
        if !self.check_awaiting_seek() {
            let last_observation = self.last_observation.as_ref().unwrap();
            let buffer_gap = get_buffer_gap(last_observation);
            if !self.is_rebuffering {
                if !last_observation.ended() {
                    match buffer_gap {
                        None => {
                            Logger::info("Starting rebuffering period due to no buffer gap");
                            self.is_rebuffering = true;
                            jsStartRebuffering();
                        }
                        Some(buffer_gap) if buffer_gap < 0.5 => {
                            let current_time = last_observation.current_time();
                            let duration = last_observation.duration();
                            if current_time + buffer_gap < duration - 0.001 {
                                Logger::info(&format!(
                                    "Starting rebuffering period. bg: {}",
                                    buffer_gap
                                ));
                                self.is_rebuffering = true;
                                jsStartRebuffering();
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                let mut quit_rebuffering = false;
                if let Some(buffer_gap) = buffer_gap {
                    if buffer_gap > 2. {
                        Logger::info(&format!("Quitting rebuffering period. bg: {}", buffer_gap));
                        quit_rebuffering = true;
                    } else {
                        let current_time = last_observation.current_time();
                        let duration = last_observation.duration();
                        if current_time + buffer_gap >= duration - 0.001 {
                            quit_rebuffering = true;
                        }
                    }
                }

                if quit_rebuffering || last_observation.ended() {
                    self.is_rebuffering = false;
                    jsStopRebuffering();
                }
            }
        }
    }

    /// Update the current `readyState` of the `MediaSource`.
    /// TODO trigger MediaObservation when MediaSourceReadyState' changes?
    pub(crate) fn update_media_source_ready_state(&mut self, ready_state: MediaSourceReadyState) {
        self.media_source_ready_state = Some(ready_state);
        self.check_end_of_stream();
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

    /// Announce that the last chronological segment has been pushed to the buffer of a
    /// given `media_type`.
    ///
    /// Calling this method for the media_type of each created SourceBuffer allows to properly end
    /// the stream once those last segments are reached.
    /// Pushing further segments for that `media_type` is still possible after calling `end_buffer`
    /// in which case, `end_buffer` should be re-called once, the new last chronological segment
    /// has been pushed.
    pub(crate) fn end_buffer(&mut self, media_type: MediaType) {
        match self.get_buffer_mut(media_type) {
            None => Logger::warn(&format!(
                "Asked to end a non existent {} buffer",
                media_type
            )),
            Some(sb) => {
                sb.announce_last_segment_pushed();
            }
        }
        self.check_end_of_stream();
    }

    /// Get reference to SourceBuffer attached to this `MediaElementReference` for this
    /// `media_type`.
    ///
    /// `None` if no SourceBuffer has been created for this `MediaType`
    fn get_buffer(&self, media_type: MediaType) -> Option<&source_buffers::SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio_buffer.as_ref(),
            MediaType::Video => self.video_buffer.as_ref(),
        }
    }

    /// Get mutable reference to SourceBuffer attached to this `MediaElementReference` for this
    /// `media_type`.
    ///
    /// `None` if no SourceBuffer has been created for this `MediaType`
    fn get_buffer_mut(
        &mut self,
        media_type: MediaType,
    ) -> Option<&mut source_buffers::SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio_buffer.as_mut(),
            MediaType::Video => self.video_buffer.as_mut(),
        }
    }

    /// Perform checks that all conditions for calling the `endOfStream` MSE API have been reached
    /// and call `jsEndOfStream` if that's the case.
    ///
    /// To call when any of its condition might have changed.
    fn check_end_of_stream(&self) {
        if self.video_buffer.is_none() && self.audio_buffer.is_none() {
            return;
        }
        if self.is_buffer_ended(MediaType::Audio)
            && self.is_buffer_ended(MediaType::Video)
            && self.media_source_ready_state != Some(MediaSourceReadyState::Closed)
        {
            jsEndOfStream();
        }
    }

    /// Returns `true` if the `SourceBuffer` of the corresponding `media_type` has ended, that is:
    ///   - its last chronological segment has been pushed.
    ///   - it has no operation left to perform.
    fn is_buffer_ended(&self, media_type: MediaType) -> bool {
        match self.get_buffer(media_type) {
            None => true,
            Some(sb) => sb.is_last_segment_pushed() && !sb.operations_pending(),
        }
    }

    /// Check if a scheduled seek is awaiting and if all condition to perform it are reached.
    /// If both are true, perform the seek.
    ///
    /// To call when any of its condition might have changed.
    ///
    /// Returns `true` if a seek has been performed
    fn check_awaiting_seek(&mut self) -> bool {
        if self.awaiting_seek.is_some()
            && self.last_observation.as_ref().unwrap().ready_state() >= 1
        {
            let awaiting_seek = self.awaiting_seek.unwrap();
            if let Some(media_pos) = self.playlist_pos_to_media_pos(awaiting_seek) {
                Logger::info(&format!(
                    "Perform awaited seek to {} ({})",
                    awaiting_seek, media_pos
                ));
                jsSeek(media_pos);
                self.awaiting_seek = None;
                return true;
            }
        }
        false
    }

    /// Convert a media position, which is the position as played on the
    /// media element, to a playlist position, which is the position actually
    /// used in this player.
    ///
    /// None if the `MediaElementReference` has not enough information yet to
    /// make that conversion.
    fn media_pos_to_playlist_pos(&self, pos: f64) -> Option<f64> {
        Some(pos - self.media_offset?)
    }

    /// Convert a playlist position, which is the position used in this player,
    /// to a media position, which is the position as played on the media
    /// element.
    ///
    /// None if the `MediaElementReference` has not enough information yet to
    /// make that conversion.
    fn playlist_pos_to_media_pos(&self, pos: f64) -> Option<f64> {
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
            AddSourceBufferError::NoMediaSourceAttached { message } => {
                SourceBufferCreationError::NoMediaSourceAttached { message }
            }
            AddSourceBufferError::MediaSourceIsClosed => {
                SourceBufferCreationError::MediaSourceIsClosed
            }
            AddSourceBufferError::QuotaExceededError { message } => {
                SourceBufferCreationError::QuotaExceededError { message }
            }
            AddSourceBufferError::TypeNotSupportedError { mime_type, message } => {
                SourceBufferCreationError::CantPlayType { mime_type, message }
            }
            AddSourceBufferError::EmptyMimeType => SourceBufferCreationError::EmptyMimeType,
            AddSourceBufferError::UnknownError { message } => {
                SourceBufferCreationError::UnknownError { message }
            }
        }
    }
}

#[derive(Error, Debug)]
pub(crate) enum AttachMediaSourceError {
    #[error("Error when attaching MediaSource: No content is currently loaded.")]
    NoContentLoaded,
    #[error("Uncategorized Error when attaching MediaSource: {message}")]
    UnknownError { message: String },
}

impl From<(AttachMediaSourceErrorCode, Option<String>)> for AttachMediaSourceError {
    fn from(x: (AttachMediaSourceErrorCode, Option<String>)) -> Self {
        match x.0 {
            AttachMediaSourceErrorCode::NoContentLoaded => AttachMediaSourceError::NoContentLoaded,
            AttachMediaSourceErrorCode::UnknownError => AttachMediaSourceError::UnknownError {
                message: x.1.unwrap_or_else(|| "Unknown Error.".to_string()),
            },
        }
    }
}

fn get_buffer_gap(observation: &MediaObservation) -> Option<f64> {
    let current_time = observation.current_time();
    let current_buffered = observation
        .buffered()
        .iter()
        .find(|b| current_time >= b.0 && current_time < b.1);
    Some(current_buffered?.1 - current_time)
}
