use super::audio_track_list::AudioTrackList;
use super::media_playlist::{MediaPlaylist, MediaPlaylistParsingError, StartAttribute};
use super::media_tag::{MediaTag, MediaTagParsingError};
use super::variant_stream::{VariantParsingError, VariantStream};
use super::MediaTagType;
use crate::utils::url::Url;
use std::{error, fmt, io};

/// Represents a parsed HLS MultiVariant Playlist (a.k.a. Master Playlist).
///
/// It is already pre-parsed to build useful concepts, such as audio tracks, on top of it.
pub struct MultiVariantPlaylist {

    /// Describes and contains the default MediaPlaylist of all the variants announced in the
    /// MultiVariant Playlist.
    variants: Vec<VariantStream>,

    /// Abstraction to simplify the management of tracks.
    ///
    /// It actually corresponds to pre-parsed Media tags in the MultiVariant Playlist with their
    /// `TYPE` set to `"audio"`.
    audio_tracks: AudioTrackList,

    /// Parsed Media tags in the MultiVariant Playlist which are not of `TYPE` `"audio"`
    other_media: Vec<MediaTag>,

    /// Supplementary information that will need to be communicated to other Media Playlists
    /// once they are loaded and parsed.
    context: Option<MediaPlaylistContext>,

    /// Url of the MultiVariant Playlist once it is fetched (post a potential HTTP redirect).
    url: Url,
}

impl MultiVariantPlaylist {
    /// Creates a new `MultiVariantPlaylist` object by giving its entire content through a
    /// `BufRead` Abstraction.
    pub fn parse(
        playlist: impl io::BufRead,
        url: Url,
    ) -> Result<Self, MultiVariantPlaylistParsingError> {
        let playlist_base_url = url.pathname();
        let mut variants: Vec<VariantStream> = vec![];
        let mut audio_media: Vec<MediaTag> = vec![];
        let mut other_media: Vec<MediaTag> = vec![];

        let mut lines = playlist.lines();
        while let Some(line) = lines.next() {
            let str_line = if let Ok(s) = line {
                s
            } else {
                return Err(MultiVariantPlaylistParsingError::UnableToReadLine);
            };
            if str_line.is_empty() {
                continue;
            } else if let Some(stripped) = str_line.strip_prefix("#EXT") {
                let colon_idx = match &stripped.find(':') {
                    None => continue,
                    Some(idx) => idx + 4,
                };
                match &str_line[4..colon_idx] {
                    "-X-STREAM-INF" => {
                        let variant_url =
                            match lines.next() {
                                None => return Err(
                                    MultiVariantPlaylistParsingError::MissingUriLineAfterVariant,
                                ),
                                Some(Err(_)) => {
                                    return Err(
                                        MultiVariantPlaylistParsingError::UnableToReadVariantUri,
                                    )
                                }
                                Some(Ok(l)) => Url::new(l),
                            };

                        let variant = VariantStream::create_from_stream_inf(
                            &str_line,
                            variant_url,
                            playlist_base_url,
                        )?;
                        variants.push(variant);
                    }
                    "-X-MEDIA" => {
                        let media = MediaTag::create(&str_line, &url)?;
                        if media.typ() == MediaTagType::Audio {
                            audio_media.push(media);
                        } else {
                            other_media.push(media);
                        }
                    }
                    _ => {}
                }
            } else if str_line.starts_with('#') {
                continue;
            } else {
                // URI
            }
        }
        variants.sort_by_key(|x| x.bandwidth());
        Ok(MultiVariantPlaylist {
            url,
            variants,
            audio_tracks: AudioTrackList::new(audio_media),
            other_media,
            context: None,
        })
    }

