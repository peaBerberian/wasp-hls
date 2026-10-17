use super::{
    audio_track_list::AudioTrackList,
    media_playlist::{MediaPlaylist, MediaPlaylistParsingError},
    media_tag::{MediaTag, MediaTagParsingError},
    playlist_lines::read_playlist_line,
    timeline_sync::TimelineReference,
    top_level_playlist::{
        is_media_playlist_tag_name, is_media_segment_tag_name,
        multivariant_playlist_singleton_tag_name, tag_name,
    },
    value_parsers::{parse_start_attribute, StartAttribute},
    variable_substitution::{
        parse_define_tag, VariableDefinition, VariableDefinitionError, VariableStore,
    },
    variant_stream::{VariantParsingError, VariantStream},
    AudioTrack, MediaTagType,
};
use crate::{
    bindings::MediaType,
    utils::{logger::*, url::Url},
};
use std::{
    collections::{HashMap, HashSet},
    error, fmt, io,
};

/// Represents a parsed HLS Multivariant Playlist (a.k.a. Master Playlist).
///
/// It is already pre-parsed to build useful concepts, such as audio tracks, on top of it.
pub struct MultivariantPlaylist {
    /// Variant metadata, independent of the media playlists they reference.
    variants: Vec<VariantStream>,

    /// Primary media playlists. Variants with the same resolved URI share an entry.
    primary_media: Vec<PrimaryMediaPlaylist>,

    /// Maps variant's id to index in `primary_media`
    primary_id_by_variant: HashMap<u32, usize>,

    /// Abstraction to simplify the management of tracks.
    ///
    /// It actually corresponds to pre-parsed Media tags in the Multivariant Playlist with their
    /// `TYPE` set to `"audio"`.
    audio_tracks: AudioTrackList,

    /// Parsed Media tags in the Multivariant Playlist which are not of `TYPE` `"audio"`
    other_media: Vec<MediaTag>,

    /// Supplementary information that will need to be communicated to other Media Playlists
    /// once they are loaded and parsed.
    context: MediaPlaylistContext,

    last_id: u32,

    /// Url of the Multivariant Playlist once it is fetched (post a potential HTTP redirect).
    url: Url,
}

/// MediaPlaylist information directly linked to a variant stream (EXT-X-STREAM-INF)
struct PrimaryMediaPlaylist {
    /// URL where to reach that playlist
    url: Url,
    /// Media type inferred from variant declarations, when known.
    media_type: Option<MediaType>,
    /// Playlist information, if fetched
    playlist: Option<MediaPlaylist>,
}

