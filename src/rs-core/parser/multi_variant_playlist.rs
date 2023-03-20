use super::audio_track_list::AudioTrackList;
use super::media_playlist::{MediaPlaylist, MediaPlaylistParsingError, StartAttribute};
use super::media_tag::{MediaTag, MediaTagParsingError};
use super::variant_stream::{VariantParsingError, VariantStream};
use super::{AudioTrack, MediaTagType};
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

    last_id: u32,

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
        let mut last_id = 0u32;
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
                            last_id,
                        )?;
                        last_id += 1;
                        variants.push(variant);
                    }
                    "-X-MEDIA" => {
                        let media = MediaTag::create(&str_line, &url, last_id)?;
                        last_id += 1;
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
            last_id,
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

    pub(crate) fn all_variants(&self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    /// Returns information on all known variants linked to this `MultiVariantPlaylist`, ordered by
    /// `bandwidth` ascending, for which all codecs are known to be supported.
    pub(crate) fn supported_variants(&self) -> Vec<&VariantStream> {
        self.variants
            .iter()
            .filter(|v| v.supported().unwrap_or(false))
            .collect()
    }

    /// Returns information on all known variants linked to this `MultiVariantPlaylist`, ordered by
    /// `bandwidth` ascending, for which all codecs are known to be supported and which are linked
    /// to the given track_id
    pub(crate) fn supported_variants_for_audio(&self, track_id: u32) -> Vec<&VariantStream> {
        let group_ids = self.audio_tracks.groups_for(track_id);
        self.variants
            .iter()
            .filter(|v| {
                if let Some(group) = v.audio_group() {
                    group_ids.contains(&group) && v.supported().unwrap_or(false)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns mutable reference to information on all known variants linked to this
    /// `MultiVariantPlaylist`, ordered by `bandwidth` ascending.
    pub(crate) fn variants(&mut self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    /// Returns mutable reference to information on all known variants linked to this
    /// `MultiVariantPlaylist`, ordered by `bandwidth` ascending.
    pub(crate) fn variants_mut(&mut self) -> &mut [VariantStream] {
        self.variants.as_mut_slice()
    }

    /// Returns information on a specific variant linked to this `MultiVariantPlaylist` based on
    /// its `id`.
    ///
    /// Returns `None` if no variant with that `id` are found.
    pub(crate) fn variant(&self, id: u32) -> Option<&VariantStream> {
        self.variants.iter().find(|v| v.id() == id)
    }

    /// Returns the `MediaPlaylist` object of the default Media Playlist linked to the variant
    /// whose `id` is given in argument
    ///
    /// Returns `None` either if the variant does not exist or if it does but its `MediaPlaylist`
    /// hasn't been fetched yet.
    pub(crate) fn variant_default_playlist(&self, id: u32) -> Option<&MediaPlaylist> {
        self.variant(id).and_then(|v| v.media_playlist())
    }

    /// Returns the `id` of the video media that should be chosen when loading the variant given
    /// in argument.
    ///
    /// Returns `None` if there's no specific video media that can be loaded because none
    /// are compatible with the given characteristics.
    pub(crate) fn video_media_playlist_id_for(
        &self,
        curr_variant: &VariantStream,
    ) -> Option<MediaPlaylistPermanentId> {
        if curr_variant.has_type(crate::bindings::MediaType::Video) {
            Some(MediaPlaylistPermanentId::new(
                MediaPlaylistUrlLocation::Variant,
                curr_variant.id().to_owned(),
            ))
        } else {
            None
        }
    }

    /// Returns the `id` of the audio media that should be chosen when loading the variant given
    /// in argument.
    ///
    /// If an audio track is currently selected, you also should communicate its own `id` as
    /// argument, as it media influence which audio media will be choosen.
    ///
    /// Returns `None` if there's no specific audio media that can be loaded because none
    /// are compatible with the given characteristics.
    pub(crate) fn audio_media_playlist_id_for(
        &self,
        curr_variant: &VariantStream,
        curr_audio_track: Option<u32>,
    ) -> Option<MediaPlaylistPermanentId> {
        if let Some(group_id) = curr_variant.audio_group() {
            if let Some(track_id) = curr_audio_track {
                self.audio_tracks
                    .iter()
                    .find(|t| t.id() == track_id)
                    .and_then(|t| t.medias().iter().find(|m| m.group_id() == group_id))
                    .map(|m| {
                        if m.url().is_some() {
                            MediaPlaylistPermanentId::new(
                                MediaPlaylistUrlLocation::AudioTrack,
                                m.id().to_owned(),
                            )
                        } else {
                            // AudioTrack without URL have in fact their MediaPlaylist's URL in the
                            // variant
                            MediaPlaylistPermanentId::new(
                                MediaPlaylistUrlLocation::Variant,
                                curr_variant.id().to_owned(),
                            )
                        }
                    })
            } else {
                self.audio_tracks.iter_media().fold(None, |acc, (_, m)| {
                    if m.group_id() == group_id
                        && m.is_autoselect()
                        && (acc.is_none() || m.is_default())
                    {
                        if m.url().is_some() {
                            Some(MediaPlaylistPermanentId::new(
                                MediaPlaylistUrlLocation::AudioTrack,
                                m.id().to_owned(),
                            ))
                        } else {
                            // AudioTrack without URL have in fact their MediaPlaylist's URL in the
                            // variant
                            Some(MediaPlaylistPermanentId::new(
                                MediaPlaylistUrlLocation::Variant,
                                curr_variant.id().to_owned(),
                            ))
                        }
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
    fn audio_url(&self, media_id: u32) -> Option<&Url> {
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
    fn audio_playlist(&self, media_id: u32) -> Option<&MediaPlaylist> {
        self.audio_tracks
            .get_media(media_id)
            .and_then(|x| x.media_playlist())
    }

    fn other_media_playlist(&self, media_id: u32) -> Option<&MediaPlaylist> {
        self.other_media
            .iter()
            .find(|x| x.id() == media_id)
            .and_then(|x| x.media_playlist())
    }

    pub(crate) fn variant_from_idx(&self, idx: usize) -> Option<&VariantStream> {
        self.variants.get(idx)
    }

    fn other_media_url(&self, media_id: u32) -> Option<&Url> {
        self.other_media
            .iter()
            .find(|x| x.id() == media_id)
            .and_then(|x| {
                if let Some(playlist) = x.media_playlist() {
                    Some(playlist.url())
                } else {
                    x.url()
                }
            })
    }

    /// Returns the `Url` of the MediaPlaylist whose `MediaPlaylistPermanentId` is given in
    /// argument.
    ///
    /// Returns `None` when any of the following is true:
    ///   - The given `MediaPlaylistPermanentId` does not correspond to any known media of
    ///     the content.
    ///   - The given `MediaPlaylistPermanentId` is linked to some media which isn't linked to
    ///     a MediaPlaylist. In that condition, it is the MediaPlaylist linked to its Variant
    ///     stream that should be done.
    /// Both are probably an error as a `MediaPlaylistPermanentId` should always identify a
    /// `MediaPlaylist` .
    pub(crate) fn media_playlist_url(&self, wanted_id: &MediaPlaylistPermanentId) -> Option<&Url> {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => Some(self.variant(wanted_id.id())?.url()),
            MediaPlaylistUrlLocation::AudioTrack => self.audio_url(wanted_id.id()),
            MediaPlaylistUrlLocation::OtherMedia => self.other_media_url(wanted_id.id()),
        }
    }

    pub(crate) fn media_playlist(
        &self,
        wanted_id: &MediaPlaylistPermanentId,
    ) -> Option<&MediaPlaylist> {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => {
                Some(self.variant(wanted_id.id())?.media_playlist()?)
            }
            MediaPlaylistUrlLocation::AudioTrack => self.audio_playlist(wanted_id.id()),
            MediaPlaylistUrlLocation::OtherMedia => self.other_media_playlist(wanted_id.id()),
        }
    }

    pub(crate) fn update_media_playlist(
        &mut self,
        id: &MediaPlaylistPermanentId,
        data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match id.location() {
            MediaPlaylistUrlLocation::Variant => {
                self.update_variant_media_playlist(id.id(), data, url)
            }
            MediaPlaylistUrlLocation::AudioTrack => {
                self.update_audio_media_playlist(id.id(), data, url)
            }
            MediaPlaylistUrlLocation::OtherMedia => {
                self.update_other_media_playlist(id.id(), data, url)
            }
        }
    }

    fn update_variant_media_playlist(
        &mut self,
        variant_id: u32,
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

    fn update_audio_media_playlist(
        &mut self,
        id: u32,
        media_playlist_data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.audio_tracks.get_mut_media(id) {
            Some(m) => Ok(m.update(media_playlist_data, url, self.context.as_ref())?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    fn update_other_media_playlist(
        &mut self,
        media_tag_id: u32,
        media_playlist_data: impl io::BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.other_media.iter_mut().find(|v| v.id() == media_tag_id) {
            Some(m) => Ok(m.update(media_playlist_data, url, self.context.as_ref())?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    pub(crate) fn audio_tracks(&self) -> &[AudioTrack] {
        self.audio_tracks.as_slice()
    }

    pub(crate) fn audio_track_for_media_id(
        &self,
        id: &MediaPlaylistPermanentId,
    ) -> Option<&AudioTrack> {
        match id.location() {
            MediaPlaylistUrlLocation::AudioTrack => self.audio_tracks.track_for_media(id.id()),
            MediaPlaylistUrlLocation::Variant => {
                let variant = self.variant(id.id())?;
                let group = variant.audio_group()?;
                self.audio_tracks
                    .iter_media()
                    .find(|(_, a)| a.url().is_none() && a.group_id() == group)
                    .map(|t| t.0)
            }
            _ => None,
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

/// Identifier allowing to identify a given MediaPlaylist
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MediaPlaylistPermanentId {
    location: MediaPlaylistUrlLocation,
    id: u32,
}

impl MediaPlaylistPermanentId {
    fn new(location: MediaPlaylistUrlLocation, id: u32) -> Self {
        Self { location, id }
    }

    fn location(&self) -> MediaPlaylistUrlLocation {
        self.location
    }

    fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn as_u32(&self) -> u32 {
        self.id
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MediaPlaylistUrlLocation {
    /// This Media Playlist's URL is defined by a variant in the `MultiVariantPlaylist` object.
    Variant,
    /// This Media Playlist's URL is an audio-specific track in the `MultiVariantPlaylist` object.
    AudioTrack,
    /// This Media Playlist's URL is defined as another media in the `MultiVariantPlaylist` object.
    OtherMedia,
}
