use crate::bindings::MediaType;

/// Infer the media type from a recognized codec identifier's base name.
pub(crate) fn guess_media_type_from_codec(codec: &str) -> Option<MediaType> {
    let base = codec.split_once('.').map_or(codec, |(base, _)| base);
    match base {
        "mp4a" | "ec-3" | "ac-3" => Some(MediaType::Audio),
        "avc1" | "avc3" | "hvc1" | "hev1" | "dvh1" | "dvhe" => Some(MediaType::Video),
        _ => None,
    }
}