impl MultivariantPlaylist {
    /// Creates a new `MultivariantPlaylist` object by giving its entire content through a
    /// `BufRead` Abstraction.
    pub fn parse(
        mut playlist: impl io::BufRead,
        url: Url,
    ) -> Result<Self, MultivariantPlaylistParsingError> {
        let mut last_id = 0u32;
        let playlist_base_url = url.pathname();
        let mut variants: Vec<VariantStream> = vec![];
        let mut audio_media: Vec<MediaTag> = vec![];
        let mut other_media: Vec<MediaTag> = vec![];
        let mut start = None;
        let mut independent_segments = None;
        let mut variable_store = VariableStore::from_url(&url);
        let mut seen_singleton_tags = HashSet::new();

        let mut line_bytes = Vec::new();
        let mut variant_line_bytes = Vec::new();
        match read_playlist_line(&mut playlist, &mut line_bytes) {
            Ok(Some(x)) if x == "#EXTM3U" => {
                // Fine
            }
            _ => {
                return Err(MultivariantPlaylistParsingError::Unknown);
            }
        }
        while let Some(str_line) = read_playlist_line(&mut playlist, &mut line_bytes)
            .map_err(|_| MultivariantPlaylistParsingError::UnableToReadLine)?
        {
            if str_line.is_empty() {
                continue;
            } else if let Some(stripped) = str_line.strip_prefix("#EXT") {
                if let Some(tag_name) = tag_name(&str_line) {
                    if is_media_playlist_tag_name(tag_name) || is_media_segment_tag_name(tag_name) {
                        return Err(MultivariantPlaylistParsingError::ConflictingPlaylistTagTypes);
                    }

                    if let Some(singleton_tag_name) =
                        multivariant_playlist_singleton_tag_name(tag_name)
                    {
                        if !seen_singleton_tags.insert(singleton_tag_name) {
                            return Err(MultivariantPlaylistParsingError::DuplicateTag);
                        }
                    }
                }

                let colon_idx = match &stripped.find(':') {
                    None => continue,
                    Some(idx) => idx + 4,
                };
                match &str_line[4..colon_idx] {
                    // NOTE: Like hls.js and Shaka, do not reject playlists solely on
                    // EXT-X-VERSION. This deliberately relaxes sections 6.3.1 and 8
                    // to avoid rejecting otherwise playable content; a version
                    // ceiling cannot describe our individual feature support.
                    "-X-VERSION" => {}
                    "-X-DEFINE" => match parse_define_tag(&str_line) {
                        Ok(VariableDefinition::Name { name, value }) => {
                            variable_store.define(name, value)?
                        }
                        Ok(VariableDefinition::Import { .. }) => {
                            return Err(MultivariantPlaylistParsingError::Unknown);
                        }
                        Ok(VariableDefinition::QueryParam { name }) => {
                            variable_store.define_query_param(&name)?
                        }
                        Err(err) => return Err(err.into()),
                    },
                    "-X-STREAM-INF" => {
                        let variant_url = match read_playlist_line(
                            &mut playlist,
                            &mut variant_line_bytes,
                        ) {
                            Ok(None) => {
                                return Err(
                                    MultivariantPlaylistParsingError::MissingUriLineAfterVariant,
                                )
                            }
                            Err(_) => {
                                return Err(
                                    MultivariantPlaylistParsingError::UnableToReadVariantUri,
                                )
                            }
                            Ok(Some(l)) => {
                                let trimmed = l.trim();
                                if trimmed.is_empty() || trimmed.starts_with('#') {
                                    return Err(
                                            MultivariantPlaylistParsingError::MissingUriLineAfterVariant,
                                        );
                                }
                                let line = variable_store.substitute(&l)?;
                                Url::new(line.into_owned())
                            }
                        };

                        let variant = VariantStream::create_from_stream_inf(
                            &str_line,
                            variant_url,
                            playlist_base_url,
                            last_id,
                            &variable_store,
                        )?;
                        last_id += 1;
                        variants.push(variant);
                    }
                    "-X-MEDIA" => {
                        let media = MediaTag::create(&str_line, &url, last_id, &variable_store)?;
                        last_id += 1;
                        if media.typ() == MediaTagType::Audio {
                            audio_media.push(media);
                        } else {
                            other_media.push(media);
                        }
                    }
                    "-X-INDEPENDENT-SEGMENTS" => independent_segments = Some(true),
                    "-X-START" => match parse_start_attribute(&str_line) {
                        Ok(st) => {
                            start = Some(st);
                        }
                        _ => {
                            log_warn!("Parser: Failed to parse `EXT-X-START` attribute");
                        }
                    },
                    _ => {}
                }
            } else if str_line.starts_with('#') {
                continue;
            } else {
                // URI
            }
        }

        // Renditions may be declared after the variants that reference them.
        for variant in &variants {
            if let Some(group_id) = variant.audio_group() {
                if !audio_media.iter().any(|media| media.group_id() == group_id) {
                    return Err(MultivariantPlaylistParsingError::MissingAudioGroup(
                        group_id.to_owned(),
                    ));
                }
            }
        }

        use std::cmp::Ordering;
        variants.sort_by(|a, b| {
            let cmp = a
                .score()
                .unwrap_or(0.)
                .partial_cmp(&b.score().unwrap_or(0.))
                .unwrap_or(Ordering::Equal);
            if cmp == Ordering::Equal {
                a.bandwidth().cmp(&b.bandwidth())
            } else {
                cmp
            }
        });

        let mut primary_media = Vec::new();
        let mut primary_id_by_variant = HashMap::new();
        let mut primary_id_by_uri = HashMap::new();
        for variant in &variants {
            let media_type = if variant.codecs(MediaType::Video).is_none()
                && variant.codecs(MediaType::Audio).is_none()
            {
                None
            } else if variant.has_type(MediaType::Video) {
                Some(MediaType::Video)
            } else if variant.has_type(MediaType::Audio) {
                Some(MediaType::Audio)
            } else {
                None
            };

            let primary_id = *primary_id_by_uri
                .entry(variant.url().get_ref())
                .or_insert_with(|| {
                    // (id is the index in the `primary_media` array)
                    let id = primary_media.len();
                    primary_media.push(PrimaryMediaPlaylist {
                        url: variant.url().clone(),
                        media_type,
                        playlist: None,
                    });
                    id
                });

            // For a shared URL, prefer a video declaration, then audio, then unknown.
            let primary = &mut primary_media[primary_id];
            if media_type == Some(MediaType::Video) || primary.media_type.is_none() {
                primary.media_type = media_type;
            }

            primary_id_by_variant.insert(variant.id(), primary_id);
        }

        Ok(MultivariantPlaylist {
            last_id,
            url,
            variants,
            primary_media,
            primary_id_by_variant,
            audio_tracks: AudioTrackList::new(audio_media),
            other_media,
            context: MediaPlaylistContext {
                start,
                independent_segments,
                multivariant_variables: variable_store.definitions().clone(),
            },
        })
    }

    /// Returns a reference to the last known URL of the Multivariant Playlist.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    pub(crate) fn all_variants(&self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    /// Media type declared for a playlist, whether selected or not. Muxed playlists use Video.
    /// Returns None when the type is not known from the multivariant playlist.
    pub(crate) fn playlist_media_type(&self, id: &MediaPlaylistPermanentId) -> Option<MediaType> {
        match id.location() {
            MediaPlaylistUrlLocation::Variant => {
                self.primary_media.get(id.id() as usize)?.media_type
            }
            MediaPlaylistUrlLocation::AudioTrack => self
                .audio_tracks
                .media_tag(id.id())
                .map(|_| MediaType::Audio),
            MediaPlaylistUrlLocation::OtherMedia => self
                .other_media
                .iter()
                .find(|media| media.id() == id.id())
                .and_then(|media| match media.typ() {
                    MediaTagType::Subtitles => Some(MediaType::Text),
                    MediaTagType::Video => Some(MediaType::Video),
                    _ => None,
                }),
            MediaPlaylistUrlLocation::Direct => None,
        }
    }

