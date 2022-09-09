use crate::{bindings::{
    DataSource,
    jsAddSourceBuffer,
    jsAppendBuffer,
    jsAppendBufferJsBlob,
    jsRemoveBuffer,
    jsSeek,
    MediaType, jsAttachMediaSource, jsEndOfStream, MediaObservation,
}, dispatcher::MediaSourceReadyState, Logger};

pub(crate) struct MediaElementReference {
    initial_seek_performed: bool,
    incoming_seek: Option<f64>,
    last_observation: Option<MediaObservation>,
    buffers: Option<MediaBuffers>,
}

impl MediaElementReference {
    pub fn new() -> Self {
        Self {
            initial_seek_performed: false,
            incoming_seek: None,
            last_observation: None,
            buffers: None,
        }
    }

    pub fn initialize(&mut self) {
        self.initial_seek_performed = false;
        jsAttachMediaSource();
        self.buffers = Some(MediaBuffers {
            ready_state: MediaSourceReadyState::Closed,
            video: None,
            audio: None,
        });
    }

    pub fn buffers(&self) -> Option<&MediaBuffers> {
        self.buffers.as_ref()
    }

    pub fn buffers_mut(&mut self) -> Option<&mut MediaBuffers> {
        self.buffers.as_mut()
    }

    pub fn seek_once_ready(&mut self, position: f64) {
        if !self.initial_seek_performed {
            self.incoming_seek = Some(position);
        } else {
            if let Some(obs) = &self.last_observation {
                if obs.ready_state() < 1 {
                    self.incoming_seek = Some(position);
                    return ;
                }
            }
            jsSeek(position);
        }
    }

    pub fn on_observation(&mut self, observation: MediaObservation) {
        self.last_observation = Some(observation);
        if !self.initial_seek_performed &&
            self.last_observation.as_ref().unwrap().ready_state() >= 1
        {
            if let Some(pos) = self.incoming_seek {
                jsSeek(pos);
            }
            self.initial_seek_performed = true;
        }
    }

    pub fn reset(&mut self) {
        self.initial_seek_performed = false;
        self.last_observation = None;
        self.buffers = None;
    }
}

/// Structure keeping track of the MediaSource and its attached audio and video
/// SourceBuffers, making sure only one is created for each and that one is not
/// created once media data has been pushed to any of them.
pub(crate) struct MediaBuffers {
    /// Current state of the attached MediaSource.
    ///
    /// `None` if no MediaSource is attached for now.
    ready_state: MediaSourceReadyState,

    /// Video SourceBuffer currently created for video data.
    /// `None` if no SourceBuffer has been created for that type.
    video: Option<SourceBuffer>,

    /// Audio SourceBuffer currently created for audio data.
    /// `None` if no SourceBuffer has been created for that type.
    audio: Option<SourceBuffer>,
}

impl MediaBuffers {
    /// Returns the current `ready_state` of the `MediaBuffers`, which is
    /// binded to the `MediaSource`'s (from the Media Source specification)
    /// `readyState` concept.
    ///
    /// This `ready_state` is linked to the last "attached" (through the
    /// `attach_new` method) `MediaSource` and should be equal to
    /// `MediaSourceReadyState::Closed` when no `MediaSource` is currently
    /// attached.
    pub(crate) fn ready_state(&self) -> MediaSourceReadyState {
        self.ready_state
    }

    /// Update the current `ready_state` of the `MediaBuffers`.
    /// This should be called only to link it to the original `MediaSource`'s
    /// (from the Media Source specification) `readyState` concept.
    ///
    /// This `ready_state` is linked to the last "attached" (through the
    /// `attach_new` method) `MediaSource` and should be equal to
    /// `MediaSourceReadyState::Closed` when no `MediaSource` is currently
    /// attached.
    pub(crate) fn update_ready_state(&mut self, ready_state: MediaSourceReadyState) {
        self.ready_state = ready_state;
        self.check_end_of_stream();
    }