    /// Returns a reference to the last known URL of the MultiVariant Playlist.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    /// Returns information on all known variants linked to this `MultiVariantPlaylist`, ordered by
    /// `bandwidth` ascending.
    pub(crate) fn variants(&self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    /// Returns information on a specific variant linked to this `MultiVariantPlaylist` based on
    /// its `id`.
    ///
    /// Returns `None` if no variant with that `id` are found.
    pub(crate) fn variant(&self, variant_id: &str) -> Option<&VariantStream> {
        self.variants.iter().find(|v| v.id() == variant_id)
    }

    /// Returns the `MediaPlaylist` object of the default Media Playlist linked to the variant
    /// whose `id` is given in argument
    pub(crate) fn variant_default_playlist(&self, variant_id: &str) -> Option<&MediaPlaylist> {
        self.variant(variant_id)
            .and_then(|v| v.media_playlist())
    }

    /// Returns the `id` of the audio media that should be chosen when loading the variant given
    /// in argument.
    ///
    /// If an audio track is currently selected, you also should communicate its own `id` as
    /// argument, as it media influence which audio media will be choosen.
    ///
    /// Returns `None` if there's no specific audio media that can be loaded because none
    /// are compatible with the given characteristics.
    pub(crate) fn audio_media_id_for(
        &self,
        curr_variant: &VariantStream,
        curr_audio_track: Option<&str>,
    ) -> Option<&str> {
        // TODO improve this giga mess
        if let Some(group_id) = curr_variant.audio_group() {
            if let Some(track_id) = curr_audio_track {
                self.audio_tracks
                    .iter()
                    .find(|t| t.id() == track_id)
                    .and_then(|t| t.medias().iter().find(|m| m.group_id() == group_id))
                    .and_then(|m| m.id())
            } else {
                self.audio_tracks.iter_media().fold(None, |acc, m| {
                    if m.group_id() == group_id
                        && m.is_autoselect()
                        && (acc.is_none() || m.is_default())
                    {
                        m.id()
                    } else {
                        acc
                    }
                })
            }
        } else {
            None
        }
    }

    /// Returns an Option to the reference of an `Url` to the Media Playlist whose media `id` (as
    /// returned by methods such as `audio_media_id_for`) is given as argument.
    ///
    /// Returns `null` either if no audio media is found with that `id` or if there is but no
    /// particular `Url` is linked to it (e.g. because its corresponding data is directly in
    /// segments of the variant's MediaPlaylist).
    pub(crate) fn audio_url(&self, media_id: &str) -> Option<&Url> {
        self.audio_tracks.get_media(media_id).and_then(|x| {
            if let Some(playlist) = x.media_playlist() {
                Some(playlist.url())
            } else {
                x.url()
            }
        })
    }

    /// Returns optional reference to the `MediaPlaylist` linked to the given media `id`.
    ///
    /// Returns `None` either if the given `id` isn't linked to any media, if it has no
    /// linked `MediaPlaylist` or if its Media Playlist hasn't been fetched yet.
    pub(crate) fn audio_playlist(&self, media_id: &str) -> Option<&MediaPlaylist> {
        self.audio_tracks
            .get_media(media_id)
            .and_then(|x| x.media_playlist())
    }

    pub(crate) fn other_media_playlist(&self, media_id: &str) -> Option<&MediaPlaylist> {
        self.other_media
            .iter()
            .find(|x| x.id() == Some(media_id))
            .and_then(|x| x.media_playlist())
    }

    pub(crate) fn variant_from_idx(&self, idx: usize) -> Option<&VariantStream> {
        self.variants.get(idx)
    }

    pub(crate) fn other_media_url(&self, media_id: &str) -> Option<&Url> {
        self.other_media
            .iter()
            .find(|x| x.id() == Some(media_id))
            .and_then(|x| {
                if let Some(playlist) = x.media_playlist() {
                    Some(playlist.url())
                } else {
                    x.url()
                }
            })
    }

    pub(crate) fn update_variant_media_playlist(
        &mut self,
        variant_id: &str,
        media_playlist_data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.variants.iter_mut().find(|v| v.id() == variant_id) {
            Some(v) => {
                Ok(v.update_media_playlist(media_playlist_data, url, self.context.as_ref())?)
            }
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    pub(crate) fn update_audio_media_playlist(
        &mut self,
        id: &str,
        media_playlist_data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.audio_tracks.get_mut_media(id) {
            Some(m) => Ok(m.update(media_playlist_data, url, self.context.as_ref())?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    pub(crate) fn update_other_media_playlist(
        &mut self,
        media_tag_id: &str,
        media_playlist_data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self
            .other_media
            .iter_mut()
            .find(|v| v.id() == Some(media_tag_id))
        {
            Some(m) => Ok(m.update(media_playlist_data, url, self.context.as_ref())?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }
}

/// Values parsed from a MultiVariantPlaylist that may have an influence on
/// parsed MediaPlaylists.
#[derive(Debug, Default)]
pub(crate) struct MediaPlaylistContext {
    independent_segments: Option<bool>,
    start: Option<StartAttribute>,
}

impl MediaPlaylistContext {
    pub(crate) fn start(&self) -> Option<&StartAttribute> {
        self.start.as_ref()
    }
    pub(crate) fn independent_segments(&self) -> Option<bool> {
        self.independent_segments
    }
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

    Unknown,
}

impl error::Error for MultiVariantPlaylistParsingError {}

impl fmt::Display for MultiVariantPlaylistParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MultiVariantPlaylistParsingError::MissingUriLineAfterVariant => {
                write!(f, "A variant is missing its URI")
            }
            MultiVariantPlaylistParsingError::UnableToReadVariantUri => {
                write!(f, "Unable to read URI of one of the variants")
            }
            MultiVariantPlaylistParsingError::VariantMissingBandwidth => {
                write!(f, "A variant is missing its mandatory BANDWIDTH attribute")
            }
            MultiVariantPlaylistParsingError::InvalidDecimalInteger => {
                write!(f, "A decimal attribute was in the wrong format.")
            }
            MultiVariantPlaylistParsingError::MediaTagMissingType => {
                write!(f, "A media tag is missing a TYPE attribute")
            }
            MultiVariantPlaylistParsingError::MediaTagMissingName => {
                write!(f, "A media tag is missing a NAME attribute")
            }
            MultiVariantPlaylistParsingError::MediaTagMissingGroupId => {
                write!(f, "A media tag is missing a GROUP-ID attribute")
            }
            MultiVariantPlaylistParsingError::UnableToReadLine => write!(
                f,
                "A line of the MultiVariantPlaylist was impossible to parse"
            ),
            _ => write!(
                f,
                "An unknown error was encountered while parsing the MultiVariantPlaylist"
            ),
        }
    }
}

impl From<VariantParsingError> for MultiVariantPlaylistParsingError {
    fn from(err: VariantParsingError) -> MultiVariantPlaylistParsingError {
        match err {
            VariantParsingError::InvalidDecimalInteger => {
                MultiVariantPlaylistParsingError::InvalidDecimalInteger
            }
            VariantParsingError::MissingBandwidth => {
                MultiVariantPlaylistParsingError::VariantMissingBandwidth
            }
        }
    }
}

impl error::Error for MediaPlaylistUpdateError {}

impl From<MediaTagParsingError> for MultiVariantPlaylistParsingError {
    fn from(err: MediaTagParsingError) -> MultiVariantPlaylistParsingError {
        match err {
            MediaTagParsingError::MissingType => {
                MultiVariantPlaylistParsingError::MediaTagMissingType
            }
            MediaTagParsingError::MissingGroupId => {
                MultiVariantPlaylistParsingError::MediaTagMissingGroupId
            }
            MediaTagParsingError::MissingName => {
                MultiVariantPlaylistParsingError::MediaTagMissingName
            }
            _ => MultiVariantPlaylistParsingError::Unknown,
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
        match self {
            MediaPlaylistUpdateError::NotFound => write!(
                f,
                "The media playlist to update was not found in the MultiVariant Playlist."
            ),
            MediaPlaylistUpdateError::ParsingError(og_error) => {
                write!(f, "Could not update MediaPlaylist: {}", og_error)
            }
        }
    }
}

impl From<MediaPlaylistParsingError> for MediaPlaylistUpdateError {
    fn from(err: MediaPlaylistParsingError) -> MediaPlaylistUpdateError {
        MediaPlaylistUpdateError::ParsingError(err)
    }
}
