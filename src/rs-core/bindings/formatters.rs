use crate::{
    media_element::SourceBufferCreationError,
    parser::{AudioTrack, ByteRange, VariantStream, VideoResolution},
};

static NULL_RESOLUTION: VideoResolution = VideoResolution::new(0, 0);

pub(crate) unsafe fn format_variants_info_for_js(variants: &[VariantStream]) -> Vec<u32> {
    let mut ret: Vec<u32> = vec![];
    ret.push(variants.len() as u32);
    variants.iter().for_each(|v| {
        let id = v.id();
        ret.push(id.len() as u32);
        ret.push(id.as_ptr() as u32); // The unsafe-related part
        let resolution = v.resolution().unwrap_or(&NULL_RESOLUTION);
        ret.push(resolution.height());
        ret.push(resolution.width());
        ret.push(v.frame_rate().unwrap_or(0.) as u32);
        ret.push(v.bandwidth() as u32);
    });
    ret
}

pub(crate) fn format_range_for_js(original: Option<&ByteRange>) -> (Option<usize>, Option<usize>) {
    match original {
        None => (None, None),
        Some(ByteRange {
            first_byte,
            last_byte,
        }) => (Some(*first_byte), Some(*last_byte)),
    }
}

use super::SourceBufferCreationErrorCode;
pub(crate) fn format_source_buffer_creation_err_for_js(
    err: SourceBufferCreationError,
) -> (SourceBufferCreationErrorCode, String) {
    match err {
        SourceBufferCreationError::EmptyMimeType => (
            crate::bindings::SourceBufferCreationErrorCode::EmptyMimeType,
            err.to_string(),
        ),
        SourceBufferCreationError::NoMediaSourceAttached { .. } => (
            crate::bindings::SourceBufferCreationErrorCode::NoMediaSourceAttached,
            err.to_string(),
        ),
        SourceBufferCreationError::MediaSourceIsClosed => (
            crate::bindings::SourceBufferCreationErrorCode::MediaSourceIsClosed,
            err.to_string(),
        ),
        SourceBufferCreationError::QuotaExceededError { .. } => (
            crate::bindings::SourceBufferCreationErrorCode::QuotaExceededError,
            err.to_string(),
        ),
        SourceBufferCreationError::CantPlayType { .. } => (
            crate::bindings::SourceBufferCreationErrorCode::CantPlayType,
            err.to_string(),
        ),
        SourceBufferCreationError::AlreadyCreatedWithSameType { .. } => (
            crate::bindings::SourceBufferCreationErrorCode::AlreadyCreatedWithSameType,
            err.to_string(),
        ),
        SourceBufferCreationError::UnknownError { .. } => (
            crate::bindings::SourceBufferCreationErrorCode::Unknown,
            err.to_string(),
        ),
    }
}

pub(crate) unsafe fn format_audio_tracks_for_js(tracks: &[AudioTrack]) -> Vec<u32> {
    let mut ret: Vec<u32> = vec![];
    ret.push(tracks.len() as u32);
    tracks.iter().for_each(|t| {
        let id = t.id();
        ret.push(id.len() as u32);
        ret.push(id.as_ptr() as u32);

        let language = t.language().unwrap_or("");
        ret.push(language.len() as u32);
        ret.push(language.as_ptr() as u32);

        let assoc_language = t.assoc_language().unwrap_or("");
        ret.push(assoc_language.len() as u32);
        ret.push(assoc_language.as_ptr() as u32);

        let name = t.name();
        ret.push(name.len() as u32);
        ret.push(name.as_ptr() as u32);
        ret.push(t.channels().unwrap_or(0));
    });
    ret
}