    /// Returns `true` if `SourceBuffer` can currently be created on this
    /// `MediaBuffers`.
    ///
    /// Returns `false` when this is too late for the `MediaSource` currently
    /// attached (see `attach_new` method).
    pub fn can_still_create_source_buffer(&self) -> bool {
        match (&self.audio, &self.video) {
            (None, None) => true,
            (Some(asb), Some(vsb)) => !asb.has_been_updated && !vsb.has_been_updated,
            (Some(asb), None) => !asb.has_been_updated,
            (None, Some(vsb)) => !vsb.has_been_updated,
        }
    }

    /// Create a new `SourceBuffer` instance linked to this
    /// `MediaBuffers`'s `MediaSource`.
    ///
    /// A `MediaSource` first need to be attached for a `SourceBuffer` to be
    /// created (see `attach_new` method).
    pub fn create_source_buffer(
        &mut self,
        media_type: MediaType,
        mime_type: &str,
        codec: &str
    ) -> Result<(), SourceBufferCreationError> {
        let sb_codec = format!("{};codecs=\"{}\"", mime_type, codec).to_owned();
        match media_type {
            MediaType::Audio => {
                if self.audio.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType(media_type))
                } else {
                    self.audio = Some(SourceBuffer::new(media_type, sb_codec));
                    Ok(())
                }
            }
            MediaType::Video => {
                if self.video.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType(media_type))
                } else {
                    self.video = Some(SourceBuffer::new(media_type, sb_codec));
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
        match self.get_mut(media_type) {
            None => Err(PushSegmentError::NoSourceBuffer(media_type)),
            Some(sb) => {
                sb.append_buffer(metadata);
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
        match self.get_mut(media_type) {
            None => Err(RemoveDataError::NoSourceBuffer(media_type)),
            Some(sb) => {
                sb.remove_buffer(start, end);
                Ok(())
            },
        }
    }

    /// Callback that should be called once one of the `SourceBuffer` linked to this
    /// `MediaBuffers` has "updated" (meaning: one of its operation has ended).
    pub(crate) fn on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        if let Some(ref mut sb) = self.audio {
            if sb.id == source_buffer_id {
                sb.on_update_end();
            }
        }
        if let Some(ref mut sb) = self.video {
            if sb.id == source_buffer_id {
                sb.on_update_end();
            }
        }
        self.check_end_of_stream();
    }

    /// Returns `true` if a `SourceBuffer` of the given `MediaType` is currently
    /// linked to this `MediaBuffers`.
    pub(crate) fn has(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.audio.is_some(),
            MediaType::Video => self.video.is_some(),
        }
    }


    pub fn end(&mut self, media_type: MediaType) {
        match self.get_mut(media_type) {
            None => {
                Logger::warn(&format!("Asked to end a non existent {} buffer", media_type))
            },
            Some(sb) => {
                sb.last_segment_pushed = true;
            }
        }
        self.check_end_of_stream();
    }

    fn get(&self, media_type: MediaType) -> Option<&SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio.as_ref(),
            MediaType::Video => self.video.as_ref()
        }
    }

    fn get_mut(&mut self, media_type: MediaType) -> Option<&mut SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio.as_mut(),
            MediaType::Video => self.video.as_mut()
        }
    }

    fn check_end_of_stream(&self) {
        if self.video.is_none() && self.audio.is_none() {
            return;
        }
        if self.is_ended(MediaType::Audio) &&
            self.is_ended(MediaType::Video) &&
            self.ready_state != MediaSourceReadyState::Closed
        {
            jsEndOfStream();
        }
    }

    fn is_ended(&self, media_type: MediaType) -> bool {
        match self.get(media_type) {
            None => true,
            Some(sb) => sb.last_segment_pushed && !sb.is_updating,
        }
    }
}

pub enum PushSegmentError {
    NoSourceBuffer(MediaType),
}

pub enum RemoveDataError {
    NoSourceBuffer(MediaType),
}

/// Identify a unique JavaScript `SourceBuffer`
pub type SourceBufferId = u32;

/// Error that can trigger when attempting to create a SourceBuffer.
#[derive(Debug)]
pub enum SourceBufferCreationError {
    /// A SourceBuffer was already created for the given `MediaType`.
    AlreadyCreatedWithSameType(MediaType),

