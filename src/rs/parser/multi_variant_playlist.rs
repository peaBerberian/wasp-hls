use std::{ error, fmt, io };
use crate::utils::url::Url;
use super::media_tag::{MediaTag, MediaTagParsingError};
use super::variant_stream::{VariantStream, VariantParsingError};
use super::media_playlist::{MediaPlaylist, MediaPlaylistParsingError};

#[derive(Debug)]
pub struct MultiVariantPlaylist {
    variants: Vec<VariantStream>,
    media: Vec<MediaTag>,
    media_playlist_context: MediaPlaylistContext,
}

impl MultiVariantPlaylist {
    pub fn get_variant_video_playlist(&self, variant_idx: usize) -> Option<&MediaPlaylist> {
        match self.variants.get(variant_idx) {
            None => None,
            Some(v) => v.media_playlist.as_ref(),
        }
    }
}

/// Values parsed from a MultiVariantPlaylist that may have an influence on
/// parsed MediaPlaylists.
#[derive(Debug)]
struct MediaPlaylistContext {
    // version: Option<u32>,
    // independent_segments: Option<bool>,
    // start: Option<StartAttribute>,
}

// TODO information on the line at which the error was encountered?
#[derive(Debug)]
pub enum MultiVariantPlaylistParsingError {
    MissingUriLineAfterVariant,
    UnableToReadVariantUri,
    VariantMissingBandwidth,

    // TODO information about attribute?
    InvalidDecimalInteger,

    MediaTagMissingType,
    MediaTagMissingName,
    MediaTagMissingGroupId,

    UnableToReadLine,
}

impl error::Error for MultiVariantPlaylistParsingError { }

impl fmt::Display for MultiVariantPlaylistParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
           MultiVariantPlaylistParsingError::MissingUriLineAfterVariant =>
               write!(f, "A variant is missing its URI"),
           MultiVariantPlaylistParsingError::UnableToReadVariantUri =>
               write!(f, "Unable to read URI of one of the variants"),
           MultiVariantPlaylistParsingError::VariantMissingBandwidth =>
               write!(f, "A variant is missing its mandatory BANDWIDTH attribute"),
           MultiVariantPlaylistParsingError::InvalidDecimalInteger =>
               write!(f, "A decimal attribute was in the wrong format."),
           MultiVariantPlaylistParsingError::MediaTagMissingType =>
               write!(f, "A media tag is missing a TYPE attribute"),
           MultiVariantPlaylistParsingError::MediaTagMissingName =>
               write!(f, "A media tag is missing a NAME attribute"),
           MultiVariantPlaylistParsingError::MediaTagMissingGroupId =>
               write!(f, "A media tag is missing a GROUP-ID attribute"),
           MultiVariantPlaylistParsingError::UnableToReadLine =>
               write!(f, "A line of the MultiVariantPlaylist was impossible to parse"),
        }
    }
}

impl From<VariantParsingError> for MultiVariantPlaylistParsingError {
    fn from(err : VariantParsingError) -> MultiVariantPlaylistParsingError {
        match err {
            VariantParsingError::InvalidDecimalInteger =>
                MultiVariantPlaylistParsingError::InvalidDecimalInteger,
            VariantParsingError::MissingBandwidth =>
                MultiVariantPlaylistParsingError::VariantMissingBandwidth,
        }
    }
}

impl error::Error for MediaPlaylistUpdateError { }

impl From<MediaTagParsingError> for MultiVariantPlaylistParsingError {
    fn from(err : MediaTagParsingError) -> MultiVariantPlaylistParsingError {
        match err {
            MediaTagParsingError::MissingType =>
                MultiVariantPlaylistParsingError::MediaTagMissingType,
            MediaTagParsingError::MissingGroupId =>
                MultiVariantPlaylistParsingError::MediaTagMissingGroupId,
            MediaTagParsingError::MissingName =>
                MultiVariantPlaylistParsingError::MediaTagMissingName,
        }
    }
}

#[derive(Debug)]
pub enum MediaPlaylistUpdateError {
    NotFound,
    ParsingError(MediaPlaylistParsingError),
}

impl fmt::Display for MediaPlaylistUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
           MediaPlaylistUpdateError::NotFound =>
               write!(f, "The media playlist to update was not found in the MultiVariant Playlist."),
           MediaPlaylistUpdateError::ParsingError(og_error) =>
               write!(f, "Could not update MediaPlaylist: {}", og_error),
        }
    }
}

impl From<MediaPlaylistParsingError> for MediaPlaylistUpdateError {
    fn from(err : MediaPlaylistParsingError) -> MediaPlaylistUpdateError {
        MediaPlaylistUpdateError::ParsingError(err)
    }
}

impl MultiVariantPlaylist {
    pub fn parse(
        playlist: impl io::BufRead,
        url: Url
    ) -> Result<Self, MultiVariantPlaylistParsingError> {
        let playlist_base_url = url.pathname();
        let mut ret = MultiVariantPlaylist {
            media: vec![],
            variants: vec![],
            media_playlist_context: MediaPlaylistContext {},
        };

        let mut lines = playlist.lines();
        while let Some(line) = lines.next() {
            let str_line = if let Ok(s) = line {
                s
            } else {
                return Err(MultiVariantPlaylistParsingError::UnableToReadLine);
            };
            if str_line.is_empty() {
                continue;
            } else if str_line.starts_with("#EXT") {
                let colon_idx = match &str_line[4..].find(':') {
                    None => continue,
                    Some(idx) => idx + 4,
                };
                match &str_line[4..colon_idx] {
                    "-X-STREAM-INF" => {
                        let variant_url = match lines.next() {
                            None => return Err(MultiVariantPlaylistParsingError::MissingUriLineAfterVariant),
                            Some(Err(_)) => return Err(MultiVariantPlaylistParsingError::UnableToReadVariantUri),
                            Some(Ok(l)) => Url::new(l),
                        };
                        let variant_url = if variant_url.is_absolute() {
                            variant_url
                        } else {
                            Url::from_relative(playlist_base_url, variant_url)
                        };

                        let variant = VariantStream::create_from_stream_inf(&str_line, variant_url)?;
                        ret.variants.push(variant);
                    },
                    "-X-MEDIA" => {
                        let media = MediaTag::create(&str_line, &url)?;
                        ret.media.push(media);
                    },
                    _ => {},
                }
            } else if str_line.starts_with('#') {
                continue;
            } else {
                // URI
            }
        }
        ret.variants.sort_by_key(|x| x.bandwidth);
        Ok(ret)
    }

    pub fn variants(&self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    pub fn medias(&self) -> &[MediaTag] {
        self.media.as_slice()
    }

    pub fn get_variant(&self, variant_idx: usize) -> Option<&VariantStream> {
        self.variants.get(variant_idx)
    }

    pub fn update_variant_media_playlist(&mut self,
        variant_idx: usize,
        media_playlist_data: impl io::BufRead,
        url: Url
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.variants.get_mut(variant_idx) {
            Some(v) => Ok(v.update_media_playlist(media_playlist_data, url)?),
            None => Err(MediaPlaylistUpdateError::NotFound)
        }
    }

    pub fn get_media(&self, variant_idx: usize) -> Option<&MediaTag> {
        self.media.get(variant_idx)
    }

    pub fn update_media_tag_media_playlist(&mut self,
        media_tag_idx: usize,
        media_playlist_data: impl io::BufRead,
        url: Url
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.media.get_mut(media_tag_idx) {
            Some(m) => Ok(m.update(media_playlist_data, url)?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }
}
