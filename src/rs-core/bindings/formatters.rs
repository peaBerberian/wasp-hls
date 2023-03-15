use crate::{
    media_element::SourceBufferCreationError,
    parser::{ByteRange, VariantStream, VideoResolution},
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

// impl<'a> AvailableAudioTrack<'a> {
//     // Byte 1-4: Length of the AudioTrack's inner data, as a little-endian u32
//     // Byte 5-9: id of the track as a little-endian u32
//     // Byte 10: `1` if current / `0` if not
//     // Byte 11: length of the language. `0` if no language.
//     // Byte 12-x: language as utf-8
//     // Byte x+1: length of the assoc_language. `0` if no assoc_language.
//     // Byte x+2-y: assoc_language as utf-8
//     // Byte y+1: length of the track's name. `0` if no name.
//     // Byte y+2-z: name as utf-8
//     // Byte z+1-z+5: Channels as a little-endian u32. `0` if unknown
//     fn serialize_for_js(&'a self) -> Vec<u8> {
//         let mut track_info = vec![];
//         // Set length at 0 for now
//         track_info.push(0);
//         track_info.push(0);
//         track_info.push(0);
//         track_info.push(0);

//         let mut current_length = 4;

//         let id_u8 = self.id().to_le_bytes(); current_length += 4;
//         track_info.extend(id_u8);
//         track_info.push(if self.is_current() { 1 } else { 0 }); current_length += 1;
//         if let Some(lang) = self.language() {
//             let len = lang.len();
//             track_info.extend(len.to_le_bytes());
//             track_info.extend(lang.as_bytes());
//             current_length += 4 + len;
//         } else {
//             track_info.push(0);
//             track_info.push(0);
//             track_info.push(0);
//             track_info.push(0);
//             current_length += 4;
//         }
//         if let Some(assoc_lang) = self.assoc_language() {
//             let len = assoc_lang.len();
//             track_info.extend(len.to_le_bytes());
//             track_info.extend(assoc_lang.as_bytes());
//             current_length += 4 + len;
//         } else {
//             track_info.push(0);
//             track_info.push(0);
//             track_info.push(0);
//             track_info.push(0);
//             current_length += 4;
//         }

//         let name = self.name();
//         let name_len = name.len();
//         track_info.extend(name_len.to_le_bytes());
//         track_info.extend(name.as_bytes());
//         current_length += 4 + name_len;

//         track_info.extend(if let Some(val) = self.channels() {
//             val.to_le_bytes()
//         } else {
//             [0; 4]
//         });
//         current_length += 4;

//         let current_length_le = current_length.to_le_bytes();
//         track_info[0] = current_length_le[0];
//         track_info[1] = current_length_le[1];
//         track_info[2] = current_length_le[2];
//         track_info[3] = current_length_le[3];

//         track_info
//     }
// }