    /// No JavaScript MediaSource is currently attached to the media element
    /// itself linked to the current dispatcher instance.
    NoMediaSourceAttached,

    /// The `SourceBuffer` could not have been created because the `MediaSource`
    /// has a "closed" state.
    MediaSourceIsClosed,

    /// The `SourceBuffer` could not have been created because a `QuotaExceededError`
    /// JavaScript error has been received while doing so. This often happens when
    /// a SourceBuffer is created after another one already had data pushed to it.
    QuotaExceededError(String),

    /// The wanted SourceBuffer could not have been created because the given
    /// mime-type and codecs couple is not supported by the current device.
    CantPlayType(String),

    /// An uncategorized error has been received while trying to create the
    /// SourceBuffer.
    UnknownError(String),
}

/// Abstraction over the Media Source Extension's `SourceBuffer` concept.
///
/// This is the interface allowing to interact with lower-level media buffers.
pub struct SourceBuffer {
    /// The `SourceBufferId` given on SourceBuffer creation, used to identify
    /// this `SourceBuffer` in the current dispatcher instance.
    id: SourceBufferId,

    /// The current queue of operations being scheduled, from the most urgent to
    /// the less urgent.
    /// The next operation in queue will be performed and removed from the queue
    /// once the SourceBuffer is not "updating" anymore.
    queue: Vec<SourceBufferQueueElement>,

    /// The Content-Type currently linked to the SourceBuffer
    typ: String,

    /// If `true`, an operation is currently performed on the `SourceBuffer`, in
    /// which case it will need to push further operations in its internal
    /// `queue`.
    is_updating: bool,

    /// Set to `true` as soon as the first operation is being performed, at
    /// which point, some actions cannot be taken anymore (like creating other
    /// `SourceBuffer` instances).
    has_been_updated: bool,

    /// If `true` the chronologically last possible media chunk has been
    /// scheduled to be pushed.
    /// This allows for example to properly "end" the `SourceBuffer`.
    last_segment_pushed: bool,
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
        let x = jsAddSourceBuffer(media_type, &typ,);
        Self {
            id: x as u32,
            typ,
            is_updating: false,
            queue: vec![],
            has_been_updated: false,
            last_segment_pushed: false,
        }
    }

//     pub fn get_buffered(&self) -> Vec<(f64, f64)> {
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

    pub fn get_segment_queue(&self) -> &[SourceBufferQueueElement] {
        &self.queue
    }

    pub fn append_buffer(&mut self, metadata: PushMetadata) {
        self.has_been_updated = true;
        if self.is_updating {
            self.queue.push(SourceBufferQueueElement::Push(metadata));
        } else {
            self.push_now(metadata);
        }
    }

    pub fn remove_buffer(&mut self, start: f64, end: f64) {
        self.has_been_updated = true;
        if self.is_updating {
            self.queue.push(SourceBufferQueueElement::Remove { start, end });
        } else {
            self.is_updating = true;
            jsRemoveBuffer(self.id, start, end);
        }
    }

    fn on_update_end(&mut self) {
        use SourceBufferQueueElement::*;
        if self.queue.is_empty() {
            self.is_updating = false;
        } else {
            let next_item = self.queue.remove(0);
            match next_item {
                Push(md) => self.push_now(md),
                Remove { start, end } => {
                    jsRemoveBuffer(self.id, start, end);
                },
            }
        }
    }

    fn push_now(&mut self, md: PushMetadata) {
        self.is_updating = true;
        match md.segment_data {
            DataSource::Raw(v) => {
                Logger::debug(&format!("Pushing raw data {} {}", self.id, self.typ));
                jsAppendBuffer(self.id, &v);
            },
            DataSource::JsBlob(j) => {
                Logger::debug(&format!("Pushing JS Blob {} {}", self.id, self.typ));
                jsAppendBufferJsBlob(self.id, j.get_id());
            },
        }

    }
}

pub struct PushMetadata {
    segment_data: DataSource,
}

impl PushMetadata {
    pub fn new(segment_data: DataSource) -> Self {
        Self { segment_data }
    }
}