    /// Returns information on all known variants linked to this `MultivariantPlaylist`, ordered by
    /// `bandwidth` ascending, which are linked to the given track_id.
    pub(crate) fn variants_for_audio(&self, track_id: u32) -> Vec<&VariantStream> {
        let group_ids = self.audio_tracks.groups_for_track_id(track_id);
        self.variants
            .iter()
            .filter(|v| {
                if let Some(group) = v.audio_group() {
                    group_ids.contains(&group)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns mutable reference to information on all known variants linked to this
    /// `MultivariantPlaylist`, ordered by `bandwidth` ascending.
    pub(crate) fn variants(&mut self) -> &[VariantStream] {
        self.variants.as_slice()
    }

    /// Returns mutable reference to information on all known variants linked to this
    /// `MultivariantPlaylist`, ordered by `bandwidth` ascending.
    pub(crate) fn variants_mut(&mut self) -> &mut [VariantStream] {
        self.variants.as_mut_slice()
    }

    /// Returns information on a specific variant linked to this `MultivariantPlaylist` based on
    /// its `id`.
    ///
    /// Returns `None` if no variant with that `id` are found.
    pub(crate) fn variant(&self, id: u32) -> Option<&VariantStream> {
        self.variants.iter().find(|v| v.id() == id)
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
                (*self.primary_id_by_variant.get(&curr_variant.id())?)
                    .try_into()
                    .ok()?,
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
        let primary_id: u32 = (*self.primary_id_by_variant.get(&curr_variant.id())?)
            .try_into()
            .ok()?;
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
                                primary_id,
                            )
                        }
                    })
            } else {
                self.audio_tracks
                    .iter_tracks_media()
                    .fold(None, |acc, (_, m)| {
                        if m.group_id() == group_id && (acc.is_none() || m.is_default()) {
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
                                    primary_id,
                                ))
                            }
                        } else {
                            acc
                        }
                    })
            }
        } else if curr_variant.has_type(MediaType::Audio)
            && !curr_variant.has_type(MediaType::Video)
        {
            // Without an AUDIO group, an audio-only variant carries its own audio.
            Some(MediaPlaylistPermanentId::new(
                MediaPlaylistUrlLocation::Variant,
                primary_id,
            ))
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
        self.audio_tracks.media_tag(media_id).and_then(|x| {
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
            .media_tag(media_id)
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
    ///   - The given resource is not linked to a MultiVariantPlaylist
    ///
    /// Those are probably errors as a `MediaPlaylistPermanentId` should always identify a
    /// `MediaPlaylist`.
    pub(crate) fn media_playlist_url(&self, wanted_id: &MediaPlaylistPermanentId) -> Option<&Url> {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => {
                let media = self.primary_media.get(wanted_id.id() as usize)?;
                Some(
                    media
                        .playlist
                        .as_ref()
                        .map_or(&media.url, |playlist| playlist.url()),
                )
            }
            MediaPlaylistUrlLocation::AudioTrack => self.audio_url(wanted_id.id()),
            MediaPlaylistUrlLocation::OtherMedia => self.other_media_url(wanted_id.id()),
            MediaPlaylistUrlLocation::Direct => None, // Should not happen
        }
    }

    /// Returns reference to the `MediaPlaylist` whose `MediaPlaylistPermanentId` is given in
    /// argument.
    pub(crate) fn media_playlist(
        &self,
        wanted_id: &MediaPlaylistPermanentId,
    ) -> Option<&MediaPlaylist> {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => self
                .primary_media
                .get(wanted_id.id() as usize)?
                .playlist
                .as_ref(),
            MediaPlaylistUrlLocation::AudioTrack => self.audio_playlist(wanted_id.id()),
            MediaPlaylistUrlLocation::OtherMedia => self.other_media_playlist(wanted_id.id()),
            MediaPlaylistUrlLocation::Direct => None, // Should not happen
        }
    }

    pub(crate) fn media_playlist_mut(
        &mut self,
        wanted_id: &MediaPlaylistPermanentId,
    ) -> Option<&mut MediaPlaylist> {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => self
                .primary_media
                .get_mut(wanted_id.id() as usize)
                .and_then(|media| media.playlist.as_mut()),
            MediaPlaylistUrlLocation::AudioTrack => self
                .audio_tracks
                .media_tag_mut(wanted_id.id())
                .and_then(|m| m.media_playlist_mut()),
            MediaPlaylistUrlLocation::OtherMedia => self
                .other_media
                .iter_mut()
                .find(|m| m.id() == wanted_id.id())
                .and_then(|m| m.media_playlist_mut()),
            MediaPlaylistUrlLocation::Direct => None,
        }
    }

    pub(crate) fn clear_media_playlist(&mut self, wanted_id: &MediaPlaylistPermanentId) {
        match wanted_id.location() {
            MediaPlaylistUrlLocation::Variant => {
                if let Some(media) = self.primary_media.get_mut(wanted_id.id() as usize) {
                    media.playlist = None;
                }
            }
            MediaPlaylistUrlLocation::AudioTrack => {
                if let Some(media_tag) = self.audio_tracks.media_tag_mut(wanted_id.id()) {
                    media_tag.clear_media_playlist();
                }
            }
            MediaPlaylistUrlLocation::OtherMedia => {
                if let Some(media_tag) = self
                    .other_media
                    .iter_mut()
                    .find(|m| m.id() == wanted_id.id())
                {
                    media_tag.clear_media_playlist();
                }
            }
            MediaPlaylistUrlLocation::Direct => {}
        }
    }

    pub(crate) fn update_media_playlist(
        &mut self,
        id: &MediaPlaylistPermanentId,
        data: impl io::BufRead,
        url: Url,
        sync_playlist_id: Option<MediaPlaylistPermanentId>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        let timeline_reference = sync_playlist_id
            .filter(|reference_id| reference_id != id)
            .and_then(|reference_id| self.media_playlist(&reference_id))
            .map(|pl| {
                TimelineReference::from_segment_list(
                    pl.segment_list().media(),
                    pl.uses_program_date_time(),
                )
            });
        match id.location() {
            MediaPlaylistUrlLocation::Variant => {
                self.update_variant_media_playlist(id.id(), data, url, timeline_reference.as_ref())
            }
            MediaPlaylistUrlLocation::AudioTrack => {
                self.update_audio_media_playlist(id.id(), data, url, timeline_reference.as_ref())
            }
            MediaPlaylistUrlLocation::OtherMedia => {
                self.update_other_media_playlist(id.id(), data, url, timeline_reference.as_ref())
            }
            // Should not happen
            MediaPlaylistUrlLocation::Direct => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    fn update_variant_media_playlist(
        &mut self,
        primary_id: u32,
        media_playlist_data: impl io::BufRead,
        url: Url,
        timeline_reference: Option<&TimelineReference>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        let media = self
            .primary_media
            .get_mut(primary_id as usize)
            .ok_or(MediaPlaylistUpdateError::NotFound)?;
        let playlist = MediaPlaylist::create(
            media_playlist_data,
            url,
            media.playlist.as_ref(),
            timeline_reference,
            &self.context,
        )?;
        Ok(media.playlist.insert(playlist))
    }

    fn update_audio_media_playlist(
        &mut self,
        id: u32,
        media_playlist_data: impl io::BufRead,
        url: Url,
        timeline_reference: Option<&TimelineReference>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.audio_tracks.media_tag_mut(id) {
            Some(m) => Ok(m.update(media_playlist_data, url, timeline_reference, &self.context)?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    fn update_other_media_playlist(
        &mut self,
        media_tag_id: u32,
        media_playlist_data: impl io::BufRead,
        url: Url,
        timeline_reference: Option<&TimelineReference>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match self.other_media.iter_mut().find(|v| v.id() == media_tag_id) {
            Some(m) => Ok(m.update(media_playlist_data, url, timeline_reference, &self.context)?),
            None => Err(MediaPlaylistUpdateError::NotFound),
        }
    }

    /// Resolve an explicit subtitle rendition only within the variant's declared group.
    /// Selection policy (including equivalent renditions in another group) belongs to the core.
    pub(crate) fn subtitle_media_playlist_id_for(
        &self,
        variant: &VariantStream,
        track_id: u32,
    ) -> Option<MediaPlaylistPermanentId> {
        let group = variant.subtitle_group()?;
        self.other_media
            .iter()
            .find(|m| {
                m.id() == track_id
                    && m.typ() == MediaTagType::Subtitles
                    && m.group_id() == group
                    && m.url().is_some()
            })
            .map(|m| MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::OtherMedia, m.id()))
    }

    /// Subtitle renditions referenced by at least one variant. IDs identify renditions;
    /// a core selection policy must preserve preferences when switching groups.
    pub(crate) fn subtitle_tracks(&self) -> Vec<&MediaTag> {
        self.other_media
            .iter()
            .filter(|m| {
                m.typ() == MediaTagType::Subtitles
                    && m.url().is_some()
                    && self
                        .variants
                        .iter()
                        .any(|v| v.subtitle_group() == Some(m.group_id()))
            })
            .collect()
    }

    // TODO(subtitles): resolve selected subtitle rendition against the active variant,
    // store/update its media playlist and report changes independently of A/V readiness.
    pub(crate) fn audio_tracks(&self) -> &[AudioTrack] {
        self.audio_tracks.as_slice()
    }

    pub(crate) fn audio_track_for_media_id(
        &self,
        id: &MediaPlaylistPermanentId,
    ) -> Option<&AudioTrack> {
        match id.location() {
            MediaPlaylistUrlLocation::AudioTrack => self.audio_tracks.track_for_media_tag(id.id()),
            _ => None,
        }
    }
}

/// Values parsed from a MultivariantPlaylist that may have an influence on
/// parsed MediaPlaylists.
#[derive(Debug, Default)]
pub(crate) struct MediaPlaylistContext {
    independent_segments: Option<bool>,
    start: Option<StartAttribute>,
    multivariant_variables: HashMap<String, String>,
}

impl MediaPlaylistContext {
    pub(crate) fn start(&self) -> Option<&StartAttribute> {
        self.start.as_ref()
    }
    pub(crate) fn independent_segments(&self) -> Option<bool> {
        self.independent_segments
    }
    pub(crate) fn multivariant_variables(&self) -> &HashMap<String, String> {
        &self.multivariant_variables
    }
}

// NOTE: should we add information on the line at which the error was encountered?
// It may not be always trivial relatively to the cost of development though.
#[derive(Debug)]
pub(crate) enum MultivariantPlaylistParsingError {
    MissingUriLineAfterVariant,
    UnableToReadVariantUri,
    VariantMissingBandwidth,
    MissingAudioGroup(String),
    DuplicateTag,
    ConflictingPlaylistTagTypes,

    // TODO information about which attribute we're talking about?
    InvalidDecimalInteger,

    MediaTagMissingType,
    MediaTagMissingName,
    MediaTagMissingGroupId,

    UnableToReadLine,

    // TODO: Should we split those into each define error variants?
    // Or would that be too much different error codes?
    VariableDefinition(VariableDefinitionError),

    Unknown,
}

impl error::Error for MultivariantPlaylistParsingError {}

impl fmt::Display for MultivariantPlaylistParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MultivariantPlaylistParsingError::MissingUriLineAfterVariant => {
                write!(f, "A variant is missing its URI")
            }
            MultivariantPlaylistParsingError::UnableToReadVariantUri => {
                write!(f, "Unable to read URI of one of the variants")
            }
            MultivariantPlaylistParsingError::VariantMissingBandwidth => {
                write!(f, "A variant is missing its mandatory BANDWIDTH attribute")
            }
            MultivariantPlaylistParsingError::MissingAudioGroup(group_id) => {
                write!(f, "A variant references undefined AUDIO group \"{}\"", group_id)
            }
            MultivariantPlaylistParsingError::DuplicateTag => {
                write!(f, "A singleton tag was duplicated in the Multivariant Playlist")
            }
            MultivariantPlaylistParsingError::ConflictingPlaylistTagTypes => write!(
                f,
                "The Multivariant Playlist contains Media Playlist or Media Segment tags and must fail to parse"
            ),
            MultivariantPlaylistParsingError::InvalidDecimalInteger => {
                write!(f, "A decimal attribute was in the wrong format.")
            }
            MultivariantPlaylistParsingError::MediaTagMissingType => {
                write!(f, "A media tag is missing a TYPE attribute")
            }
            MultivariantPlaylistParsingError::MediaTagMissingName => {
                write!(f, "A media tag is missing a NAME attribute")
            }
            MultivariantPlaylistParsingError::MediaTagMissingGroupId => {
                write!(f, "A media tag is missing a GROUP-ID attribute")
            }
            MultivariantPlaylistParsingError::UnableToReadLine => write!(
                f,
                "A line of the MultivariantPlaylist was impossible to parse"
            ),
            MultivariantPlaylistParsingError::VariableDefinition(err) => {
                write!(f, "Invalid EXT-X-DEFINE usage in the Multivariant Playlist: {:?}", err)
            }
            MultivariantPlaylistParsingError::Unknown => write!(
                f,
                "An unknown error was encountered while parsing the MultivariantPlaylist"
            ),
        }
    }
}

