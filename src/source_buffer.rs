use std::fmt;

use crate::js_functions::{
    PlayerId,
    jsAddSourceBuffer,
    jsAppendBuffer,
    jsRemoveBuffer, jsGetSourceBufferBuffered,
};

/// Identify a unique JavaScript `SourceBuffer`
pub type SourceBufferId = u32;

/// Structure keeping track of the created audio and video SourceBuffers, making
/// sure only one is created for each and that one is not created once media
/// data has been pushed to any of them.
pub struct SourceBuffersStore {
    /// This identifier will identify the media element and MediaSource on the
    /// JavaScript-side (by proxy of the corresponding `WaspHlsPlayer`'s id).
    id: PlayerId,

    /// Video SourceBuffer currently created for video data.
    /// None if no SourceBuffer has been created for that type.
    pub video: Option<SourceBuffer>,

    /// Audio SourceBuffer currently created for audio data.
    /// None if no SourceBuffer has been created for that type.
    pub audio: Option<SourceBuffer>,
}

/// Error that can trigger when attempting to create a SourceBuffer.
#[derive(Debug)]
pub enum SourceBufferCreationError {
    /// A SourceBuffer was already created for the given `MediaType`.
    AlreadyCreatedWithSameType(MediaType),

    /// No JavaScript MediaSource is currently attached to the media element
    /// itself linked to the current player instance.
    NoMediaSourceAttached,

    /// The `SourceBuffer` could not have been created because the `MediaSource`
    /// has a "closed" state.
    MediaSourceIsClosed,

    /// The `SourceBuffer` could not have been created because a `QuotaExceededError`
    /// JavaScript error has been received while doing so. This often happens when
    /// a SourceBuffer is created after another one already had data pushed to it.
    QuotaExceededError,

    /// The wanted SourceBuffer could not have been created because the given
    /// mime-type and codecs couple is not supported by the current device.
    CantPlayType(String),

    /// An uncategorized error has been received while trying to create the
    /// SourceBuffer.
    /// TODO Accompanying string?
    UnknownError,
}

impl SourceBuffersStore {
    pub fn new(id: PlayerId) -> Self {
        SourceBuffersStore { id, video: None, audio: None }
    }

    pub fn can_still_create_source_buffer(&self) -> bool {
        match (&self.audio, &self.video) {
            (None, None) => true,
            (Some(asb), Some(vsb)) => !asb.has_been_updated && !vsb.has_been_updated,
            (Some(asb), None) => !asb.has_been_updated,
            (None, Some(vsb)) => !vsb.has_been_updated,
        }
    }

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
                    self.audio = Some(SourceBuffer::new(self.id, sb_codec)?);
                    Ok(())
                }
            }
            MediaType::Video => {
                if self.video.is_some() {
                    Err(SourceBufferCreationError::AlreadyCreatedWithSameType(media_type))
                } else {
                    self.video = Some(SourceBuffer::new(self.id, sb_codec)?);
                    Ok(())
                }
            }
        }
    }

    pub fn has(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.audio.is_some(),
            MediaType::Video => self.video.is_some(),
        }
    }

    pub fn get(&self, media_type: MediaType) -> Option<&SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio.as_ref(),
            MediaType::Video => self.video.as_ref()
        }
    }

    pub fn get_mut(&mut self, media_type: MediaType) -> Option<&mut SourceBuffer> {
        match media_type {
            MediaType::Audio => self.audio.as_mut(),
            MediaType::Video => self.video.as_mut()
        }
    }
}

pub struct SourceBuffer {
    player_id: PlayerId,
    pub id: SourceBufferId,
    queue: Vec<SourceBufferQueueElement>,
    typ: String,
    is_updating: bool,
    has_been_updated: bool,
}

pub enum SourceBufferQueueElement {
    Push(PushMetadata),
    Remove { start: f64, end: f64 },
}

impl SourceBuffer {
    fn new(player_id: PlayerId, typ: String) -> Result<Self, SourceBufferCreationError> {
        let res = jsAddSourceBuffer(player_id, &typ);
        match res {
            x if x >= 0 => Ok(Self {
                player_id,
                id: x as u32,
                typ,
                is_updating: false,
                queue: vec![],
                has_been_updated: false,
            }),
            -1 => Err(SourceBufferCreationError::NoMediaSourceAttached),
            -2 => Err(SourceBufferCreationError::MediaSourceIsClosed),
            -3 => Err(SourceBufferCreationError::QuotaExceededError),
            -4 => Err(SourceBufferCreationError::CantPlayType(typ)),
            _ => Err(SourceBufferCreationError::UnknownError),
        }
    }

    pub fn get_buffered(&self) -> Vec<(f64, f64)> {
        let buffered = jsGetSourceBufferBuffered(self.player_id, self.id);
        let og_len = buffered.len();
        if og_len % 2 != 0 {
            panic!("Unexpected buffered value: not even.");
        }
        let mut ret : Vec<(f64, f64)> = Vec::with_capacity(og_len / 2);
        for i in 0..og_len / 2 {
            ret.push((buffered[i], buffered[i+1]));
        }
        ret
    }

    pub fn get_segment_queue(&self) -> &[SourceBufferQueueElement] {
        &self.queue
    }

    pub fn append_buffer(&mut self, metadata: PushMetadata) {
        self.has_been_updated = true;
        if self.is_updating {
            self.queue.push(SourceBufferQueueElement::Push(metadata));
        } else {
            self.is_updating = true;
            jsAppendBuffer(self.player_id, self.id, &metadata.data);
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

    pub fn on_update_end(&mut self) {
        use SourceBufferQueueElement::*;
        if self.queue.is_empty() {
            self.is_updating = false;
        } else {
            let next_item = self.queue.remove(0);
            match next_item {
                Push(md) =>
                    jsAppendBuffer(self.player_id, self.id, &md.data),
                Remove { start, end } => jsRemoveBuffer(self.id, start, end),
            }
        }
    }
}

pub struct PushMetadata {
    pub data: Vec<u8>,
}

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
