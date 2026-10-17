mod abi_types;
pub mod formatters;
mod js_functions;

pub use abi_types::*;
pub use js_functions::*;

impl From<AvPlaybackMediaType> for MediaType {
    fn from(value: AvPlaybackMediaType) -> Self {
        match value {
            AvPlaybackMediaType::Audio => Self::Audio,
            AvPlaybackMediaType::Video => Self::Video,
        }
    }
}

impl MediaType {
    /// Narrow only when entering A/V probe or buffer operations.
    pub(crate) fn av_playback(self) -> Option<AvPlaybackMediaType> {
        match self {
            Self::Audio => Some(AvPlaybackMediaType::Audio),
            Self::Video => Some(AvPlaybackMediaType::Video),
            Self::Text => None,
        }
    }
}