impl From<VariantParsingError> for MultivariantPlaylistParsingError {
    fn from(err: VariantParsingError) -> MultivariantPlaylistParsingError {
        match err {
            VariantParsingError::InvalidDecimalInteger => {
                MultivariantPlaylistParsingError::InvalidDecimalInteger
            }
            VariantParsingError::MissingBandwidth => {
                MultivariantPlaylistParsingError::VariantMissingBandwidth
            }
        }
    }
}

impl error::Error for MediaPlaylistUpdateError {}

impl From<MediaTagParsingError> for MultivariantPlaylistParsingError {
    fn from(err: MediaTagParsingError) -> MultivariantPlaylistParsingError {
        match err {
            MediaTagParsingError::MissingType => {
                MultivariantPlaylistParsingError::MediaTagMissingType
            }
            MediaTagParsingError::MissingGroupId => {
                MultivariantPlaylistParsingError::MediaTagMissingGroupId
            }
            MediaTagParsingError::MissingName => {
                MultivariantPlaylistParsingError::MediaTagMissingName
            }
            _ => MultivariantPlaylistParsingError::Unknown,
        }
    }
}

impl From<VariableDefinitionError> for MultivariantPlaylistParsingError {
    fn from(err: VariableDefinitionError) -> MultivariantPlaylistParsingError {
        MultivariantPlaylistParsingError::VariableDefinition(err)
    }
}

#[derive(Debug)]
pub(crate) enum MediaPlaylistUpdateError {
    NotFound,
    ParsingError(MediaPlaylistParsingError),
}

impl fmt::Display for MediaPlaylistUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MediaPlaylistUpdateError::NotFound => write!(
                f,
                "The media playlist to update was not found in the Multivariant Playlist."
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MediaPlaylistPermanentId {
    location: MediaPlaylistUrlLocation,
    id: u32,
}

impl MediaPlaylistPermanentId {
    pub(crate) fn new(location: MediaPlaylistUrlLocation, id: u32) -> Self {
        Self { location, id }
    }

    fn location(&self) -> MediaPlaylistUrlLocation {
        self.location
    }

    fn id(&self) -> u32 {
        self.id
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(id: u32) -> Self {
        Self::new(MediaPlaylistUrlLocation::Variant, id)
    }

    #[cfg(test)]
    pub(crate) fn as_u32(&self) -> u32 {
        self.id
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum MediaPlaylistUrlLocation {
    /// This Media Playlist's URL is defined by a variant in the `MultivariantPlaylist` object.
    Variant,
    /// This Media Playlist's URL is an audio-specific track in the `MultivariantPlaylist` object.
    AudioTrack,
    /// This Media Playlist's URL is defined as another media in the `MultivariantPlaylist` object.
    OtherMedia,
    /// This Media Playlist was directly loaded as the top-level playlist.
    /// In that case, no `MultiVariantPlaylist` should be available.
    Direct,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_text_codecs_separately_from_muxed_audio_and_video() {
        let input = b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000,CODECS=\"avc1.42E01E,mp4a.40.2,stpp.ttml.im1t\"\nvideo.m3u8\n";
        let parsed = MultivariantPlaylist::parse(
            &input[..],
            Url::new("https://example.com/master.m3u8".into()),
        )
        .unwrap();
        let variant = parsed.variant_from_idx(0).unwrap();
        assert!(variant.has_type(MediaType::Text));
        assert_eq!(
            variant.codecs(MediaType::Text).as_deref(),
            Some("stpp.ttml.im1t")
        );
        let video = variant.codecs(MediaType::Video).unwrap();
        assert!(video.contains("mp4a.40.2"));
        assert!(!video.contains("stpp"));
    }

    #[test]
    fn subtitle_renditions_resolve_groups_and_parse_independent_playlists() {
        let input = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="English",LANGUAGE="en",ASSOC-LANGUAGE="en-US",DEFAULT=YES,AUTOSELECT=YES,FORCED=YES,CHARACTERISTICS="public.accessibility.transcribes-spoken-dialog",URI="subs/en.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="other",NAME="Other",URI="other.m3u8"
#EXT-X-MEDIA:TYPE=CLOSED-CAPTIONS,GROUP-ID="cc",NAME="CC",INSTREAM-ID="CC1"
#EXT-X-STREAM-INF:BANDWIDTH=100000,SUBTITLES="subs"
video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=200000,SUBTITLES="other"
video2.m3u8
"#;
        let mut parsed = MultivariantPlaylist::parse(
            input.as_bytes(),
            Url::new("https://example.com/master.m3u8".into()),
        )
        .unwrap();
        let tracks = parsed.subtitle_tracks();
        assert_eq!(tracks.len(), 2);
        let english = tracks.iter().find(|t| t.language() == Some("en")).unwrap();
        assert_eq!(english.assoc_language(), Some("en-US"));
        assert!(english.is_default() && english.is_autoselect() && english.is_forced());
        assert_eq!(
            english.characteristics(),
            &["public.accessibility.transcribes-spoken-dialog"]
        );
        let track_id = english.id();
        let variant = parsed
            .all_variants()
            .iter()
            .find(|v| v.subtitle_group() == Some("subs"))
            .unwrap();
        let playlist_id = parsed
            .subtitle_media_playlist_id_for(variant, track_id)
            .unwrap();
        let other = parsed
            .all_variants()
            .iter()
            .find(|v| v.subtitle_group() == Some("other"))
            .unwrap();
        assert!(parsed
            .subtitle_media_playlist_id_for(other, track_id)
            .is_none());
        assert_eq!(
            parsed.playlist_media_type(&playlist_id),
            Some(crate::bindings::MediaType::Text)
        );
        assert_eq!(
            parsed.media_playlist_url(&playlist_id).unwrap().get_ref(),
            "https://example.com/subs/en.m3u8"
        );
        let media = b"#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:901\n#EXT-X-MAP:URI=\"header.vtt\"\n#EXTINF:6,\nsegment.vtt\n#EXT-X-ENDLIST\n";
        parsed
            .update_media_playlist(
                &playlist_id,
                &media[..],
                Url::new("https://example.com/subs/en.m3u8".into()),
                None,
            )
            .unwrap();
        assert!(parsed.media_playlist(&playlist_id).is_some());
        parsed.clear_media_playlist(&playlist_id);
        assert!(parsed.media_playlist(&playlist_id).is_none());
    }

    #[test]
    fn audio_only_variant_without_audio_group_selects_its_primary_playlist() {
        let playlist = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=65000,CODECS="mp4a.40.5"
audio-only.m3u8
"#;
        let parsed = MultivariantPlaylist::parse(
            playlist.as_bytes(),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();
        let variant = parsed.variant_from_idx(0).unwrap();

        assert!(parsed.video_media_playlist_id_for(variant).is_none());
        let audio_id = parsed
            .audio_media_playlist_id_for(variant, None)
            .expect("An audio-only variant must select its own media playlist");
        assert_eq!(audio_id.location(), MediaPlaylistUrlLocation::Variant);
        assert_eq!(
            parsed.media_playlist_url(&audio_id).unwrap().get_ref(),
            "https://example.com/audio-only.m3u8"
        );
    }

    #[test]
    fn retains_variants_with_unknown_required_attributes_and_enum_values() {
        for attribute in ["REQ-FUTURE=YES", "HDCP-LEVEL=FUTURE", "VIDEO-RANGE=FUTURE"] {
            let playlist =
                format!("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,{attribute}\nvideo.m3u8\n");
            let parsed = MultivariantPlaylist::parse(
                playlist.as_bytes(),
                Url::new("https://example.com/master.m3u8".to_owned()),
            )
            .unwrap();
            assert_eq!(parsed.variants.len(), 1, "{attribute}");
        }
    }

    #[test]
    fn retained_variants_still_validate_attributes_and_uri_lines() {
        for uri in ["", "#EXT-X-STREAM-INF:BANDWIDTH=1000"] {
            let playlist =
                format!("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,REQ-FUTURE=YES\n{uri}\n");
            assert!(matches!(
                MultivariantPlaylist::parse(
                    playlist.as_bytes(),
                    Url::new("https://example.com/master.m3u8".to_owned())
                ),
                Err(MultivariantPlaylistParsingError::MissingUriLineAfterVariant)
            ));
        }
        let playlist =
            b"#EXTM3U\n#EXT-X-STREAM-INF:CODECS=\"{$missing}\",REQ-FUTURE=YES\nvideo.m3u8\n";
        assert!(MultivariantPlaylist::parse(
            &playlist[..],
            Url::new("https://example.com/master.m3u8".to_owned())
        )
        .is_err());
    }

    #[test]
    fn unknown_tags_are_ignored() {
        let playlist = b"#EXTM3U\n#EXT-X-FUTURE\n#EXT-X-FUTURE-ATTR:VALUE=YES\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n";
        let parsed = MultivariantPlaylist::parse(
            &playlist[..],
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();
        assert_eq!(parsed.variants.len(), 1);
    }

    #[test]
    fn variants_with_the_same_primary_video_uri_share_playlist_identity_and_state() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="french",NAME="French",URI="french.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="english",NAME="English",URI="english.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="french"
video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.4D401F,mp4a.40.2",AUDIO="english"
video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=3000,CODECS="avc1.4D401F,mp4a.40.2",AUDIO="english"
other-video.m3u8
"#;
        let mut parsed = MultivariantPlaylist::parse(
            multivariant.as_bytes(),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();
        let first = parsed.variant_from_idx(0).unwrap().id();
        let second = parsed.variant_from_idx(1).unwrap().id();
        let third = parsed.variant_from_idx(2).unwrap().id();
        assert_ne!(first, second);
        let first_video = parsed
            .video_media_playlist_id_for(parsed.variant(first).unwrap())
            .unwrap();
        let second_video = parsed
            .video_media_playlist_id_for(parsed.variant(second).unwrap())
            .unwrap();
        assert_eq!(first_video, second_video);
        assert_eq!(first_video.id(), 0);
        assert_eq!(
            parsed
                .video_media_playlist_id_for(parsed.variant(third).unwrap())
                .unwrap()
                .id(),
            1
        );
        assert_ne!(
            parsed.audio_media_playlist_id_for(parsed.variant(first).unwrap(), None),
            parsed.audio_media_playlist_id_for(parsed.variant(second).unwrap(), None)
        );

        parsed
            .update_media_playlist(
                &first_video,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nsegment.ts\n#EXT-X-ENDLIST\n"
                    .as_slice(),
                Url::new("https://example.com/video.m3u8".to_owned()),
                None,
            )
            .unwrap();
        assert!(parsed.media_playlist(&second_video).is_some());
        parsed.clear_media_playlist(&second_video);
        assert!(parsed.media_playlist(&first_video).is_none());
    }

    #[test]
    fn quality_comparisons_distinguish_playlist_kinds_and_preserve_shared_media() {
        use crate::media_element::{BufferedChunk, SegmentQualityContext};

        let video = MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::Variant, 1);
        let audio = MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::AudioTrack, 1);
        let chunk = BufferedChunk::new_for_test(0., 4., SegmentQualityContext::new(1., video));
        let shared_media = SegmentQualityContext::new(2., video);
        assert!(chunk.is_quality_equal(&shared_media));
        assert!(!chunk.is_worse_than(&shared_media));
        assert!(chunk.is_worse_than(&SegmentQualityContext::new(2., audio)));
    }

    use std::io::Cursor;

    #[test]
    fn rejects_audio_references_without_a_matching_audio_group() {
        for rendition in [
            "",
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"other\",NAME=\"Other\"\n",
            "#EXT-X-MEDIA:TYPE=VIDEO,GROUP-ID=\"english\",NAME=\"Video\"\n",
        ] {
            let playlist = format!(
                "#EXTM3U\n{rendition}#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO=\"english\"\nvideo.m3u8\n"
            );
            let result = MultivariantPlaylist::parse(
                Cursor::new(playlist),
                Url::new("https://example.com/master.m3u8".into()),
            );
            assert!(matches!(
                result,
                Err(MultivariantPlaylistParsingError::MissingAudioGroup(group_id))
                    if group_id == "english"
            ));
        }
    }

    #[test]
    fn accepts_audio_groups_declared_after_variants_with_or_without_uri() {
        for uri in ["", ",URI=\"audio.m3u8\""] {
            let playlist = format!(
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO=\"english\"\nvideo.m3u8\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"english\",NAME=\"English\"{uri}\n"
            );
            assert!(MultivariantPlaylist::parse(
                Cursor::new(playlist),
                Url::new("https://example.com/master.m3u8".into()),
            )
            .is_ok());
        }
    }

    #[test]
    fn keeps_invalid_utf8_in_variant_uri() {
        let playlist = b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvariant-\xff.m3u8\n";
        let parsed = MultivariantPlaylist::parse(
            Cursor::new(playlist),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();

        assert_eq!(parsed.variants.len(), 1);
        assert!(parsed.variants[0].url().get_ref().contains('�'));
    }

    #[test]
    fn substitutes_multivariant_variables_in_variant_and_media_tags() {
        let playlist = r#"#EXTM3U
#EXT-X-DEFINE:NAME="host",VALUE="https://cdn.example.com"
#EXT-X-DEFINE:NAME="group",VALUE="aud-main"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="{$group}",NAME="Track {$group}",URI="{$host}/audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="{$group}"
{$host}/video.m3u8
"#;
        let parsed = MultivariantPlaylist::parse(
            Cursor::new(playlist),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();

        assert_eq!(
            parsed.variant_from_idx(0).unwrap().url().get_ref(),
            "https://cdn.example.com/video.m3u8"
        );
        assert_eq!(parsed.audio_tracks()[0].name(), "Track aud-main");
        assert_eq!(
            parsed.audio_tracks()[0].medias()[0]
                .url()
                .unwrap()
                .get_ref(),
            "https://cdn.example.com/audio.m3u8"
        );
    }

    #[test]
    fn imports_multivariant_variables_and_query_params_in_media_playlists() {
        let multivariant = r#"#EXTM3U
#EXT-X-DEFINE:NAME="cdn",VALUE="https://cdn.example.com"
#EXT-X-STREAM-INF:BANDWIDTH=1000
video.m3u8
"#;
        let mut parsed = MultivariantPlaylist::parse(
            Cursor::new(multivariant),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();
        let media_playlist = r#"#EXTM3U
#EXT-X-DEFINE:IMPORT="cdn"
#EXT-X-DEFINE:QUERYPARAM="token"
#EXT-X-TARGETDURATION:4
#EXTINF:4,
{$cdn}/seg.ts?auth={$token}
"#;

        let media = parsed
            .update_media_playlist(
                &MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::Variant, 0),
                Cursor::new(media_playlist),
                Url::new("https://example.com/video.m3u8?token=abc%20123".to_owned()),
                None,
            )
            .unwrap();

        assert_eq!(
            media.segment_list().media()[0].url().get_ref(),
            "https://cdn.example.com/seg.ts?auth=abc 123"
        );
        assert!(parsed
            .media_playlist(&MediaPlaylistPermanentId::new(
                MediaPlaylistUrlLocation::Variant,
                0
            ))
            .is_some());
    }

    #[test]
    fn variables_do_not_implicitly_persist_across_media_playlist_reloads() {
        let multivariant = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000
video.m3u8
"#;
        let mut parsed = MultivariantPlaylist::parse(
            Cursor::new(multivariant),
            Url::new("https://example.com/master.m3u8".to_owned()),
        )
        .unwrap();
        let first_media = r#"#EXTM3U
#EXT-X-DEFINE:NAME="cdn",VALUE="https://cdn.example.com"
#EXT-X-TARGETDURATION:4
#EXTINF:4,
{$cdn}/seg-1.ts
"#;
        parsed
            .update_media_playlist(
                &MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::Variant, 0),
                Cursor::new(first_media),
                Url::new("https://example.com/video.m3u8".to_owned()),
                None,
            )
            .unwrap();

        let second_media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
{$cdn}/seg-2.ts
"#;
        let err = parsed
            .update_media_playlist(
                &MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::Variant, 0),
                Cursor::new(second_media),
                Url::new("https://example.com/video.m3u8".to_owned()),
                None,
            )
            .unwrap_err();

        assert!(matches!(
            err,
            MediaPlaylistUpdateError::ParsingError(MediaPlaylistParsingError::VariableDefinition(
                _
            ))
        ));
    }

    #[test]
    fn rejects_duplicate_singleton_tags() {
        let err = MultivariantPlaylist::parse(
            Cursor::new(
                r#"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-VERSION:7
#EXT-X-STREAM-INF:BANDWIDTH=1000
video.m3u8
"#,
            ),
            Url::new("https://example.com/master.m3u8".to_owned()),
        );

        assert!(matches!(
            err,
            Err(MultivariantPlaylistParsingError::DuplicateTag)
        ));
    }

    #[test]
    fn rejects_media_tags_in_multivariant_playlists() {
        let err = MultivariantPlaylist::parse(
            Cursor::new(
                r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000
video.m3u8
#EXT-X-TARGETDURATION:4
"#,
            ),
            Url::new("https://example.com/master.m3u8".to_owned()),
        );

        assert!(matches!(
            err,
            Err(MultivariantPlaylistParsingError::ConflictingPlaylistTagTypes)
        ));
    }

    #[test]
    fn rejects_missing_uri_lines_after_variant_when_followed_by_comment() {
        let err = MultivariantPlaylist::parse(
            Cursor::new(
                r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000
#comment
"#,
            ),
            Url::new("https://example.com/master.m3u8".to_owned()),
        );

        assert!(matches!(
            err,
            Err(MultivariantPlaylistParsingError::MissingUriLineAfterVariant)
        ));
    }
}
