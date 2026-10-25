// XXX TODO: After review of the file, private methods should be moved under the
// |                           private methods                           |
// comment

use crate::{
    bindings::{
        formatters::DIRECT_MEDIA_AUDIO_TRACK_ID, jsIsTypeSupported, MediaType, PlaylistNature,
    },
    parser::{
        AudioTrack, ExternalMediaInfo, MediaPlaylist, MediaPlaylistUpdateError, SegmentTimeInfo,
        TopLevelPlaylist, TopLevelPlaylistParsingError, VariantStream,
    },
    text,
    utils::{logger::*, url::Url},
};
use std::{collections::HashMap, io::BufRead};

use crate::parser::ByteRange;
pub(crate) use crate::parser::MediaPlaylistPermanentId;

/// Stores information about the current loaded Multivariant Playlist and its sub-playlists:
///   - Information on the Multivariant Playlist itself.
///   - On the current variant selected.
///   - Information on the different audio and video Media Playlists selected.
// TODO: Consider extracting PlaybackSelection with variant/rendition choices, track constraints,
// and support verification. Keep playlist data here. Only do this if moving the behavior together
// simplifies both PlaylistStore and the dispatcher, rather than adding an object for selected IDs.
pub(crate) struct PlaylistStore {
    /// Representation of the top-level playlist of the currently loaded HLS content.
    playlist: TopLevelPlaylist,

    /// Variant currently selected.
    /// `None` if the current content does not rely on variants (e.g. when playing
    /// Media Playlists directly)
    current_variant_id: Option<u32>,

    /// Chosen playlist for video.
    ///
    /// Also concerns playlist containing both audio and video.
    ///
    /// Set to `None` if no video playlist is chosen.
    current_video_id: Option<MediaPlaylistPermanentId>,

    /// Chosen playlist for audio.
    ///
    /// Set to `None` if no audio playlist is chosen.
    current_audio_id: Option<MediaPlaylistPermanentId>,

    /// `id` identifier for the currently-selected audio track. `None` if no audio track is
    /// explicitely selected.
    ///
    /// Note that unlike `current_video_id` and `current_audio_id`, this identifier is not linked to a
    /// Playlist but to the `id` of the track itself.
    fixed_audio_track: Option<u32>,
    fixed_subtitle_track: Option<u32>,
    subtitles_disabled: bool,
    current_text_id: Option<MediaPlaylistPermanentId>,

    /// If `true` a variant is being manually locked and as such, cannot change.
    is_variant_locked: bool,

    /// Cached result of codec support resolution for the chosen audio and video media.
    ///
    /// Changing the choice invalidates this result. Individual variant support results remain
    /// cached in `variant_support` and can be reused on the next check.
    selected_media_support_resolved: bool,

    /// Runtime support cache for variants in the current multivariant playlist.
    /// Key is the variant id, bool is `true` if supported, `false` if not.
    variant_support: HashMap<u32, bool>,

    /// Probe metadata inferred for currently known multivariant media playlists.
    multivariant_media_info: HashMap<MediaPlaylistPermanentId, ExternalMediaInfo>,

    /// Maximum positive, finite target duration observed per media type for this content.
    /// Retained when playlists are deselected or their cached contents are invalidated.
    max_observed_audio_target_duration: Option<f64>,
    max_observed_video_target_duration: Option<f64>,
    max_observed_text_target_duration: Option<f64>,
}

impl PlaylistStore {
    /// Parse the downloaded top-level playlist and create its store.
    pub(crate) fn try_new(data: &[u8], url: Url) -> Result<Self, PlaylistStoreError> {
        let playlist = TopLevelPlaylist::parse(data, url).map_err(PlaylistStoreError::Parsing)?;
        if matches!(&playlist, TopLevelPlaylist::Multivariant(pl) if pl.all_variants().is_empty()) {
            return Err(PlaylistStoreError::NoInitialVariant);
        }

        Ok(Self {
            playlist,
            current_variant_id: None,
            current_audio_id: None,
            current_video_id: None,
            fixed_audio_track: None,
            fixed_subtitle_track: None,
            subtitles_disabled: false,
            current_text_id: None,
            is_variant_locked: false,
            selected_media_support_resolved: false,
            variant_support: HashMap::new(),
            multivariant_media_info: HashMap::new(),
            max_observed_audio_target_duration: None,
            max_observed_video_target_duration: None,
            max_observed_text_target_duration: None,
        })
    }

    /// Returns a reference to the `Url` to the top-level Playlist stored by this
    /// `PlaylistStore`.
    /// XXX TODO: top_level_url?
    pub(crate) fn url(&self) -> &Url {
        self.playlist.url()
    }

    /// Returns which kind of Top-level Playlist we're relying on currently (either
    /// a MultiVariant Playlist or a direct Media Playlist one)
    pub(crate) fn playlist_kind(&self) -> crate::bindings::PlaylistType {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(_) => {
                crate::bindings::PlaylistType::MultivariantPlaylist
            }
            TopLevelPlaylist::DirectMedia(_) => crate::bindings::PlaylistType::MediaPlaylist,
        }
    }

    /// Iterate over selected, loaded playlists, with at most one entry per media type.
    ///
    /// The tuples are defined as such:
    ///   - first, the `MediaType`
    ///   - Second, a reference to the parsed `MediaPlaylist`
    pub(crate) fn current_media_playlists(
        &self,
    ) -> impl Iterator<Item = (MediaType, &MediaPlaylist)> + Clone {
        [MediaType::Audio, MediaType::Video, MediaType::Text]
            .into_iter()
            .filter_map(move |media_type| {
                self.current_media_playlist(media_type)
                    .map(|playlist| (media_type, playlist))
            })
    }

    // TODO: This one is ugly, remove
    pub(crate) fn direct_media_playlist(
        &self,
    ) -> Option<(&MediaPlaylistPermanentId, &MediaPlaylist)> {
        match &self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => Some((playlist.id(), playlist.playlist())),
            TopLevelPlaylist::Multivariant(_) => None,
        }
    }

    /// Returns `true` if the given `id` is linked to one of the playlists that are currently
    /// considered, regarless of if the playlist is actually loaded.
    pub(crate) fn is_current_media_playlist(&self, id: &MediaPlaylistPermanentId) -> bool {
        Some(id) == self.current_audio_id.as_ref()
            || Some(id) == self.current_video_id.as_ref()
            || Some(id) == self.current_text_id.as_ref()
    }

    /// Returns the `MediaType` associated to the given Media Playlist id, if this is a playlist
    /// currently considered (regardless of if it is loaded).
    ///
    /// Returns `None` when that playlist is not one of the currently-selected media playlists.
    pub(crate) fn selected_playlist_media_type(
        &self,
        id: &MediaPlaylistPermanentId,
    ) -> Option<MediaType> {
        if Some(id) == self.current_video_id.as_ref() {
            Some(MediaType::Video)
        } else if Some(id) == self.current_audio_id.as_ref() {
            Some(MediaType::Audio)
        } else if Some(id) == self.current_text_id.as_ref() {
            Some(MediaType::Text)
        } else {
            None
        }
    }

    /// Returns true if a MediaPlaylist specifically for the given `MediaType` has been
    /// selected, regardless if that playlist has been loaded or not.
    /// "Muxed" media playlists (with both audio and video) are considered Video.
    pub(crate) fn has_distinct_media_type(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.current_audio_id.is_some(),
            MediaType::Video => self.current_video_id.is_some(),
            MediaType::Text => self.current_text_id.is_some(),
        }
    }

    /// Initialize or update a `MediaPlaylist`, based on its `MediaPlaylistPermanentId`.
    pub(crate) fn update_media_playlist(
        &mut self,
        id: &MediaPlaylistPermanentId,
        media_playlist_data: impl BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        // First select a "sync playlist". We may need to e.g. synchronize the timelines of audio
        // and video media playlist. To do that, providing an alternative playlist's identity  will
        // be useful.
        let sync_playlist_id = if self.current_video_id.as_ref() == Some(id) {
            self.current_audio_id.filter(|curr_audio| curr_audio != id)
        } else if self.current_audio_id.as_ref() == Some(id) {
            self.current_video_id.filter(|curr_video| curr_video != id)
        } else {
            // Reuse the timeline of this media type when possible; otherwise use the other type.
            let media_type = self
                .multivariant_media_info
                .get(id)
                .map(|info| info.media_type)
                .or_else(|| match &self.playlist {
                    TopLevelPlaylist::Multivariant(playlist) => playlist.playlist_media_type(id),
                    TopLevelPlaylist::DirectMedia(_) => None,
                });
            let references = match media_type {
                Some(MediaType::Audio) => [self.current_audio_id, self.current_video_id],
                Some(MediaType::Video | MediaType::Text) | None => {
                    [self.current_video_id, self.current_audio_id]
                }
            };
            references
                .into_iter()
                .flatten()
                .find(|reference| reference != id && self.media_playlist(reference).is_some())
        };
        let media_type = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.playlist_media_type(id),
            TopLevelPlaylist::DirectMedia(playlist) => {
                playlist.external_media_info().map(|info| info.media_type)
            }
        }
        .or_else(|| self.selected_playlist_media_type(id));
        let media_playlist = match &mut self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => {
                playlist.update_media_playlist(id, media_playlist_data, url, sync_playlist_id)
            }
            TopLevelPlaylist::DirectMedia(playlist) => {
                playlist.update_media_playlist(id, media_playlist_data, url, sync_playlist_id)
            }
        }?;
        let duration = media_playlist.target_duration();

        if let Some(media_type) = media_type {
            if duration.is_finite() && duration > 0. {
                let maximum = match media_type {
                    MediaType::Audio => &mut self.max_observed_audio_target_duration,
                    MediaType::Video => &mut self.max_observed_video_target_duration,
                    MediaType::Text => &mut self.max_observed_text_target_duration,
                };
                *maximum = Some(maximum.map_or(duration, |known| known.max(duration)));
            }
        }
        Ok(media_playlist)
    }

    /// Get the codec string of the current playlist associated to this media type if known.
    pub(crate) fn current_codec(&self, media_type: MediaType) -> Option<String> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(_) => {
                if !self.has_distinct_media_type(media_type) {
                    return None;
                }
                self.curr_multivariant_media_info(media_type)
                    .map(|info| info.codec.clone())
                    .or_else(|| self.current_variant()?.codecs(media_type))
            }
            TopLevelPlaylist::DirectMedia(playlist) => playlist
                .external_media_info()
                .filter(|info| info.media_type == media_type)
                .map(|info| info.codec.clone()),
        }
    }

    /// Get the mime-type of the current playlist associated to this media type if known.
    pub(crate) fn current_mime_type(&self, media_type: MediaType) -> Option<String> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(_) => {
                if !self.has_distinct_media_type(media_type) {
                    return None;
                }
                self.curr_multivariant_media_info(media_type)
                    .map(|info| info.mime_type.clone())
                    .or_else(|| {
                        self.current_media_playlist(media_type)
                            .and_then(|playlist| playlist.mime_type(media_type))
                            .map(str::to_string)
                    })
            }
            TopLevelPlaylist::DirectMedia(playlist) => playlist
                .external_media_info()
                .filter(|info| info.media_type == media_type)
                .map(|info| info.mime_type.clone()),
        }
    }

    /// Returns the next startup action required before playback can begin.
    ///
    /// This first ensures that enough stream metadata is known for the currently selected startup
    /// path. Once that metadata is complete, it resolves whether the selected startup streams are
    /// actually supported by the current environment.
    pub(crate) fn startup_status(
        &mut self,
        wanted_position: f64,
    ) -> Result<StartupStatus, PlaylistStoreError> {
        match &self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => {
                if playlist.external_media_info().is_none() {
                    return self
                        .next_direct_media_probe(wanted_position)
                        .map(|probe_segment| {
                            StartupStatus::NeedsProbes(vec![(probe_segment, None)])
                        })
                        .ok_or(PlaylistStoreError::NoProbeSegment);
                }
            }
            TopLevelPlaylist::Multivariant(_) => {
                if self.current_variant_id.is_none() {
                    if self.compatible_variants().next().is_none() {
                        return Err(PlaylistStoreError::NoSupportedVariant);
                    }
                    return Ok(StartupStatus::VariantSelectionNeeded);
                }
                if [MediaType::Audio, MediaType::Video]
                    .into_iter()
                    .any(|media_type| {
                        self.has_distinct_media_type(media_type)
                            && !self.has_loaded_media_playlist(media_type)
                    })
                {
                    return Ok(StartupStatus::AwaitingPlaylists);
                }
                let probe_requests = [MediaType::Audio, MediaType::Video]
                    .into_iter()
                    .filter(|media_type| {
                        self.has_distinct_media_type(*media_type)
                            && (self.current_codec(*media_type).is_none()
                                || self.current_mime_type(*media_type).is_none())
                    })
                    .map(|media_type| {
                        self.next_media_playlist_probe(media_type, wanted_position)
                            .map(|probe_segment| (probe_segment, Some(media_type)))
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(PlaylistStoreError::NoProbeSegment)?;
                if !probe_requests.is_empty() {
                    return Ok(StartupStatus::NeedsProbes(probe_requests));
                }
            }
        }

        match &self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => {
                let media_info = playlist.external_media_info().unwrap();
                if media_info.media_type == MediaType::Text {
                    return Err(PlaylistStoreError::TextOnlyMainContent);
                }
                match self.current_media_support(media_info.media_type) {
                    Some(true) => Ok(StartupStatus::Ready),
                    Some(false) => Err(PlaylistStoreError::UnsupportedStartupStream),
                    None => Ok(StartupStatus::AwaitingSupportCheck),
                }
            }
            TopLevelPlaylist::Multivariant(_) => {
                match self.resolve_multivariant_audio_video_support()? {
                    MultivariantStartupStatus::Ready => Ok(StartupStatus::Ready),
                    MultivariantStartupStatus::AwaitingSupportCheck => {
                        Ok(StartupStatus::AwaitingSupportCheck)
                    }
                    MultivariantStartupStatus::VariantSelectionNeeded => {
                        Ok(StartupStatus::VariantSelectionNeeded)
                    }
                }
            }
        }
    }

    /// Communicate back precise information probed from a probe segment alongside the `MediaType`
    /// concerned.
    /// TODO: Current way of doing this might lead to erasure when the playlist is considered
    /// "stale". It's not really a bug, more an inefficiency, especially for variant switching which
    /// may happen often. Though this would be only for poorly-packaged contents.
    pub(crate) fn set_external_media_info(
        &mut self,
        media_info: ExternalMediaInfo,
        media_type: MediaType,
    ) -> ExternalMediaInfoUpdate {
        let mut changes = ExternalMediaInfoUpdate::default();
        let changed = match &mut self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => {
                let changed = playlist.external_media_info() != Some(&media_info);
                let direct_media_id = *playlist.id();
                playlist.set_external_media_info(media_info);
                // A direct playlist's type is unknown until its first segment is probed.
                let duration = playlist.playlist().target_duration();
                if duration.is_finite() && duration > 0. {
                    let maximum = match media_type {
                        MediaType::Audio => &mut self.max_observed_audio_target_duration,
                        MediaType::Video => &mut self.max_observed_video_target_duration,
                        MediaType::Text => &mut self.max_observed_text_target_duration,
                    };
                    *maximum = Some(maximum.map_or(duration, |known| known.max(duration)));
                }
                let (audio_id, video_id, text_id) = match media_type {
                    MediaType::Audio => (Some(direct_media_id), None, None),
                    MediaType::Video => (None, Some(direct_media_id), None),
                    MediaType::Text => (None, None, Some(direct_media_id)),
                };
                if self.current_audio_id != audio_id {
                    self.current_audio_id = audio_id;
                    changes.changed_playlists.push(MediaType::Audio);
                }
                if self.current_video_id != video_id {
                    self.current_video_id = video_id;
                    changes.changed_playlists.push(MediaType::Video);
                }
                if self.current_text_id != text_id {
                    self.current_text_id = text_id;
                    changes.changed_playlists.push(MediaType::Text);
                }
                changed
            }
            TopLevelPlaylist::Multivariant(_) => {
                let Some(id) = self.current_media_playlist_id(media_type).copied() else {
                    return changes;
                };
                let changed = self.multivariant_media_info.get(&id) != Some(&media_info);
                self.multivariant_media_info.insert(id, media_info);
                changed
            }
        };
        // resolve_multivariant_audio_video_support currently caches only audio/video support.
        if changed && matches!(media_type, MediaType::Audio | MediaType::Video) {
            self.clear_variant_supports();
        }
        changes
    }

    /// Look up a variant independently of the current track choice or support filtering.
    pub(crate) fn variant(&self, variant_id: u32) -> Option<&VariantStream> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.variant(variant_id),
            TopLevelPlaylist::DirectMedia(_) => None,
        }
    }

    /// Iterate over variants in the current multivariant playlist, excluding known unsupported ones.
    pub(crate) fn available_variants(&self) -> impl Iterator<Item = &VariantStream> {
        let variants = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.all_variants(),
            TopLevelPlaylist::DirectMedia(_) => &[],
        };
        variants
            .iter()
            .filter(|variant| self.variant_support(variant.id()) != Some(false))
    }

    /// Iterate over variants compatible with the current track choices, excluding known
    /// unsupported variants. A variant lock does not change this pool.
    pub(crate) fn compatible_variants(&self) -> impl Iterator<Item = &VariantStream> {
        // XXX TODO: To review heavily, the diff to main is large in that one
        let playlist = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => Some(playlist),
            TopLevelPlaylist::DirectMedia(_) => None,
        };
        let audio_id = self
            .fixed_audio_track
            .or_else(|| self.current_audio_track_id());
        let subtitle_id = self
            .fixed_subtitle_track
            .filter(|_| !self.subtitles_disabled);
        self.available_variants().filter(move |variant| {
            let Some(playlist) = playlist else {
                return false;
            };
            if let Some(track_id) = audio_id {
                if !playlist
                    .audio_renditions_for(variant)
                    .any(|(track, _)| track.id() == track_id)
                {
                    return false;
                }
            }
            if let Some(track_id) = subtitle_id {
                if !playlist
                    .subtitle_renditions_for(variant)
                    .any(|(track, _)| track.id() == track_id)
                {
                    return false;
                }
            }
            true
        })
    }

    /// Maximum positive, finite target duration observed for this media type, in seconds.
    /// Returns `None` until a playlist of this type has been identified with a valid duration.
    /// Deselection and cache invalidation do not discard previously observed durations.
    pub(crate) fn max_observed_target_duration(&self, media_type: MediaType) -> Option<f64> {
        match media_type {
            MediaType::Audio => self.max_observed_audio_target_duration,
            MediaType::Video => self.max_observed_video_target_duration,
            MediaType::Text => self.max_observed_text_target_duration,
        }
    }

    /// Returns `true` if the currently selected media playlists use a timeline
    /// based on `EXT-X-PROGRAM-DATE-TIME`.
    pub(crate) fn uses_program_date_time(&self) -> bool {
        self.current_media_playlists()
            .any(|(_, playlist)| playlist.uses_program_date_time())
    }

    /// Returns the `id` of the variant currently considered.
    ///
    /// Returns `None` in cases where either no variant is selected or just in direct
    /// media playlist cases - where there's no variant.
    pub(crate) fn current_variant_id(&self) -> Option<u32> {
        // TODO: Don't remember why we don't just self.current_variant_id.
        // To check the semantic diff and which one is actually wanted here.
        self.current_variant().map(|v| v.id())
    }

    /// Force a given variant and prevent it from changing, by communicating its `id`.
    /// This can also make the initial variant selection.
    ///
    /// To be able to change again the variant, you can call `lock_variant` again or
    /// you can call the `unlock_variant` method.
    ///
    /// Returns whether the variant was newly locked, was already locked, or could not be found.
    pub(crate) fn lock_variant(&mut self, variant_id: u32) -> LockVariantResponse {
        let is_compatible = self
            .compatible_variants()
            .any(|variant| variant.id() == variant_id);

        if !is_compatible {
            return LockVariantResponse::NoVariantWithId;
        }

        if self.is_variant_locked && self.current_variant_id == Some(variant_id) {
            return LockVariantResponse::AlreadyLocked;
        }
        let VariantUpdate {
            changed_playlists,
            tracks,
            variant_changed,
            locked_variant_change: _, // The successful lock is reported by VariantLocked.
        } = self.set_variant(variant_id);
        self.is_variant_locked = true;
        LockVariantResponse::VariantLocked {
            changed_media_types: changed_playlists,
            tracks,
            variant_changed,
        }
    }

    /// Disable a variant lock, previously created through the `lock_variant` method, to
    /// let adaptive streaming choose the right one instead.
    /// Return true if a lock was removed.
    pub(crate) fn unlock_variant(&mut self) -> bool {
        if self.is_variant_locked {
            self.is_variant_locked = false;
            true
        } else {
            false
        }
    }

    /// Returns `true` if a variant is currently locked, preventing adaptive streaming
    /// from choosing the more adapted one. Such lock can be enabled through the
    /// lock_variant` method.
    pub(crate) fn is_variant_locked(&self) -> bool {
        self.is_variant_locked
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
    ///
    /// Both are probably an error as a `MediaPlaylistPermanentId` should always identify a
    /// `MediaPlaylist`.
    pub(crate) fn media_playlist_url(&self, wanted_id: &MediaPlaylistPermanentId) -> Option<&Url> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.media_playlist_url(wanted_id),
            TopLevelPlaylist::DirectMedia(playlist) => playlist.media_playlist_url(wanted_id),
        }
    }

    /// Returns the `MediaPlaylistPermanentId` of the MediaPlaylist linked to the media
    /// of the given `MediaType`.
    ///
    /// Returns `None` if there's no choosen MediaPlaylist for the given MediaType.
    pub(crate) fn current_media_playlist_id(
        &self,
        media_type: MediaType,
    ) -> Option<&MediaPlaylistPermanentId> {
        match media_type {
            MediaType::Video => self.current_video_id.as_ref(),
            MediaType::Audio => self.current_audio_id.as_ref(),
            MediaType::Text => self.current_text_id.as_ref(),
        }
    }

    /// Returns `true` if the currently selected media playlist for the given `MediaType`
    /// is loaded.
    pub(crate) fn has_loaded_media_playlist(&self, media_type: MediaType) -> bool {
        self.current_media_playlist(media_type).is_some()
    }

    /// Returns `true` if the currently loaded media playlist for the given `MediaType`
    /// contains the requested sequence number.
    pub(crate) fn loaded_playlist_contains_sequence(
        &self,
        media_type: MediaType,
        sequence_number: u32,
    ) -> bool {
        self.current_media_playlist(media_type)
            .is_some_and(|playlist| playlist.contains_sequence(sequence_number))
    }

    /// Resolve a playlist ID for this variant using the current track choices.
    /// Ok(None) means no separate playlist is needed for this media type.
    /// Err means a fixed track choice is incompatible with the variant.
    /// Does not change the selected variant or tracks.
    pub(crate) fn resolve_playlist_id_for_variant(
        &self,
        variant: &VariantStream,
        media_type: MediaType,
    ) -> Result<Option<MediaPlaylistPermanentId>, IncompatibleTrack> {
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return Ok(None);
        };
        match media_type {
            MediaType::Audio => {
                let track_id = self.audio_track_id_for_variant(variant)?;
                let audio_id = playlist.audio_media_playlist_id_for(variant, track_id);
                let video_id = playlist.video_media_playlist_id_for(variant);
                let (audio_id, _) = Self::normalize_current_media_ids(audio_id, video_id);
                Ok(audio_id)
            }
            MediaType::Video => Ok(playlist.video_media_playlist_id_for(variant)),
            MediaType::Text => match self.subtitle_track_id_for_variant(variant)? {
                Some(track_id) => Ok(playlist.subtitle_media_playlist_id_for(variant, track_id)),
                None => Ok(None),
            },
        }
    }

    /// Return the loaded playlist for this ID, even if its variant isn't currently selected.
    pub(crate) fn media_playlist(&self, id: &MediaPlaylistPermanentId) -> Option<&MediaPlaylist> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.media_playlist(id),
            TopLevelPlaylist::DirectMedia(playlist) => playlist.media_playlist(id),
        }
    }

    /// Gives an indication of which kind of playlist it is: VoD/live/Event?
    pub(crate) fn playlist_type(&self) -> PlaylistNature {
        let media_playlists = self.current_media_playlists();
        media_playlists.fold(PlaylistNature::Unknown, |acc, p| match acc {
            PlaylistNature::Live => PlaylistNature::Live,
            PlaylistNature::Event => {
                if p.1.playlist_type() == PlaylistNature::Live {
                    PlaylistNature::Live
                } else {
                    PlaylistNature::Event
                }
            }
            PlaylistNature::VoD => match p.1.playlist_type() {
                PlaylistNature::Live => PlaylistNature::Live,
                PlaylistNature::Event => PlaylistNature::Event,
                _ => PlaylistNature::VoD,
            },
            _ => p.1.playlist_type(),
        })
    }

    /// Returns `true` once the currently selected content cannot publish new
    /// segments anymore.
    pub(crate) fn is_finalized(&self) -> bool {
        match &self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => !playlist.playlist().may_be_refreshed(),
            TopLevelPlaylist::Multivariant(_) => {
                let mut selected_types = [MediaType::Audio, MediaType::Video, MediaType::Text]
                    .into_iter()
                    .filter(|media_type| self.has_distinct_media_type(*media_type))
                    .peekable();
                selected_types.peek().is_some()
                    && selected_types.all(|media_type| {
                        self.current_media_playlist(media_type)
                            .is_some_and(|playlist| !playlist.may_be_refreshed())
                    })
            }
        }
    }

    /// Returns currently estimated start time in seconds at which to begin playing the content.
    ///
    /// This value may change depending on the chosen MediaPlaylist that are also loaded.
    /// TODO: Should this playback policy move to the dispatcher?
    pub(crate) fn expected_start_time(&self) -> f64 {
        let playback_playlists = self
            .current_media_playlists()
            .filter(|(media_type, _)| matches!(media_type, MediaType::Audio | MediaType::Video));

        let mut has_playback_playlist = false;
        let mut all_live = true;
        let mut live_playback_start: Option<f64> = None;
        let mut playback_window_start: Option<f64> = None;

        for (_, playlist) in playback_playlists {
            has_playback_playlist = true;
            if let Some(wanted_start) = playlist.wanted_start() {
                return wanted_start;
            }
            all_live &= playlist.is_live();
            if let Some(end) = playlist.ending() {
                let start = (end - (3. * playlist.target_duration())).max(0.);
                live_playback_start =
                    Some(live_playback_start.map_or(start, |known| known.min(start)));
            }
            if let Some(beginning) = playlist.beginning() {
                playback_window_start =
                    Some(playback_window_start.map_or(beginning, |known| known.max(beginning)));
            }
        }

        if !has_playback_playlist {
            if let Some((_, playlist)) = self.direct_media_playlist() {
                if let Some(wanted_start) = playlist.wanted_start() {
                    return wanted_start;
                }
                if playlist.is_live() {
                    return playlist
                        .ending()
                        .map(|end| (end - (3. * playlist.target_duration())).max(0.))
                        .unwrap_or(0.);
                }
                return playlist.beginning().unwrap_or(0.);
            }
            return 0.;
        }

        if all_live {
            live_playback_start.unwrap_or(0.)
        } else {
            playback_window_start.unwrap_or(0.)
        }
    }

    /// Returns the `id` of the `AudioTrack` object which is associated to the current audio
    /// media loaded.
    ///
    /// Returns `None` if no current audio media is known currently or if no `AudioTrack` is
    /// linked to it.
    pub(crate) fn current_audio_track_id(&self) -> Option<u32> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist
                .audio_track_for_media_id(self.current_audio_id.as_ref()?)
                .map(|p| p.id()),
            TopLevelPlaylist::DirectMedia(_) => None,
        }
    }

    /// Returns the `id` of the `SubtitleTrack` associated with the current subtitle playlist.
    ///
    /// Returns `None` if no subtitle playlist is selected or no track is linked to it.
    pub(crate) fn current_subtitle_track_id(&self) -> Option<u32> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist
                .subtitle_track_for_media_id(self.current_text_id.as_ref()?)
                .map(|track| track.id()),
            TopLevelPlaylist::DirectMedia(_) => None,
        }
    }

    /// Return the audio track ID to use for this variant.
    /// Return Ok(None) to use the variant's own audio, if present.
    /// Return Err if the fixed audio track is unavailable on this variant.
    fn audio_track_id_for_variant(
        &self,
        variant: &VariantStream,
    ) -> Result<Option<u32>, IncompatibleTrack> {
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return Ok(None);
        };
        if let Some(track_id) = self.fixed_audio_track {
            if playlist
                .audio_renditions_for(variant)
                .any(|(track, _)| track.id() == track_id)
            {
                return Ok(Some(track_id));
            }
            return Err(IncompatibleTrack);
        }
        let mut renditions = playlist.audio_renditions_for(variant);
        let Some((first_track, first_media)) = renditions.next() else {
            return Ok(None);
        };
        if first_media.is_default() {
            return Ok(Some(first_track.id()));
        }
        Ok(Some(
            renditions
                .find(|(_, media)| media.is_default())
                .map_or(first_track.id(), |(track, _)| track.id()),
        ))
    }

    /// Returns the `id` of the `AudioTrack` object explicitely selected through the
    /// `set_audio_track` API.
    ///
    /// Returns `None` if no audio track is currently selected.
    pub(crate) fn fixed_audio_track_id(&self) -> Option<u32> {
        self.fixed_audio_track
    }

    pub(crate) fn fixed_subtitle_track_id(&self) -> Option<u32> {
        self.fixed_subtitle_track
    }

    /// Choose subtitles and report the resulting selection changes.
    pub(crate) fn set_subtitle_track(
        &mut self,
        track_id: Option<u32>,
    ) -> Result<SubtitleTrackUpdate, TrackSelectionError> {
        if track_id.is_some_and(|id| !self.subtitle_tracks().iter().any(|track| track.id() == id)) {
            return Err(TrackSelectionError::UnknownTrack);
        }
        if let (Some(track_id), TopLevelPlaylist::Multivariant(playlist)) =
            (track_id, &self.playlist)
        {
            let audio_id = self
                .fixed_audio_track
                .or_else(|| self.current_audio_track_id());
            let audio_variants = audio_id.map(|id| playlist.variants_for_audio(id));
            if !playlist
                .variants_for_subtitle(track_id)
                .iter()
                .any(|variant| {
                    self.variant_support(variant.id()) != Some(false)
                        && audio_variants.as_ref().is_none_or(|variants| {
                            variants
                                .iter()
                                .any(|candidate| candidate.id() == variant.id())
                        })
                })
            {
                return Err(TrackSelectionError::NoCompatibleVariant);
            }
        }
        let mut changes = SubtitleTrackUpdate::default();
        self.fixed_subtitle_track = track_id;
        self.subtitles_disabled = false;
        match self.resolve_subtitle_selection() {
            Ok(playlist_id) => {
                (changes.playlist_changed, changes.track_change) =
                    self.update_text_playlist_id(playlist_id);
            }
            Err(IncompatibleTrack) => {
                // Keep the selected playlist until the compatible variant is applied.
                if self.is_variant_locked {
                    self.is_variant_locked = false;
                    changes.variant_lock_removed = true;
                }
                changes.needs_variant_selection = true;
            }
        }
        Ok(changes)
    }

    pub(crate) fn disable_subtitle_track(&mut self) -> SubtitleDisableUpdate {
        self.subtitles_disabled = true;
        let (playlist_changed, track_change) = self.update_text_playlist_id(None);
        SubtitleDisableUpdate {
            playlist_changed,
            track_change,
        }
    }

    /// Update the subtitle playlist and report whether its playlist ID and track ID changed.
    fn update_text_playlist_id(
        &mut self,
        playlist_id: Option<MediaPlaylistPermanentId>,
    ) -> (bool, TrackChange) {
        if self.current_text_id == playlist_id {
            return (false, TrackChange::Unchanged);
        }
        let previous_track = self.current_subtitle_track_id();
        self.current_text_id = playlist_id;
        let track = self.current_subtitle_track_id();
        let track_change = if previous_track != track {
            TrackChange::Changed(track)
        } else {
            TrackChange::Unchanged
        };
        (true, track_change)
    }

    /// Resolve the current subtitle choice without changing the selected playlist.
    /// Return an error if the fixed track choice requires a different variant.
    fn resolve_subtitle_selection(
        &self,
    ) -> Result<Option<MediaPlaylistPermanentId>, IncompatibleTrack> {
        let Some(variant) = self.current_variant() else {
            return Ok(None);
        };
        self.resolve_playlist_id_for_variant(variant, MediaType::Text)
    }

    /// Resolve subtitles within this variant; reject an unavailable fixed choice.
    fn subtitle_track_id_for_variant(
        &self,
        variant: &VariantStream,
    ) -> Result<Option<u32>, IncompatibleTrack> {
        if self.subtitles_disabled {
            return Ok(None);
        }
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return Ok(None);
        };
        if let Some(track_id) = self.fixed_subtitle_track {
            if playlist
                .subtitle_renditions_for(variant)
                .any(|(track, _)| track.id() == track_id)
            {
                return Ok(Some(track_id));
            }
            return Err(IncompatibleTrack);
        }
        let audio_language = self
            .audio_track_id_for_variant(variant)?
            .and_then(|id| self.audio_tracks().iter().find(|track| track.id() == id))
            .and_then(|track| track.language());
        Ok(playlist
            .subtitle_renditions_for(variant)
            .filter(|(_, media)| {
                media.is_default()
                    || (media.is_forced()
                        && media.is_autoselect()
                        && media.language().zip(audio_language).is_some_and(
                            |(candidate, language)| candidate.eq_ignore_ascii_case(language),
                        ))
            })
            .max_by_key(|(_, media)| {
                (
                    media.is_default(),
                    media
                        .language()
                        .zip(audio_language)
                        .is_some_and(|(candidate, language)| {
                            candidate.eq_ignore_ascii_case(language)
                        }),
                )
            })
            .map(|(track, _)| track.id()))
    }

    /// Returns the list of available audio tracks on the current content.
    pub(crate) fn audio_tracks(&self) -> &[AudioTrack] {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.audio_tracks(),
            TopLevelPlaylist::DirectMedia(_) => &[],
        }
    }

    pub(crate) fn subtitle_tracks(&self) -> &[crate::parser::SubtitleTrack] {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.subtitle_tracks(),
            TopLevelPlaylist::DirectMedia(_) => &[],
        }
    }

    /// Explicitely select an `AudioTrack` based on its `id` property or disable the explicit
    /// selection of one (by giving `None` as argument).
    ///
    /// Rejects a runtime selection with no linked variant without changing any state.
    /// TODO: The name of this method is poor, as it doesn't communicate well the place that it
    /// "locks-in" an audio track, e.g. unlike `set_variant`
    pub(crate) fn set_audio_track(
        &mut self,
        track_id: Option<u32>,
    ) -> Result<AudioTrackUpdate, TrackSelectionError> {
        if let Some(id) = track_id {
            let exists = match &self.playlist {
                TopLevelPlaylist::Multivariant(playlist) => {
                    playlist.audio_tracks().iter().any(|track| track.id() == id)
                }
                TopLevelPlaylist::DirectMedia(_) => {
                    id == DIRECT_MEDIA_AUDIO_TRACK_ID
                        && self.has_distinct_media_type(MediaType::Audio)
                }
            };
            if !exists {
                return Err(TrackSelectionError::UnknownTrack);
            }
        }
        let mut changes = AudioTrackUpdate::default();
        let previous_track = self.effective_audio_track_id();
        self.set_audio_track_inner(track_id, &mut changes)?;
        let track = self.effective_audio_track_id();
        if previous_track != track {
            // XXX TODO: Why wasn't it already the case if `self.effective_audio_track_id` changed?
            changes.tracks.audio = TrackChange::Changed(track);
        }
        if !changes.needs_variant_selection {
            let (playlist_changed, track_change) = match self.resolve_subtitle_selection() {
                Ok(playlist_id) => self.update_text_playlist_id(playlist_id),
                Err(IncompatibleTrack) => self.update_text_playlist_id(None),
            };
            if playlist_changed {
                changes.subtitle_playlist_changed = true;
            }
            changes.tracks.subtitles = track_change;
        }
        Ok(changes)
    }

    fn set_audio_track_inner(
        &mut self,
        track_id: Option<u32>,
        changes: &mut AudioTrackUpdate,
    ) -> Result<(), TrackSelectionError> {
        if let (TopLevelPlaylist::Multivariant(playlist), Some(track_id)) =
            (&self.playlist, track_id)
        {
            // Support depends on the chosen track and will be checked again after a change.
            // Here, only reject tracks for which the manifest provides no candidate at all.
            let subtitle_variants = self
                .fixed_subtitle_track
                .filter(|_| !self.subtitles_disabled)
                .map(|id| playlist.variants_for_subtitle(id));
            if !playlist.variants_for_audio(track_id).iter().any(|variant| {
                subtitle_variants.as_ref().is_none_or(|variants| {
                    variants
                        .iter()
                        .any(|candidate| candidate.id() == variant.id())
                })
            }) {
                return Err(TrackSelectionError::NoCompatibleVariant);
            }
        }

        self.fixed_audio_track = track_id;

        if let Some(variant) = self.current_variant() {
            let resolved_track = match self.audio_track_id_for_variant(variant) {
                Ok(track_id) => track_id,
                Err(IncompatibleTrack) => {
                    if self.is_variant_locked {
                        self.is_variant_locked = false;
                        changes.variant_lock_removed = true;
                    }
                    self.clear_variant_supports();
                    changes.needs_variant_selection = true;
                    return Ok(());
                }
            };
            let raw_new_audio_id = match &self.playlist {
                TopLevelPlaylist::Multivariant(playlist) => {
                    playlist.audio_media_playlist_id_for(variant, resolved_track)
                }
                TopLevelPlaylist::DirectMedia(_) => None,
            };
            let (new_audio_id, _) =
                Self::normalize_current_media_ids(raw_new_audio_id, self.current_video_id);

            if new_audio_id != self.current_audio_id {
                self.current_audio_id = new_audio_id;
                changes.audio_playlist_changed = true;
                self.clear_variant_supports();
            }
        }
        Ok(())
    }

    /// Apply a specific variant selection and return the media types whose selected playlist
    /// changed as a consequence.
    pub(crate) fn set_variant(&mut self, variant_id: u32) -> VariantUpdate {
        let mut changes = VariantUpdate::default();
        self.set_curr_variant_and_media_id(variant_id, &mut changes);
        let (playlist_changed, track_change) = match self.resolve_subtitle_selection() {
            Ok(playlist_id) => self.update_text_playlist_id(playlist_id),
            Err(IncompatibleTrack) => self.update_text_playlist_id(None),
        };
        if playlist_changed {
            changes.changed_playlists.push(MediaType::Text);
        }
        changes.tracks.subtitles = track_change;
        changes
    }

    // +---------------------------------------------------------------------+
    // |                           private methods                           |
    // +---------------------------------------------------------------------+

    /// Returns a reference to the `VariantStream` currently selected.
    fn current_variant(&self) -> Option<&VariantStream> {
        match (&self.playlist, self.current_variant_id) {
            (TopLevelPlaylist::Multivariant(playlist), Some(current_variant_id)) => {
                playlist.variant(current_variant_id)
            }
            _ => None,
        }
    }

    /// Returns `true` if the current playlist linked to the given `MediaType` has been loaded.
    ///
    /// Returns `false` either if it has not been loaded yet or if there's no media playlist for
    /// that `MediaType`.
    /// You can call `has_distinct_media_type` to know if there's a playlist to load for a given
    /// `MediaType`.
    fn is_media_playlist_ready(&self, media_type: MediaType) -> bool {
        self.current_media_playlist(media_type).is_some()
    }

    /// Returns probe segment metadata for a direct Media Playlist associated to
    /// the `wanted_position` if it exists.
    fn next_direct_media_probe(&self, wanted_position: f64) -> Option<ProbeSegmentMetadata> {
        let playlist = match &self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => playlist.playlist(),
            TopLevelPlaylist::Multivariant(_) => return None,
        };
        Self::probe_for_media_playlist(playlist, wanted_position)
    }

    /// Returns probe segment metadata for a non-direct Media Playlist associated to
    /// the `wanted_position` if it exists.
    fn next_media_playlist_probe(
        &self,
        media_type: MediaType,
        wanted_position: f64,
    ) -> Option<ProbeSegmentMetadata> {
        let playlist = self.current_media_playlist(media_type)?;
        Self::probe_for_media_playlist(playlist, wanted_position)
    }

    /// Get probe segment for the given media playlist and preferred position.
    fn probe_for_media_playlist(
        playlist: &MediaPlaylist,
        wanted_position: f64,
    ) -> Option<ProbeSegmentMetadata> {
        // TODO: Choose probes through normal segment selection so buffered coverage is
        // considered too. The first-segment fallback below may be outside the wanted window.
        let media_segment = playlist
            .segment_list()
            .segment_from_pos(wanted_position)
            .or_else(|| playlist.segment_list().media().first())?;
        if let Some(init_segment) = playlist.segment_list().init_for(media_segment) {
            Some(ProbeSegmentMetadata {
                url: init_segment.url().clone(),
                byte_range: init_segment.byte_range().cloned(),
                context: ProbeSegmentContext::Init {
                    id: init_segment.id(),
                },
            })
        } else {
            Some(ProbeSegmentMetadata {
                url: media_segment.url().clone(),
                byte_range: media_segment.byte_range().cloned(),
                context: ProbeSegmentContext::Media {
                    sequence: media_segment.sequence(),
                    discontinuity_sequence: media_segment.discontinuity_sequence(),
                    time_info: media_segment.time_info().clone(),
                },
            })
        }
    }

    /// Internally update the current variant chosen as well as its corresponding other media.
    /// XXX TODO: inline method?
    fn set_curr_variant_and_media_id(&mut self, variant_id: u32, changes: &mut VariantUpdate) {
        let playlist = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist,
            TopLevelPlaylist::DirectMedia(_) => return,
        };
        let Some(variant) = playlist.variant(variant_id) else {
            return;
        };
        let previous_track = self.effective_audio_track_id();
        if self.current_variant_id != Some(variant_id) {
            self.current_variant_id = Some(variant_id);
            changes.variant_changed = true;
            if self.is_variant_locked {
                // XXX TODO: Do that mean a `set_variant` is stronger than a lock? Is that intended?
                changes.locked_variant_change = Some(variant_id);
            }
        }
        let (current_audio_id, current_video_id) = Self::normalize_current_media_ids(
            match self.audio_track_id_for_variant(variant) {
                Ok(track_id) => playlist.audio_media_playlist_id_for(variant, track_id),
                Err(IncompatibleTrack) => None,
            },
            playlist.video_media_playlist_id_for(variant),
        );
        if self.current_audio_id != current_audio_id {
            self.current_audio_id = current_audio_id;
            changes.changed_playlists.push(MediaType::Audio);
        }
        if self.current_video_id != current_video_id {
            self.current_video_id = current_video_id;
            changes.changed_playlists.push(MediaType::Video);
        }
        let track = self.effective_audio_track_id();
        if previous_track != track {
            changes.tracks.audio = TrackChange::Changed(track);
        }
        self.selected_media_support_resolved = false;
    }

    fn clear_variant_supports(&mut self) {
        let TopLevelPlaylist::Multivariant(_) = &self.playlist else {
            return;
        };
        // XXX TODO: I just noticed that `compatible_variants` depends on this. Why are we clearing
        // the support carelessly when calling `clear_variant_supports`?
        // To compare with last release, I think assumptions have been poorly understood somewhere
        self.variant_support.clear();
        self.selected_media_support_resolved = false;
    }

    fn variant_support(&self, variant_id: u32) -> Option<bool> {
        self.variant_support.get(&variant_id).copied()
    }

    fn set_variant_support(&mut self, variant_id: u32, supported: bool) {
        self.variant_support.insert(variant_id, supported);
    }

    // TODO: This method is kind of ugly. Better handle muxed Audio/Video identifiers?
    fn normalize_current_media_ids(
        audio_id: Option<MediaPlaylistPermanentId>,
        video_id: Option<MediaPlaylistPermanentId>,
    ) -> (
        Option<MediaPlaylistPermanentId>,
        Option<MediaPlaylistPermanentId>,
    ) {
        if audio_id.is_some() && audio_id == video_id {
            (None, video_id)
        } else {
            (audio_id, video_id)
        }
    }

    fn curr_multivariant_media_info(&self, media_type: MediaType) -> Option<&ExternalMediaInfo> {
        let playlist_id = self.current_media_playlist_id(media_type)?;
        self.multivariant_media_info.get(playlist_id)
    }

    /// Check audio/video support for variants and the currently selected playlists.
    /// Text-only variants are rejected because main content needs audio or video.
    /// Subtitle format validation belongs outside this audio/video support check.
    ///
    /// Return Ready once the selected audio/video support is known, AwaitingSupportCheck
    /// while browser replies are pending, or VariantSelectionNeeded if another variant is needed.
    /// Return an error if the selected variant is unsupported and no compatible alternative remains.
    fn resolve_multivariant_audio_video_support(
        &mut self,
    ) -> Result<MultivariantStartupStatus, PlaylistStoreError> {
        if self.selected_media_support_resolved {
            return Ok(MultivariantStartupStatus::Ready);
        }

        let current_variant_id = self.current_variant_id;
        let current_audio_required = self.current_audio_id.is_some();
        let current_video_required = self.current_video_id.is_some();
        let inferred_audio_codec = self
            .curr_multivariant_media_info(MediaType::Audio)
            .map(|i| i.codec.clone());
        let inferred_video_codec = self
            .curr_multivariant_media_info(MediaType::Video)
            .map(|i| i.codec.clone());

        let playlist = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist,
            TopLevelPlaylist::DirectMedia(_) => {
                return Ok(MultivariantStartupStatus::Ready);
            }
        };

        let mut current_variant_resolved = true;
        let mut support_updates = Vec::new();
        'variant: for v in playlist.all_variants() {
            // If support was already determined for this variant, reuse it.
            if self.variant_support(v.id()).is_some() {
                continue;
            }

            // Remove text-only variants
            if v.has_type(MediaType::Text)
                && !v.has_type(MediaType::Audio)
                && !v.has_type(MediaType::Video)
            {
                support_updates.push((v.id(), false));
                continue;
            }
            let mut variant_is_supported = false;
            let is_current_variant = Some(v.id()) == current_variant_id;
            let mut saw_codec = false;

            for (media_type, required, inferred_codec) in [
                (
                    MediaType::Video,
                    current_video_required,
                    &inferred_video_codec,
                ),
                (
                    MediaType::Audio,
                    current_audio_required,
                    &inferred_audio_codec,
                ),
            ] {
                let codec = v.codecs(media_type).or_else(|| {
                    if is_current_variant {
                        inferred_codec.clone()
                    } else {
                        None
                    }
                });

                let Some(codec) = codec else {
                    if is_current_variant && required {
                        current_variant_resolved = false;
                        continue 'variant;
                    }
                    continue;
                };
                saw_codec = true;

                match jsIsTypeSupported(media_type, &codec) {
                    Some(false) => {
                        support_updates.push((v.id(), false));
                        continue 'variant;
                    }
                    Some(true) => variant_is_supported = true,
                    None => {
                        if is_current_variant {
                            current_variant_resolved = false;
                        }
                        continue 'variant;
                    }
                }
            }

            if variant_is_supported {
                support_updates.push((v.id(), true));
            } else if is_current_variant && !saw_codec {
                current_variant_resolved = false;
            }
        }

        support_updates
            .into_iter()
            .for_each(|(variant_id, supported)| self.set_variant_support(variant_id, supported));

        let curr_variant_support = current_variant_id.and_then(|id| self.variant_support(id));

        if curr_variant_support == Some(false) {
            if !self.compatible_variants().next().is_none() {
                self.selected_media_support_resolved = false;
                return Ok(MultivariantStartupStatus::VariantSelectionNeeded);
            } else {
                log_error!("PS: No supported variant in the given MultivariantPlaylist");
                return Err(PlaylistStoreError::NoSupportedVariant);
            }
        }

        let is_selected_media_support_resolved =
            current_variant_resolved && curr_variant_support == Some(true);

        self.selected_media_support_resolved = is_selected_media_support_resolved;

        if is_selected_media_support_resolved {
            log_info!("PS: Support has been resolved for the current multivariant startup path");
        } else {
            log_info!("PS: Current multivariant startup path still needs support resolution");
        }
        if is_selected_media_support_resolved {
            Ok(MultivariantStartupStatus::Ready)
        } else {
            Ok(MultivariantStartupStatus::AwaitingSupportCheck)
        }
    }

    /// Return the audio track ID used by the player API.
    /// If an audio Media Playlist was loaded directly, return DIRECT_MEDIA_AUDIO_TRACK_ID.
    /// XXX TODO: Not clear to me as why this would be different thant current_audio_track_id
    fn effective_audio_track_id(&self) -> Option<u32> {
        self.fixed_audio_track.or_else(|| {
            if matches!(&self.playlist, TopLevelPlaylist::DirectMedia(_))
                && self.current_audio_id.is_some()
            {
                Some(DIRECT_MEDIA_AUDIO_TRACK_ID)
            } else {
                self.current_audio_track_id()
            }
        })
    }

    /// Returns a reference to the MediaPlaylist currently loaded for the given `MediaType`.
    ///
    /// Returns `None` either if there's no MediaPlaylist selected for that `MediaType` or if the
    /// MediaPlaylist is not yet loaded.
    fn current_media_playlist(&self, media_type: MediaType) -> Option<&MediaPlaylist> {
        self.media_playlist(self.current_media_playlist_id(media_type)?)
    }

    /// Support for the selected format
    fn current_media_support(&self, media_type: MediaType) -> Option<bool> {
        let codec = self.current_codec(media_type)?;
        match media_type {
            // XXX TODO: An unrecognized declared text codec or inferred MIME type should trigger
            // segment/init probing, not immediate rejection. Identify the actual format from bytes
            // before deciding that it is unsupported.
            MediaType::Text => Some(text::supports_codec(&codec)),
            MediaType::Audio | MediaType::Video => jsIsTypeSupported(media_type, &codec),
        }
    }

    #[cfg(test)]
    pub(crate) fn assume_variant_supported_for_test(&mut self, variant_id: u32) {
        self.set_variant_support(variant_id, true);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProbeSegmentMetadata {
    /// The URL to that segment.
    pub(crate) url: Url,
    /// The optional byte-range to that segment.
    pub(crate) byte_range: Option<ByteRange>,
    // Supplementary context information about that probe segment
    pub(crate) context: ProbeSegmentContext,
}

// Supplementary context information about a segment used for "probing"
#[derive(Clone, Debug)]
pub(crate) enum ProbeSegmentContext {
    /// This probe segment is a full valid initialization segment
    Init { id: f64 },
    /// This probe segment is a full valid Media segment
    Media {
        /// Sequence number identifying that segment in the playlist.
        sequence: u32,
        /// Discontinuity sequence associated with that segment in the playlist.
        discontinuity_sequence: u32,
        /// Timing information linked to that media segment
        time_info: SegmentTimeInfo,
    },
}

/// State associated with the initial setup of the top-level playlist (either the
/// Multivariant playlist or a direct media playlist).
///
/// That setup is complexified by the need to know which and if media announced in those
/// playlists are supported, which can necessitate a segment request and other async API
/// calls.
pub(crate) enum StartupStatus {
    /// An initial variant or a replacement for an unsupported variant must be chosen.
    VariantSelectionNeeded,
    /// The currently-selected media playlists are not all loaded yet.
    /// TODO: Allow segment fetching for already loaded playlists even if the other one
    /// is still pending?
    AwaitingPlaylists,
    /// A "probe segment" must be loaded and inspected for supplementary
    /// playlist information before playback can start.
    ///
    /// Once the supplementary information is extracted, is should be communicated to the
    /// `PlaylistStore` (e.g. through `set_external_media_info`).
    ///
    /// The second value of that tuple is either:
    ///
    /// - `Some(media_type)` where `media_type` is the corresponding known media type of
    ///   the corresponding active playlist.
    ///
    /// - `None` if the `MediaType` isn't known or is not important (e.g. Direct Media
    ///   Playlist cases)
    NeedsProbes(Vec<(ProbeSegmentMetadata, Option<MediaType>)>),
    /// Codecs linked to the contents are known but are currently in their way to be
    /// checked by the platform.
    ///
    /// Once known, it should be communicated back to the `PlaylistStore`.
    AwaitingSupportCheck,
    /// The top level playlist can now be fully exploited for playback.
    Ready,
}

enum MultivariantStartupStatus {
    AwaitingSupportCheck,
    VariantSelectionNeeded,
    Ready,
}

/// The fixed track choice is not available in the supplied variant.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct IncompatibleTrack;

/// Reason a track selection was rejected without changing the store.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TrackSelectionError {
    UnknownTrack,
    NoCompatibleVariant,
}

/// Selected audio and subtitle track changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TrackChanges {
    audio: TrackChange,
    subtitles: TrackChange,
}

/// Changes made by set_audio_track().
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AudioTrackUpdate {
    /// The selected audio playlist ID changed.
    audio_playlist_changed: bool,
    /// The selected subtitle playlist ID changed.
    subtitle_playlist_changed: bool,
    /// Audio/text track change caused by this operation.
    tracks: TrackChanges,
    /// An existing variant lock was removed.
    variant_lock_removed: bool,
    /// The request was saved, but needs another variant before its playlist can be selected.
    needs_variant_selection: bool,
}

/// Changes made by set_subtitle_track().
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SubtitleTrackUpdate {
    /// The selected subtitle playlist ID changed.
    playlist_changed: bool,
    /// Audio/text track change caused by this operation.
    track_change: TrackChange,
    /// An existing variant lock was removed.
    variant_lock_removed: bool,
    /// The request was saved, but needs another variant before its playlist can be selected.
    needs_variant_selection: bool,
}

/// Changes made by disable_subtitle_track().
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SubtitleDisableUpdate {
    /// The selected subtitle playlist ID changed.
    playlist_changed: bool,
    /// Audio/text track change caused by this operation.
    track_change: TrackChange,
}

/// Changes made by set_variant().
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct VariantUpdate {
    /// Media types whose selected playlist ID changed.
    changed_playlists: Vec<MediaType>,
    /// Audio/text track change caused by this operation.
    tracks: TrackChanges,
    /// The current variant ID changed.
    variant_changed: bool,
    /// The new locked variant ID, if an existing lock changed.
    locked_variant_change: Option<u32>,
}

/// Selection changes made when probe information identifies a direct playlist's media type.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ExternalMediaInfoUpdate {
    /// Media types whose selected playlist ID changed.
    changed_playlists: Vec<MediaType>,
}

impl TrackChanges {
    pub(crate) fn new(audio: TrackChange, subtitles: TrackChange) -> Self {
        Self { audio, subtitles }
    }

    pub(crate) fn audio(&self) -> TrackChange {
        self.audio
    }

    pub(crate) fn subtitles(&self) -> TrackChange {
        self.subtitles
    }
}

impl AudioTrackUpdate {
    pub(crate) fn audio_playlist_changed(&self) -> bool {
        self.audio_playlist_changed
    }

    pub(crate) fn subtitle_playlist_changed(&self) -> bool {
        self.subtitle_playlist_changed
    }

    pub(crate) fn tracks(&self) -> TrackChanges {
        self.tracks
    }

    pub(crate) fn variant_lock_removed(&self) -> bool {
        self.variant_lock_removed
    }

    pub(crate) fn needs_variant_selection(&self) -> bool {
        self.needs_variant_selection
    }
}

impl SubtitleTrackUpdate {
    pub(crate) fn playlist_changed(&self) -> bool {
        self.playlist_changed
    }

    pub(crate) fn track_change(&self) -> TrackChange {
        self.track_change
    }

    pub(crate) fn variant_lock_removed(&self) -> bool {
        self.variant_lock_removed
    }

    pub(crate) fn needs_variant_selection(&self) -> bool {
        self.needs_variant_selection
    }
}

impl SubtitleDisableUpdate {
    pub(crate) fn playlist_changed(&self) -> bool {
        self.playlist_changed
    }

    pub(crate) fn track_change(&self) -> TrackChange {
        self.track_change
    }
}

impl VariantUpdate {
    pub(crate) fn tracks(&self) -> TrackChanges {
        self.tracks
    }

    pub(crate) fn variant_changed(&self) -> bool {
        self.variant_changed
    }

    pub(crate) fn locked_variant_change(&self) -> Option<u32> {
        self.locked_variant_change
    }

    pub(crate) fn into_changed_playlists(self) -> Vec<MediaType> {
        self.changed_playlists
    }
}

impl ExternalMediaInfoUpdate {
    pub(crate) fn into_changed_playlists(self) -> Vec<MediaType> {
        self.changed_playlists
    }
}

/// A logical track ID change, independent of changes to its backing playlist.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum TrackChange {
    /// The track ID stayed the same.
    #[default]
    Unchanged,
    /// The new track ID. None means that no logical track is selected.
    Changed(Option<u32>),
}

/// Return value for the `lock_variant` API.
pub(crate) enum LockVariantResponse {
    /// Error status for when no variant with the given id was found.
    NoVariantWithId,
    /// The exact same lock was already there.
    AlreadyLocked,
    /// The variant has been locked alongside changes performed by this request.
    VariantLocked {
        // XXX TODO: Isn't it more changed_media_playlists?
        changed_media_types: Vec<MediaType>,
        tracks: TrackChanges,
        variant_changed: bool,
    },
}

use thiserror::Error;

/// Error encountered when creating/updating a PlaylistStore
#[derive(Error, Debug)]
pub(crate) enum PlaylistStoreError {
    #[error("{0}")]
    Parsing(TopLevelPlaylistParsingError),
    #[error("No supported variant was found in the MultivariantPlaylist")]
    NoSupportedVariant,
    #[error("No variant was found in the MultivariantPlaylist. Are you sure that this isn't a Media Playlist?")]
    NoInitialVariant,
    #[error("No probe segment was available to determine startup metadata")]
    NoProbeSegment,
    #[error("No supported startup stream was found for the current content")]
    UnsupportedStartupStream,
    #[error("Text-only main content is not supported; load subtitles as a rendition of audio/video content")]
    TextOnlyMainContent,
}

#[cfg(test)]
mod tests {
    use super::{
        LockVariantResponse, MultivariantStartupStatus, PlaylistStore, PlaylistStoreError,
        StartupStatus, TrackChange, TrackSelectionError,
    };
    use crate::{
        adaptive::{AdaptiveQualitySelector, PlaybackConditions},
        bindings::MediaType,
        parser::{ExternalMediaInfo, TopLevelPlaylist},
        utils::url::Url,
    };

    #[test]
    fn automatic_audio_uses_defaults_and_restores_them_after_explicit_selection() {
        for default in [None, Some("English"), Some("French")] {
            let mut manifest = "#EXTM3U\n".to_string();
            for name in ["English", "French"] {
                let is_default = if default == Some(name) { "YES" } else { "NO" };
                manifest.push_str(&format!("#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{name}\",DEFAULT={is_default},URI=\"{name}.m3u8\"\n"));
            }
            manifest.push_str("#EXT-X-STREAM-INF:BANDWIDTH=100000,AUDIO=\"audio\"\nvideo.m3u8\n");
            let mut store = PlaylistStore::try_new(
                manifest.as_bytes(),
                parse_url("https://example.com/master.m3u8"),
            )
            .unwrap();
            let variant_id = store.available_variants().next().unwrap().id();
            store.set_variant(variant_id);
            let expected = store
                .audio_tracks()
                .iter()
                .find(|track| track.name() == Some(default.unwrap_or("English")))
                .unwrap()
                .id();
            assert_eq!(store.current_audio_track_id(), Some(expected));
            let other = store
                .audio_tracks()
                .iter()
                .find(|track| track.id() != expected)
                .unwrap()
                .id();
            store.set_audio_track(Some(other)).unwrap();
            assert_eq!(store.current_audio_track_id(), Some(other));
            store.set_audio_track(None).unwrap();
            assert_eq!(store.fixed_audio_track_id(), None);
            assert_eq!(store.current_audio_track_id(), Some(expected));
        }
    }

    #[test]
    fn missing_audio_and_subtitle_groups_do_not_prevent_selecting_the_variant() {
        let manifest = b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"missing\",SUBTITLES=\"missing\"\nvideo.m3u8\n";
        let mut store =
            PlaylistStore::try_new(&manifest[..], parse_url("https://example.com/master.m3u8"))
                .unwrap();
        let variant_id = store.available_variants().next().unwrap().id();
        store.set_variant(variant_id);
        assert_eq!(store.current_variant_id(), Some(variant_id));
        assert!(store.current_media_playlist_id(MediaType::Video).is_some());
        assert!(store.current_media_playlist_id(MediaType::Text).is_none());
        assert_eq!(store.compatible_variants().count(), 1);
    }

    fn subtitle_store(defaults: bool) -> PlaylistStore {
        let manifest = format!(
            r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="French",LANGUAGE="fr",URI="audio-fr.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="a",NAME="English",LANGUAGE="en",DEFAULT={default},AUTOSELECT=YES,URI="a-en.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="a",NAME="French",LANGUAGE="fr",URI="a-fr.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="a",NAME="Forced English",LANGUAGE="en",FORCED=YES,AUTOSELECT=YES,URI="a-forced-en.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="a",NAME="Forced French",LANGUAGE="fr",FORCED=YES,AUTOSELECT=YES,URI="a-forced-fr.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="b",NAME="English",LANGUAGE="en",DEFAULT={default},AUTOSELECT=YES,URI="b-en.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="b",NAME="French",LANGUAGE="fr",URI="b-fr.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="b",NAME="French",LANGUAGE="fr",CHARACTERISTICS="public.accessibility.transcribes-spoken-dialog",URI="b-fr-sdh.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=100000,AUDIO="audio",SUBTITLES="a"
a.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=200000,AUDIO="audio",SUBTITLES="b"
b.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=300000,AUDIO="audio"
c.m3u8
"#,
            default = if defaults { "YES" } else { "NO" }
        );
        let mut store = PlaylistStore::try_new(
            manifest.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let id = store.available_variants().next().unwrap().id();
        store.set_variant(id);
        store
    }

    fn subtitle_id(store: &PlaylistStore, name: &str) -> u32 {
        store
            .subtitle_tracks()
            .iter()
            .find(|track| track.name() == Some(name))
            .unwrap()
            .id()
    }

    #[test]
    fn candidate_subtitle_lookup_uses_candidate_audio_without_changing_selection() {
        let mut store = PlaylistStore::try_new(br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="fr.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Forced English",LANGUAGE="en",FORCED=YES,AUTOSELECT=YES,URI="en-subs.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Forced French",LANGUAGE="fr",FORCED=YES,AUTOSELECT=YES,URI="fr-subs.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="en",SUBTITLES="subs"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,AUDIO="fr",SUBTITLES="subs"
fr-video.m3u8
"#, parse_url("https://example.com/master.m3u8")).unwrap();
        let first = store.available_variants().next().unwrap().id();
        let other = store.available_variants().nth(1).unwrap().id();
        store.set_variant(first);
        let current = *store.current_media_playlist_id(MediaType::Text).unwrap();
        let candidate = store
            .resolve_playlist_id_for_variant(store.variant(other).unwrap(), MediaType::Text)
            .unwrap()
            .unwrap();
        assert_ne!(candidate, current);
        assert_eq!(store.current_variant_id(), Some(first));
        assert_eq!(
            store.current_media_playlist_id(MediaType::Text),
            Some(&current)
        );
        store.set_variant(other);
        assert_eq!(
            store.current_media_playlist_id(MediaType::Text),
            Some(&candidate)
        );
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "Forced French"))
        );
        store
            .set_subtitle_track(Some(subtitle_id(&store, "Forced English")))
            .unwrap();
        assert_eq!(
            store
                .resolve_playlist_id_for_variant(store.variant(first).unwrap(), MediaType::Text)
                .unwrap(),
            Some(current)
        );
        store.disable_subtitle_track();
        assert_eq!(
            store
                .resolve_playlist_id_for_variant(store.variant(first).unwrap(), MediaType::Text)
                .unwrap(),
            None
        );
    }

    #[test]
    fn subtitle_tracks_keep_the_same_id_across_variant_groups() {
        let mut store = subtitle_store(true);
        let french = subtitle_id(&store, "French");
        assert_eq!(store.subtitle_tracks().len(), 5);
        store.set_subtitle_track(Some(french)).unwrap();
        let first_playlist = *store.current_media_playlist_id(MediaType::Text).unwrap();
        let variants: Vec<_> = store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        assert!(store
            .set_variant(variants[1])
            .changed_playlists
            .contains(&MediaType::Text));
        assert_eq!(store.current_subtitle_track_id(), Some(french));
        assert_ne!(
            store.current_media_playlist_id(MediaType::Text),
            Some(&first_playlist)
        );
        store.set_variant(variants[2]);
        assert_eq!(store.current_subtitle_track_id(), None);
        assert_eq!(store.fixed_subtitle_track_id(), Some(french));
        store.set_variant(variants[0]);
        assert_eq!(store.current_subtitle_track_id(), Some(french));
        assert_eq!(
            store.current_media_playlist_id(MediaType::Text),
            Some(&first_playlist)
        );
    }

    #[test]
    fn subtitle_null_restores_defaults_and_disable_is_separate() {
        let mut store = subtitle_store(true);
        let english = subtitle_id(&store, "English");
        let french = subtitle_id(&store, "French");
        assert_eq!(store.current_subtitle_track_id(), Some(english));
        store.set_subtitle_track(Some(french)).unwrap();
        store.disable_subtitle_track();
        assert_eq!(store.current_subtitle_track_id(), None);
        let other = store.available_variants().nth(1).unwrap().id();
        store.set_variant(other);
        assert_eq!(store.current_subtitle_track_id(), None);
        store.set_subtitle_track(None).unwrap();
        assert_eq!(store.fixed_subtitle_track_id(), None);
        assert_eq!(store.current_subtitle_track_id(), Some(english));
        store.set_subtitle_track(None).unwrap();
        store.disable_subtitle_track();
        store.set_subtitle_track(Some(french)).unwrap();
        assert_eq!(store.current_subtitle_track_id(), Some(french));
    }

    #[test]
    fn automatic_forced_subtitles_follow_the_audio_language() {
        let mut store = subtitle_store(false);
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "Forced English"))
        );
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        store.set_audio_track(Some(french)).unwrap();
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "Forced French"))
        );
        store.disable_subtitle_track();
        store.set_audio_track(None).unwrap();
        assert_eq!(store.current_subtitle_track_id(), None);
    }

    #[test]
    fn fixed_forced_subtitles_survive_audio_changes_until_automatic_is_restored() {
        let mut store = subtitle_store(false);
        let forced_english = subtitle_id(&store, "Forced English");
        let forced_french = subtitle_id(&store, "Forced French");
        let french_audio = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        store.set_subtitle_track(Some(forced_english)).unwrap();
        store.set_audio_track(Some(french_audio)).unwrap();
        assert_eq!(store.current_subtitle_track_id(), Some(forced_english));
        assert_eq!(store.fixed_subtitle_track_id(), Some(forced_english));

        store.set_subtitle_track(None).unwrap();
        assert_eq!(store.current_subtitle_track_id(), Some(forced_french));
        store.disable_subtitle_track();
        store.set_audio_track(None).unwrap();
        assert_eq!(store.current_subtitle_track_id(), None);
        store.set_subtitle_track(None).unwrap();
        assert_eq!(store.current_subtitle_track_id(), Some(forced_english));
    }

    #[test]
    fn automatic_subtitles_use_defaults_from_the_current_variant_group() {
        let manifest = b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"French\",LANGUAGE=\"fr\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"a\",NAME=\"English\",LANGUAGE=\"en\",DEFAULT=YES,URI=\"a-en.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"b\",NAME=\"English\",LANGUAGE=\"en\",URI=\"b-en.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"b\",NAME=\"Forced French\",LANGUAGE=\"fr\",FORCED=YES,AUTOSELECT=YES,URI=\"b-fr.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=100000,AUDIO=\"audio\",SUBTITLES=\"a\"\na.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=200000,AUDIO=\"audio\",SUBTITLES=\"b\"\nb.m3u8\n";
        let mut store =
            PlaylistStore::try_new(&manifest[..], parse_url("https://example.com/master.m3u8"))
                .unwrap();
        let variants: Vec<_> = store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        store.set_variant(variants[0]);
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "English"))
        );
        store.set_variant(variants[1]);
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "Forced French"))
        );
        store.set_variant(variants[0]);
        assert_eq!(
            store.current_subtitle_track_id(),
            Some(subtitle_id(&store, "English"))
        );
    }

    #[test]
    fn explicit_subtitle_choice_uses_the_compatible_variant_pool() {
        let mut store = subtitle_store(true);
        let variants: Vec<_> = store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        assert!(matches!(
            store.lock_variant(variants[0]),
            LockVariantResponse::VariantLocked { .. }
        ));
        let sdh = store
            .subtitle_tracks()
            .iter()
            .find(|track| !track.characteristics().is_empty())
            .unwrap()
            .id();
        assert!(
            store
                .set_subtitle_track(Some(sdh))
                .unwrap()
                .needs_variant_selection
        );
        assert!(!store.is_variant_locked());
        assert_eq!(
            store
                .compatible_variants()
                .map(|variant| variant.id())
                .collect::<Vec<_>>(),
            vec![variants[1]]
        );
        store.set_variant(variants[1]);
        assert_eq!(store.current_subtitle_track_id(), Some(sdh));
        store.disable_subtitle_track();
        assert_eq!(store.compatible_variants().count(), 3);
        store.set_subtitle_track(None).unwrap();
        assert_eq!(store.fixed_subtitle_track_id(), None);
        assert_eq!(store.compatible_variants().count(), 3);
    }

    #[test]
    fn invalid_subtitle_ids_leave_selection_unchanged() {
        let mut store = subtitle_store(true);
        let french = subtitle_id(&store, "French");
        store.set_subtitle_track(Some(french)).unwrap();
        assert_eq!(
            store.set_subtitle_track(Some(u32::MAX)),
            Err(super::TrackSelectionError::UnknownTrack)
        );
        assert_eq!(store.fixed_subtitle_track_id(), Some(french));
        assert_eq!(store.current_subtitle_track_id(), Some(french));
    }

    #[test]
    fn text_metadata_and_capabilities_do_not_invalidate_av_support() {
        let mut store = subtitle_store(true);
        let variant = store.current_variant_id().unwrap();
        store.set_variant_support(variant, true);
        store.selected_media_support_resolved = true;
        for (codec, expected) in [("wvtt", true), ("stpp.ttml.im1t", true), ("unknown", false)] {
            store.set_external_media_info(
                ExternalMediaInfo {
                    mime_type: "application/mp4".into(),
                    media_type: MediaType::Text,
                    codec: codec.into(),
                },
                MediaType::Text,
            );
            assert_eq!(store.current_media_support(MediaType::Text), Some(expected));
            assert_eq!(store.variant_support(variant), Some(true));
            assert!(store.selected_media_support_resolved);
        }
    }

    #[test]
    fn unchanged_audio_and_probe_inputs_reuse_cached_support() {
        let mut store = subtitle_store(true);
        let variant = store.current_variant_id().unwrap();
        let english = store.current_audio_track_id().unwrap();
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        let info = ExternalMediaInfo {
            mime_type: "audio/mp4".into(),
            media_type: MediaType::Audio,
            codec: "mp4a.40.2".into(),
        };
        store.set_external_media_info(info.clone(), MediaType::Audio);
        store.set_variant_support(variant, true);
        store.selected_media_support_resolved = true;
        store.set_external_media_info(info.clone(), MediaType::Audio);
        store.set_audio_track(Some(english)).unwrap();
        assert_eq!(store.variant_support(variant), Some(true));
        assert!(store.selected_media_support_resolved);

        // A different playlist with an unknown codec needs its own support check.
        store.set_audio_track(Some(french)).unwrap();
        assert_eq!(store.variant_support(variant), None);
        store.set_external_media_info(info.clone(), MediaType::Audio);
        store.set_variant_support(variant, true);
        store.selected_media_support_resolved = true;
        // A playlist change also invalidates support when its probed metadata is identical.
        store.set_audio_track(Some(english)).unwrap();
        assert_eq!(store.variant_support(variant), None);
        assert!(!store.selected_media_support_resolved);

        store.set_variant_support(variant, true);
        store.selected_media_support_resolved = true;
        store.set_external_media_info(
            ExternalMediaInfo {
                codec: "ec-3".into(),
                ..info
            },
            MediaType::Audio,
        );
        assert_eq!(store.variant_support(variant), None);
        assert!(!store.selected_media_support_resolved);
    }

    #[test]
    fn supported_text_format_is_rejected_only_as_main_content() {
        for codec in ["wvtt", "stpp.ttml.im1t"] {
            let mut store = PlaylistStore::try_new(
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg\n#EXT-X-ENDLIST\n",
                parse_url("https://example.com/text.m3u8"),
            )
            .unwrap();
            store.set_external_media_info(
                ExternalMediaInfo {
                    mime_type: "application/mp4".into(),
                    media_type: MediaType::Text,
                    codec: codec.into(),
                },
                MediaType::Text,
            );
            assert_eq!(store.current_media_support(MediaType::Text), Some(true));
            assert!(matches!(
                store.startup_status(0.),
                Err(PlaylistStoreError::TextOnlyMainContent)
            ));
        }
    }

    #[test]
    fn text_only_variant_does_not_wait_forever_for_av_codecs() {
        let mut store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS=\"wvtt\"\ntext.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let variant = store.available_variants().next().unwrap().id();
        store.set_variant(variant);
        assert!(matches!(
            store.startup_status(0.),
            Err(PlaylistStoreError::NoSupportedVariant)
        ));
    }

    #[test]
    fn program_date_time_query_includes_text_timing_metadata() {
        let mut store = subtitle_store(true);
        let text = *store.current_media_playlist_id(MediaType::Text).unwrap();
        store.update_media_playlist(&text,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-PROGRAM-DATE-TIME:2025-01-01T00:00:00Z\n#EXTINF:4,\nseg.vtt\n".as_slice(),
            parse_url("https://example.com/subs.m3u8"),
        ).unwrap();
        assert!(store.uses_program_date_time());
    }

    #[test]
    fn generic_playlist_metadata_includes_text_without_shortening_playback_positions() {
        let mut store = subtitle_store(true);
        for media_type in [MediaType::Audio, MediaType::Video] {
            let id = *store.current_media_playlist_id(media_type).unwrap();
            store
                .update_media_playlist(
                    &id,
                    &b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n"[..],
                    parse_url("https://example.com/media.m3u8"),
                )
                .unwrap();
        }
        assert!(!store.is_finalized());
        assert_eq!(store.current_media_playlists().count(), 2);
        let text = *store.current_media_playlist_id(MediaType::Text).unwrap();
        assert!(!store.has_loaded_media_playlist(MediaType::Text));
        store
            .update_media_playlist(
                &text,
                &b"#EXTM3U\n#EXT-X-TARGETDURATION:100\n#EXTINF:100,\nseg.vtt\n"[..],
                parse_url("https://example.com/subs.m3u8"),
            )
            .unwrap();
        assert_eq!(store.current_media_playlists().count(), 3);
        assert_eq!(
            store
                .current_media_playlist(MediaType::Text)
                .map(|playlist| playlist.target_duration()),
            Some(100.)
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Text),
            Some(100.)
        );
        assert_eq!(
            store
                .current_media_playlist(MediaType::Text)
                .unwrap()
                .segment_list()
                .media()
                .len(),
            1
        );
        assert_eq!(store.playlist_type(), crate::bindings::PlaylistNature::Live);
        assert_eq!(store.expected_start_time(), 0.);
        assert!(!store.is_finalized());
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(4.)
        );
        store
            .update_media_playlist(
                &text,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nshort.vtt\n#EXT-X-ENDLIST\n"
                    .as_slice(),
                parse_url("https://example.com/subs.m3u8"),
            )
            .unwrap();
        assert!(store.is_finalized());
        store.disable_subtitle_track();
        assert_eq!(store.current_media_playlists().count(), 2);
        if let TopLevelPlaylist::Multivariant(playlist) = &mut store.playlist {
            playlist.clear_media_playlist(&text);
        }
        assert_eq!(
            store.max_observed_target_duration(MediaType::Text),
            Some(100.)
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Audio),
            Some(4.)
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(4.)
        );
    }

    fn parse_url(url: &str) -> Url {
        Url::new(url.to_string())
    }

    #[test]
    fn switching_audio_only_variants_updates_only_the_audio_playlist() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=65000,CODECS="mp4a.40.5"
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=128000,CODECS="mp4a.40.2"
high.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let low = store.available_variants().next().unwrap().id();
        let high = store.available_variants().nth(1).unwrap().id();

        assert_eq!(
            store.set_variant(low).changed_playlists,
            vec![MediaType::Audio]
        );
        let low_playlist = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        assert!(store.current_media_playlist_id(MediaType::Video).is_none());
        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::AwaitingPlaylists)
        ));

        assert_eq!(
            store.set_variant(high).changed_playlists,
            vec![MediaType::Audio]
        );
        assert_ne!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&low_playlist)
        );
        assert!(store.current_media_playlist_id(MediaType::Video).is_none());
        assert_eq!(
            store.set_variant(low).changed_playlists,
            vec![MediaType::Audio]
        );
        assert_eq!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&low_playlist)
        );
    }

    #[test]
    fn constructor_preserves_playlist_parsing_errors() {
        use crate::parser::TopLevelPlaylistParsingError;

        for (data, expected_kind) in [
            (b"not a playlist".as_slice(), "unknown"),
            (b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"a\",URI=\"a.m3u8\"\n".as_slice(), "media"),
            (b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n#EXT-X-TARGETDURATION:4\n".as_slice(), "multivariant"),
        ] {
            let result = PlaylistStore::try_new(data, parse_url("https://example.com/playlist.m3u8"));
            let kind = match result {
                Err(PlaylistStoreError::Parsing(TopLevelPlaylistParsingError::NotAPlaylist)) => "unknown",
                Err(PlaylistStoreError::Parsing(TopLevelPlaylistParsingError::Media(_))) => "media",
                Err(PlaylistStoreError::Parsing(TopLevelPlaylistParsingError::Multivariant(_))) => "multivariant",
                _ => panic!("Expected a playlist parsing error"),
            };
            assert_eq!(kind, expected_kind);
        }
    }

    #[test]
    fn switching_variants_that_share_video_changes_only_audio_playlist() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="french",NAME="French",URI="french.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="english",NAME="English",URI="english.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="french"
video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="english"
video.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let first = store.available_variants().next().unwrap().id();
        let second = store.available_variants().nth(1).unwrap().id();
        store.set_variant(first);
        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let audio_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();

        assert_eq!(
            store.set_variant(second).changed_playlists,
            vec![MediaType::Audio]
        );
        assert_eq!(
            store.current_media_playlist_id(MediaType::Video),
            Some(&video_id)
        );
        assert_ne!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&audio_id)
        );
    }

    fn store_with_timeline_references(video_loaded: bool, audio_loaded: bool) -> PlaylistStore {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="low",NAME="English",DEFAULT=YES,URI="low-audio.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="high",NAME="English",DEFAULT=YES,URI="high-audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="low"
low-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="high"
high-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=3000
unknown.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let low = store.available_variants().next().unwrap().id();
        store.set_variant(low);
        for (media_type, loaded, sequence) in [
            (MediaType::Video, video_loaded, 100),
            (MediaType::Audio, audio_loaded, 200),
        ] {
            if loaded {
                let id = *store.current_media_playlist_id(media_type).unwrap();
                let data = format!("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:{sequence}\n#EXT-X-PROGRAM-DATE-TIME:2025-01-01T00:00:00Z\n#EXTINF:4,\nfirst.ts\n#EXTINF:4,\nsecond.ts\n");
                let TopLevelPlaylist::Multivariant(playlist) = &mut store.playlist else {
                    unreachable!();
                };
                playlist
                    .update_media_playlist(
                        &id,
                        data.as_bytes(),
                        parse_url("https://example.com/reference.m3u8"),
                        None,
                    )
                    .unwrap();
            }
        }
        store
    }

    #[test]
    fn playlist_alignment_preserves_selected_references_and_handles_unselected_playlists() {
        for (media_type, selected, video_loaded, audio_loaded, sequence, reference_type) in [
            (
                MediaType::Video,
                true,
                true,
                true,
                201,
                Some(MediaType::Audio),
            ),
            (
                MediaType::Audio,
                true,
                true,
                true,
                101,
                Some(MediaType::Video),
            ),
            (MediaType::Video, true, true, false, 101, None),
            (MediaType::Audio, true, false, true, 201, None),
            (
                MediaType::Video,
                false,
                true,
                true,
                101,
                Some(MediaType::Video),
            ),
            (
                MediaType::Audio,
                false,
                true,
                true,
                201,
                Some(MediaType::Audio),
            ),
            (
                MediaType::Video,
                false,
                false,
                true,
                201,
                Some(MediaType::Audio),
            ),
            (
                MediaType::Audio,
                false,
                true,
                false,
                101,
                Some(MediaType::Video),
            ),
        ] {
            let mut store = store_with_timeline_references(video_loaded, audio_loaded);
            let expected = reference_type
                .map(|media_type| {
                    let id = store.current_media_playlist_id(media_type).unwrap();
                    store.media_playlist(id).unwrap().segment_list().media()[1].start()
                })
                .unwrap_or(0.);
            let high = store.available_variants().nth(1).unwrap().id();
            let id = store
                .resolve_playlist_id_for_variant(store.variant(high).unwrap(), media_type)
                .unwrap()
                .unwrap();
            if selected {
                // Isolate reference selection: keep the opposite playlist as the reference.
                match media_type {
                    MediaType::Video => store.current_video_id = Some(id),
                    MediaType::Audio => store.current_audio_id = Some(id),
                    MediaType::Text => unreachable!("this test covers A/V timeline references"),
                }
            }
            let data = format!("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:{sequence}\n#EXTINF:4,\nnew.ts\n");
            let updated = store
                .update_media_playlist(
                    &id,
                    data.as_bytes(),
                    parse_url("https://example.com/new.m3u8"),
                )
                .unwrap();
            assert_eq!(updated.segment_list().media()[0].start(), expected,
                "media_type={media_type:?}, selected={selected}, video_loaded={video_loaded}, audio_loaded={audio_loaded}");
        }
    }

    #[test]
    fn unselected_playlist_with_unknown_type_uses_video_unless_probing_identifies_audio() {
        for probed_audio in [false, true] {
            let mut store = store_with_timeline_references(true, true);
            let unknown = store.available_variants().nth(2).unwrap().id();
            let id = store
                .resolve_playlist_id_for_variant(store.variant(unknown).unwrap(), MediaType::Video)
                .unwrap()
                .unwrap();
            let reference_type = if probed_audio {
                MediaType::Audio
            } else {
                MediaType::Video
            };
            let reference_id = store.current_media_playlist_id(reference_type).unwrap();
            let expected = store
                .media_playlist(reference_id)
                .unwrap()
                .segment_list()
                .media()[1]
                .start();
            if probed_audio {
                store.multivariant_media_info.insert(
                    id,
                    ExternalMediaInfo {
                        media_type: MediaType::Audio,
                        codec: "mp4a.40.2".into(),
                        mime_type: "audio/mp4".into(),
                    },
                );
            }
            let sequence = if probed_audio { 201 } else { 101 };
            let data = format!("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:{sequence}\n#EXTINF:4,\nnew.ts\n");
            let updated = store
                .update_media_playlist(
                    &id,
                    data.as_bytes(),
                    parse_url("https://example.com/new.m3u8"),
                )
                .unwrap();
            assert_eq!(updated.segment_list().media()[0].start(), expected);
        }
    }

    #[test]
    fn startup_preserves_candidates_when_initial_audio_track_has_no_variant() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"
video.m3u8
"#
            .as_slice(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let track_id = store.audio_tracks().first().unwrap().id();
        assert!(matches!(
            store.set_audio_track(Some(track_id)),
            Err(TrackSelectionError::NoCompatibleVariant)
        ));
        assert_eq!(store.fixed_audio_track_id(), None);

        assert!(store.current_variant_id().is_none());
        assert_eq!(store.compatible_variants().count(), 1);
        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::VariantSelectionNeeded)
        ));
    }

    #[test]
    fn startup_requests_initial_selection_when_compatible_variants_exist() {
        let mut store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();

        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::VariantSelectionNeeded)
        ));
    }

    fn playlist_store_with_variant(
        mut store: PlaylistStore,
        initial_bandwidth: f64,
    ) -> PlaylistStore {
        let selector = AdaptiveQualitySelector::new(initial_bandwidth);
        if let Some(selection) = selector.select_variant(
            &store.compatible_variants().collect::<Vec<_>>(),
            store.current_variant_id(),
            &PlaybackConditions {
                buffer_level: Some(0.),
                buffer_goal: 30.,
                playback_speed: 1.,
                abr_reference_segment_duration: [MediaType::Audio, MediaType::Video]
                    .into_iter()
                    .filter_map(|media_type| store.max_observed_target_duration(media_type))
                    .reduce(f64::max),
            },
        ) {
            store.set_variant(selection.best_variant_id);
        }
        store
    }

    #[test]
    fn maximum_observed_target_duration_survives_cached_live_variant_switches() {
        let playlist = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000\nhigh.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 1_500.);
        let low_id = store.available_variants().next().unwrap().id();
        let high_id = store.available_variants().nth(1).unwrap().id();
        let low_playlist_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        assert_eq!(store.max_observed_target_duration(MediaType::Video), None);

        assert!(store
            .update_media_playlist(
                &low_playlist_id,
                b"invalid playlist".as_slice(),
                parse_url("https://example.com/low.m3u8"),
            )
            .is_err());
        assert_eq!(store.max_observed_target_duration(MediaType::Video), None);

        store
            .update_media_playlist(
                &low_playlist_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nseg.ts\n".as_slice(),
                parse_url("https://example.com/low.m3u8"),
            )
            .unwrap();
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(6.)
        );

        store.set_variant(high_id);
        assert_eq!(
            store
                .current_media_playlist(MediaType::Video)
                .map(|playlist| playlist.target_duration()),
            None
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(6.)
        );
        let high_playlist_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        store
            .update_media_playlist(
                &high_playlist_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n".as_slice(),
                parse_url("https://example.com/high.m3u8"),
            )
            .unwrap();
        assert_eq!(
            store
                .current_media_playlist(MediaType::Video)
                .map(|playlist| playlist.target_duration()),
            Some(4.)
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(6.)
        );

        store.set_variant(low_id);
        assert!(store.has_loaded_media_playlist(MediaType::Video));
        assert_eq!(
            store
                .current_media_playlist(MediaType::Video)
                .map(|playlist| playlist.target_duration()),
            Some(6.)
        );
        assert_eq!(
            store.max_observed_target_duration(MediaType::Video),
            Some(6.)
        );
        for duration in [8, 4] {
            let media =
                format!("#EXTM3U\n#EXT-X-TARGETDURATION:{duration}\n#EXTINF:{duration},\nseg.ts\n");
            store
                .update_media_playlist(
                    &low_playlist_id,
                    media.as_bytes(),
                    parse_url("https://example.com/low.m3u8"),
                )
                .unwrap();
            assert_eq!(
                store
                    .current_media_playlist(MediaType::Video)
                    .map(|playlist| playlist.target_duration()),
                Some(duration as f64)
            );
            assert_eq!(
                store.max_observed_target_duration(MediaType::Video),
                Some(8.)
            );
        }
    }

    #[test]
    fn direct_target_duration_is_recorded_after_identifying_its_type() {
        for media_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
            let mut store = PlaylistStore::try_new(
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n",
                parse_url("https://example.com/direct.m3u8"),
            )
            .unwrap();
            assert_eq!(store.max_observed_target_duration(media_type), None);
            store.set_external_media_info(
                ExternalMediaInfo {
                    media_type,
                    mime_type: "application/mp4".into(),
                    codec: "test".into(),
                },
                media_type,
            );
            for queried_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
                assert_eq!(
                    store.max_observed_target_duration(queried_type),
                    (queried_type == media_type).then_some(4.)
                );
            }
            let id = *store.current_media_playlist_id(media_type).unwrap();
            for duration in [6, 2] {
                let media = format!("#EXTM3U\n#EXT-X-TARGETDURATION:{duration}\n#EXTINF:{duration},\nseg.ts\n#EXT-X-ENDLIST\n");
                store
                    .update_media_playlist(
                        &id,
                        media.as_bytes(),
                        parse_url("https://example.com/direct.m3u8"),
                    )
                    .unwrap();
                assert_eq!(store.max_observed_target_duration(media_type), Some(6.));
            }
            let new_store = PlaylistStore::try_new(
                b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n",
                parse_url("https://example.com/master.m3u8"),
            )
            .unwrap();
            assert_eq!(new_store.max_observed_target_duration(media_type), None);
        }
    }

    #[test]
    fn variant_lock_can_make_the_initial_selection() {
        let mut store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let variant = store.compatible_variants().next().unwrap();
        let variant_id = variant.id();
        let video_id = store
            .resolve_playlist_id_for_variant(variant, MediaType::Video)
            .unwrap()
            .unwrap();

        assert!(matches!(
            store.lock_variant(u32::MAX),
            LockVariantResponse::NoVariantWithId
        ));
        assert_eq!(store.current_variant_id(), None);
        assert!(!store.is_variant_locked());
        assert_eq!(store.current_media_playlist_id(MediaType::Video), None);

        let LockVariantResponse::VariantLocked {
            changed_media_types,
            tracks,
            variant_changed,
        } = store.lock_variant(variant_id)
        else {
            panic!("Expected the initial variant to be locked");
        };
        assert!(variant_changed);
        assert_eq!(changed_media_types, vec![MediaType::Video]);
        assert_eq!(tracks.audio, TrackChange::Unchanged);
        assert_eq!(tracks.subtitles, TrackChange::Unchanged);
        assert_eq!(store.current_variant_id(), Some(variant_id));
        assert!(store.is_variant_locked());
        assert_eq!(
            store.current_media_playlist_id(MediaType::Video),
            Some(&video_id)
        );
        assert!(matches!(
            store.lock_variant(variant_id),
            LockVariantResponse::AlreadyLocked
        ));
    }

    #[test]
    fn direct_media_playlist_rejects_a_variant_lock() {
        let mut store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nsegment.ts\n#EXT-X-ENDLIST\n",
            parse_url("https://example.com/direct.m3u8"),
        )
        .unwrap();

        assert!(matches!(
            store.lock_variant(0),
            LockVariantResponse::NoVariantWithId
        ));
        assert_eq!(store.current_variant_id(), None);
        assert!(!store.is_variant_locked());
    }

    #[test]
    fn adaptive_update_does_not_override_a_variant_lock() {
        let multivariant = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000
high.m3u8
"#;
        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 1_500.);
        let variants = store.available_variants().collect::<Vec<_>>();
        let low_id = variants[0].id();
        let high_id = variants[1].id();

        assert!(matches!(
            store.lock_variant(high_id),
            LockVariantResponse::VariantLocked { .. }
        ));
        assert!(matches!(
            store.lock_variant(high_id),
            LockVariantResponse::AlreadyLocked
        ));
        assert_eq!(store.compatible_variants().count(), 2);
        // Automatic recommendations respect the lock; explicit selection happens through locking.

        assert_eq!(store.current_variant_id(), Some(high_id));

        store.unlock_variant();
        assert!(!store.is_variant_locked());
        assert_eq!(store.current_variant_id(), Some(high_id));
        store.set_variant(low_id);
        assert_eq!(store.current_variant_id(), Some(low_id));
    }

    #[test]
    fn rejected_variant_lock_keeps_the_existing_lock() {
        let mut store = subtitle_store(true);
        let variants: Vec<_> = store.available_variants().map(|v| v.id()).collect();
        let sdh = store
            .subtitle_tracks()
            .iter()
            .find(|track| track.name() == Some("French") && !track.characteristics().is_empty())
            .unwrap()
            .id();
        store.set_subtitle_track(Some(sdh)).unwrap();
        store.set_variant(variants[1]);
        assert!(matches!(
            store.lock_variant(variants[1]),
            LockVariantResponse::VariantLocked { .. }
        ));
        for rejected in [u32::MAX, variants[0]] {
            assert!(matches!(
                store.lock_variant(rejected),
                LockVariantResponse::NoVariantWithId
            ));
            assert!(store.is_variant_locked());
            assert_eq!(store.current_variant_id(), Some(variants[1]));
            assert_eq!(store.current_subtitle_track_id(), Some(sdh));
        }
    }

    #[test]
    fn subtitle_results_distinguish_pending_choice_playlist_and_logical_track_changes() {
        let mut store = subtitle_store(true);
        let variants: Vec<_> = store.available_variants().map(|v| v.id()).collect();
        let selected = store.current_text_id;
        store.lock_variant(variants[0]);
        let sdh = store
            .subtitle_tracks()
            .iter()
            .find(|track| track.name() == Some("French") && !track.characteristics().is_empty())
            .unwrap()
            .id();
        let pending = store.set_subtitle_track(Some(sdh)).unwrap();
        assert!(pending.needs_variant_selection);
        assert_eq!(store.fixed_subtitle_track, Some(sdh));
        assert!(!store.subtitles_disabled);
        assert!(pending.variant_lock_removed);
        assert!(!pending.playlist_changed);
        assert_eq!(store.current_text_id, selected); // The variant switch applies the new playlist.
        let applied = store.set_variant(variants[1]);
        assert!(applied.variant_changed);
        assert!(applied.changed_playlists.contains(&MediaType::Text));
        assert_eq!(applied.tracks.subtitles, TrackChange::Changed(Some(sdh)));
        assert_eq!(
            store.set_subtitle_track(Some(sdh)).unwrap(),
            super::SubtitleTrackUpdate::default()
        );
        let disabled = store.disable_subtitle_track();
        assert!(store.subtitles_disabled);
        assert!(disabled.playlist_changed);
        assert_eq!(disabled.track_change, TrackChange::Changed(None));
        assert_eq!(
            store.disable_subtitle_track(),
            super::SubtitleDisableUpdate::default()
        );
        store.set_subtitle_track(Some(sdh)).unwrap();
        assert!(!store.subtitles_disabled);
        assert_eq!(store.fixed_subtitle_track, Some(sdh));
    }

    #[test]
    fn variant_results_do_not_confuse_playlist_changes_with_track_changes() {
        let mut store = subtitle_store(true);
        let other = store.available_variants().nth(1).unwrap().id();
        let changes = store.set_variant(other);
        assert!(changes.variant_changed);
        assert!(changes.changed_playlists.contains(&MediaType::Text));
        assert_eq!(changes.tracks.subtitles, TrackChange::Unchanged);
        assert_eq!(changes.tracks.audio, TrackChange::Unchanged);
    }

    #[test]
    fn probe_classification_and_unlock_report_actual_changes_only() {
        let mut store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nsegment.aac\n#EXT-X-ENDLIST\n",
            parse_url("https://example.com/direct.m3u8"),
        )
        .unwrap();
        let info = ExternalMediaInfo {
            media_type: MediaType::Audio,
            mime_type: "audio/aac".into(),
            codec: "mp4a.40.2".into(),
        };
        let changes = store.set_external_media_info(info.clone(), MediaType::Audio);
        assert_eq!(changes.changed_playlists, vec![MediaType::Audio]);
        assert_eq!(
            store.effective_audio_track_id(),
            Some(crate::bindings::formatters::DIRECT_MEDIA_AUDIO_TRACK_ID)
        );
        assert_eq!(
            store.set_external_media_info(info, MediaType::Audio),
            super::ExternalMediaInfoUpdate::default()
        );
        assert_eq!(
            store
                .set_audio_track(Some(
                    crate::bindings::formatters::DIRECT_MEDIA_AUDIO_TRACK_ID
                ))
                .unwrap()
                .tracks
                .audio,
            TrackChange::Unchanged
        );
        let mut store = subtitle_store(true);
        let variant = store.current_variant_id().unwrap();
        store.lock_variant(variant);
        assert!(store.unlock_variant());
        assert!(!store.unlock_variant());
    }

    #[test]
    fn audio_result_includes_forced_subtitle_changes_even_without_an_audio_playlist_change() {
        let mut store = PlaylistStore::try_new(br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",LANGUAGE="en",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="French",LANGUAGE="fr"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Forced English",LANGUAGE="en",AUTOSELECT=YES,FORCED=YES,URI="en-subs.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Forced French",LANGUAGE="fr",AUTOSELECT=YES,FORCED=YES,URI="fr-subs.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="audio",SUBTITLES="subs"
video.m3u8
"#, parse_url("https://example.com/master.m3u8")).unwrap();
        let variant = store.available_variants().next().unwrap().id();
        store.set_variant(variant);
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        let changes = store.set_audio_track(Some(french)).unwrap();
        assert!(!changes.audio_playlist_changed);
        assert!(changes.subtitle_playlist_changed);
        assert_eq!(store.fixed_audio_track, Some(french));
        assert_eq!(changes.tracks.audio, TrackChange::Changed(Some(french)));
        assert_eq!(
            changes.tracks.subtitles,
            TrackChange::Changed(Some(subtitle_id(&store, "Forced French")))
        );
        assert!(!changes.needs_variant_selection);
    }

    #[test]
    fn locking_variant_only_reports_logical_audio_track_changes() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="low",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en-low.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="high",NAME="English",LANGUAGE="en",URI="en-high.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="high",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="fr-high.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="low"
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,AUDIO="high"
high.m3u8
"#;
        let mut store = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let low_id = store.available_variants().next().unwrap().id();
        let high_id = store.available_variants().nth(1).unwrap().id();
        let french_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("French"))
            .unwrap()
            .id();
        store.set_variant(low_id);

        let LockVariantResponse::VariantLocked {
            changed_media_types: _,
            tracks,
            variant_changed: _,
        } = store.lock_variant(high_id)
        else {
            panic!("expected the high variant to be locked");
        };
        assert_eq!(tracks.audio, TrackChange::Changed(Some(french_id)));

        store.unlock_variant();
        store.set_audio_track(Some(french_id)).unwrap();
        let LockVariantResponse::VariantLocked {
            changed_media_types: _,
            tracks,
            variant_changed: _,
        } = store.lock_variant(high_id)
        else {
            panic!("expected the high variant to be locked");
        };
        assert_eq!(tracks.audio, TrackChange::Unchanged);
    }

    #[test]
    fn locking_variant_reports_when_the_selectable_audio_track_disappears() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="separate",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="audio.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="muxed",NAME="English",LANGUAGE="en",DEFAULT=YES
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="separate"
separate.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,AUDIO="muxed"
muxed.m3u8
"#;
        let mut store = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let separate_id = store.available_variants().next().unwrap().id();
        let muxed_id = store.available_variants().nth(1).unwrap().id();
        store.set_variant(separate_id);
        assert!(store.current_audio_track_id().is_some());

        let LockVariantResponse::VariantLocked {
            changed_media_types: _,
            tracks,
            variant_changed: _,
        } = store.lock_variant(muxed_id)
        else {
            panic!("expected the muxed variant to be locked");
        };
        assert_eq!(tracks.audio, TrackChange::Changed(None));
    }

    #[test]
    fn invalid_audio_id_before_variant_selection_preserves_preferences() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",URI="en.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="audio"
video.m3u8
"#
            .as_slice(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let english = store.audio_tracks()[0].id();
        assert!(store.current_variant_id().is_none());
        assert!(matches!(
            store.set_audio_track(Some(english)),
            Ok(ref changes) if !changes.needs_variant_selection && !changes.audio_playlist_changed
        ));
        assert!(matches!(
            store.set_audio_track(Some(u32::MAX - 1)),
            Err(TrackSelectionError::UnknownTrack)
        ));
        assert_eq!(store.fixed_audio_track_id(), Some(english));
        assert!(store.current_variant_id().is_none());
        assert!(matches!(
            store.set_audio_track(None),
            Ok(ref changes) if !changes.needs_variant_selection && !changes.audio_playlist_changed
        ));
        assert_eq!(store.fixed_audio_track_id(), None);
    }

    #[test]
    fn unavailable_audio_track_changes_preserve_selection_lock_and_support() {
        let playlist = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="en"
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,AUDIO="en"
high.m3u8
"#
            .as_slice(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 1_500.);
        let english_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("English"))
            .unwrap()
            .id();
        let french_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("French"))
            .unwrap()
            .id();
        store.set_audio_track(Some(english_id)).unwrap();
        let current_variant_id = store.current_variant_id().unwrap();
        assert!(matches!(
            store.lock_variant(current_variant_id),
            LockVariantResponse::VariantLocked { .. }
        ));
        let variant_ids: Vec<_> = store.available_variants().map(|v| v.id()).collect();
        for variant_id in variant_ids {
            store.set_variant_support(variant_id, variant_id == current_variant_id);
        }
        store.selected_media_support_resolved = true;
        let previous_support = store.variant_support.clone();
        let previous_audio_id = store.current_audio_id;
        let previous_video_id = store.current_video_id;

        for rejected_id in [french_id, u32::MAX - 1] {
            assert!(matches!(
                store.set_audio_track(Some(rejected_id)),
                Err(TrackSelectionError::NoCompatibleVariant)
                    | Err(TrackSelectionError::UnknownTrack)
            ));
            assert_eq!(store.fixed_audio_track_id(), Some(english_id));
            assert_eq!(store.current_audio_track_id(), Some(english_id));
            assert_eq!(store.current_variant_id(), Some(current_variant_id));
            assert_eq!(store.current_audio_id, previous_audio_id);
            assert_eq!(store.current_video_id, previous_video_id);
            assert!(store.is_variant_locked());
            assert_eq!(store.variant_support, previous_support);
            assert!(store.selected_media_support_resolved);
            assert_eq!(store.compatible_variants().count(), 1);
        }
    }

    #[test]
    fn fixed_audio_changes_request_a_variant_before_normalizing_muxed_playlists() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="en"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="fr"
fr-video.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let variants: Vec<_> = store.available_variants().map(|v| v.id()).collect();
        store.set_variant(variants[0]);
        let english = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("en"))
            .unwrap()
            .id();
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        for (current, requested, destination) in [
            (variants[0], french, variants[1]),
            (variants[1], english, variants[0]),
        ] {
            store.set_variant_support(current, true);
            assert!(matches!(
                store.lock_variant(current),
                LockVariantResponse::VariantLocked { .. }
            ));
            let previous_audio = store.current_audio_id;
            let previous_video = store.current_video_id;
            assert!(matches!(
                store.set_audio_track(Some(requested)),
                Ok(ref changes) if changes.needs_variant_selection && changes.variant_lock_removed
            ));
            assert_eq!(store.current_variant_id(), Some(current));
            assert_eq!(store.current_audio_id, previous_audio);
            assert_eq!(store.current_video_id, previous_video);
            assert_eq!(
                store.audio_track_id_for_variant(store.variant(current).unwrap()),
                Err(super::IncompatibleTrack)
            );
            assert_eq!(
                store.audio_track_id_for_variant(store.variant(destination).unwrap()),
                Ok(Some(requested))
            );
            assert_eq!(store.fixed_audio_track_id(), Some(requested));
            assert!(!store.is_variant_locked());
            store.set_variant(destination);
        }
    }

    #[test]
    fn compatible_switch_to_muxed_audio_is_a_playlist_update() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",DEFAULT=YES,URI="en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="French",LANGUAGE="fr"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="audio"
video.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let variant = store.available_variants().next().unwrap().id();
        store.set_variant(variant);
        assert!(store.current_audio_id.is_some());
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        assert!(matches!(
            store.set_audio_track(Some(french)),
            Ok(ref changes) if changes.audio_playlist_changed
        ));
        assert_eq!(store.current_audio_id, None);
        assert_eq!(store.current_variant_id(), Some(variant));
        assert_eq!(
            store.audio_track_id_for_variant(store.variant(variant).unwrap()),
            Ok(Some(french))
        );
    }

    #[test]
    fn missing_subtitle_playlist_does_not_itself_require_a_variant_switch() {
        let mut store = subtitle_store(false);
        let variants: Vec<_> = store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        store.set_variant(variants[2]); // This variant has no subtitle group.
        assert!(matches!(
            store.lock_variant(variants[2]),
            LockVariantResponse::VariantLocked { .. }
        ));

        // Automatic selection can legitimately produce no subtitle playlist.
        assert!(
            !store
                .set_subtitle_track(None)
                .unwrap()
                .needs_variant_selection
        );
        assert_eq!(store.current_text_id, None);
        assert_eq!(store.resolve_subtitle_selection(), Ok(None));
        assert!(store.is_variant_locked());

        // The same missing playlist now means that the fixed choice is incompatible.
        let french = subtitle_id(&store, "French");
        assert!(
            store
                .set_subtitle_track(Some(french))
                .unwrap()
                .needs_variant_selection
        );
        assert_eq!(store.current_text_id, None);
        assert_eq!(store.fixed_subtitle_track_id(), Some(french));
        assert_eq!(
            store.resolve_subtitle_selection(),
            Err(super::IncompatibleTrack)
        );
        assert!(!store.is_variant_locked());

        store.set_variant(variants[0]);
        let selected = store.current_text_id;
        assert!(selected.is_some());
        assert_eq!(store.resolve_subtitle_selection(), Ok(selected));
        assert_eq!(store.current_text_id, selected);
        assert!(
            !store
                .set_subtitle_track(Some(french))
                .unwrap()
                .needs_variant_selection
        );
        store.disable_subtitle_track();
        assert_eq!(store.resolve_subtitle_selection(), Ok(None));
        assert_eq!(store.fixed_subtitle_track_id(), Some(french));
    }

    #[test]
    fn fixed_subtitle_resolution_reports_an_incompatible_variant() {
        let mut store = subtitle_store(true);
        let french = subtitle_id(&store, "French");
        store.set_subtitle_track(Some(french)).unwrap();
        let incompatible = store.available_variants().nth(2).unwrap().id();
        assert_eq!(
            store.subtitle_track_id_for_variant(store.variant(incompatible).unwrap()),
            Err(super::IncompatibleTrack)
        );
        assert_eq!(
            store.resolve_playlist_id_for_variant(
                store.variant(incompatible).unwrap(),
                MediaType::Text
            ),
            Err(super::IncompatibleTrack)
        );
        assert_eq!(store.fixed_subtitle_track_id(), Some(french));
        store.disable_subtitle_track();
        assert_eq!(
            store.subtitle_track_id_for_variant(store.variant(incompatible).unwrap()),
            Ok(None)
        );
    }

    #[test]
    fn incompatible_audio_track_requests_selection_without_choosing_a_variant() {
        let playlist = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,URI="en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,AUTOSELECT=YES,URI="fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="en"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,AUDIO="fr"
fr-low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4000,AUDIO="fr"
fr-high.m3u8
"#.as_slice(),
            parse_url("https://example.com/master.m3u8"),
        ).unwrap();
        let mut store = playlist_store_with_variant(playlist, 1_500.);
        let current_id = store.current_variant_id().unwrap();
        let current_audio_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        assert!(matches!(
            store.lock_variant(current_id),
            LockVariantResponse::VariantLocked { .. }
        ));
        let french_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("French"))
            .unwrap()
            .id();

        // Results cached for the previous track must not reject a structurally valid change.
        let other_variant_ids: Vec<_> = store
            .available_variants()
            .filter(|variant| variant.id() != current_id)
            .map(|variant| variant.id())
            .collect();
        for variant_id in other_variant_ids {
            store.set_variant_support(variant_id, false);
        }

        assert!(matches!(
            store.set_audio_track(Some(french_id)),
            Ok(ref changes) if changes.needs_variant_selection && changes.variant_lock_removed
        ));
        assert_eq!(store.current_variant_id(), Some(current_id));
        assert_eq!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&current_audio_id)
        );
        assert!(!store.is_variant_locked());

        let compatible_variants = store.compatible_variants().collect::<Vec<_>>();
        assert_eq!(compatible_variants.len(), 2);
        assert!(compatible_variants
            .iter()
            .all(|variant| variant.id() != current_id));
        let selection = AdaptiveQualitySelector::new(3_125.)
            .select_variant(
                &compatible_variants,
                store.current_variant_id(),
                &PlaybackConditions {
                    buffer_level: Some(0.),
                    buffer_goal: 30.,
                    playback_speed: 1.,
                    abr_reference_segment_duration: [MediaType::Audio, MediaType::Video]
                        .into_iter()
                        .filter_map(|media_type| store.max_observed_target_duration(media_type))
                        .reduce(f64::max),
                },
            )
            .unwrap();
        store.set_variant(selection.best_variant_id);
        assert_eq!(store.current_variant().unwrap().bandwidth(), 2000);
        assert_eq!(store.current_audio_track_id(), Some(french_id));
        assert_ne!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&current_audio_id)
        );
    }

    #[test]
    fn unsupported_variant_requests_selection_from_remaining_candidates() {
        let playlist = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000\nhigh.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        ).unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        let unsupported_id = store.current_variant_id().unwrap();
        assert!(matches!(
            store.lock_variant(unsupported_id),
            LockVariantResponse::VariantLocked { .. }
        ));
        let ids: Vec<u32> = store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        for id in ids {
            store.set_variant_support(id, id != unsupported_id);
        }
        assert!(matches!(
            store.resolve_multivariant_audio_video_support(),
            Ok(MultivariantStartupStatus::VariantSelectionNeeded)
        ));
        assert_eq!(store.current_variant_id(), Some(unsupported_id));

        let compatible_variants = store.compatible_variants().collect::<Vec<_>>();
        assert_eq!(compatible_variants.len(), 2);
        assert!(compatible_variants
            .iter()
            .all(|variant| variant.id() != unsupported_id));
        let selection = AdaptiveQualitySelector::new(3_125.)
            .select_variant(
                &compatible_variants,
                store.current_variant_id(),
                &PlaybackConditions {
                    buffer_level: Some(0.),
                    buffer_goal: 30.,
                    playback_speed: 1.,
                    abr_reference_segment_duration: [MediaType::Audio, MediaType::Video]
                        .into_iter()
                        .filter_map(|media_type| store.max_observed_target_duration(media_type))
                        .reduce(f64::max),
                },
            )
            .unwrap();
        store.set_variant(selection.best_variant_id);
        assert_eq!(store.current_variant().unwrap().bandwidth(), 2000);
        assert!(store.is_variant_locked());

        store.set_variant_support(selection.best_variant_id, false);
        let last_id = store.compatible_variants().next().unwrap().id();
        store.set_variant_support(last_id, false);
        assert!(matches!(
            store.resolve_multivariant_audio_video_support(),
            Err(PlaylistStoreError::NoSupportedVariant)
        ));
    }

    #[test]
    fn shared_multivariant_playlist_is_selected_as_video_only() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="Main",DEFAULT=YES,AUTOSELECT=YES
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
https://example.com/media.m3u8
"#;
        let media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg.ts
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        assert_eq!(store.current_media_playlist_id(MediaType::Audio), None);
        let shared_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        store
            .update_media_playlist(
                &shared_id,
                media.as_bytes(),
                parse_url("https://example.com/media.m3u8"),
            )
            .unwrap();

        store.set_external_media_info(
            ExternalMediaInfo {
                mime_type: "video/mp4".to_string(),
                media_type: MediaType::Video,
                codec: "avc1.4d401e,mp4a.40.2".to_string(),
            },
            MediaType::Video,
        );

        assert_eq!(store.current_audio_track_id(), None);
        assert_eq!(store.current_codec(MediaType::Audio), None);
        assert_eq!(
            store.current_codec(MediaType::Video).as_deref(),
            Some("avc1.4d401e,mp4a.40.2")
        );
    }

    #[test]
    fn startup_status_waits_for_selected_multivariant_playlists_to_load() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="Main",DEFAULT=YES,AUTOSELECT=YES,URI="audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        assert!(store.current_media_playlist_id(MediaType::Audio).is_some());
        assert!(store.current_media_playlist_id(MediaType::Video).is_some());
        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::AwaitingPlaylists)
        ));
    }

    #[test]
    fn startup_probes_unknown_container_even_with_manifest_codecs() {
        let playlist = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS=\"avc1.42E01E\"\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        store
            .update_media_playlist(
                &video_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nsegment\n#EXT-X-ENDLIST\n"
                    .as_slice(),
                parse_url("https://example.com/video.m3u8"),
            )
            .unwrap();

        assert_eq!(
            store.current_codec(MediaType::Video).as_deref(),
            Some("avc1.42E01E")
        );
        assert!(store.current_mime_type(MediaType::Video).is_none());
        let Ok(StartupStatus::NeedsProbes(probes)) = store.startup_status(0.) else {
            panic!("Unknown container should require inspection");
        };
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].1, Some(MediaType::Video));
        assert_eq!(probes[0].0.url, parse_url("https://example.com/segment"));
    }

    #[test]
    fn startup_status_errors_when_loaded_multivariant_playlist_has_no_probe_segment() {
        let multivariant = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000
video.m3u8
"#;
        let empty_media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        store
            .update_media_playlist(
                &video_id,
                empty_media.as_bytes(),
                parse_url("https://example.com/video.m3u8"),
            )
            .unwrap();

        assert!(matches!(
            store.startup_status(0.),
            Err(PlaylistStoreError::NoProbeSegment)
        ));
    }

    #[test]
    fn switching_back_to_a_loaded_live_audio_playlist_keeps_its_cache() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="English",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French",AUTOSELECT=YES,URI="audio-fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#;
        let media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg.ts
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let english_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("English"))
            .unwrap()
            .id();
        let french_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("French"))
            .unwrap()
            .id();
        let english_playlist_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();

        store
            .update_media_playlist(
                &video_id,
                media.as_bytes(),
                parse_url("https://example.com/video.m3u8"),
            )
            .unwrap();
        store
            .update_media_playlist(
                &english_playlist_id,
                media.as_bytes(),
                parse_url("https://example.com/audio-en.m3u8"),
            )
            .unwrap();

        assert!(matches!(
            store.set_audio_track(Some(french_track_id)),
            Ok(ref changes) if changes.audio_playlist_changed
        ));
        let french_playlist_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        assert_ne!(french_playlist_id, english_playlist_id);
        assert!(!store.has_loaded_media_playlist(MediaType::Audio));

        store
            .update_media_playlist(
                &french_playlist_id,
                media.as_bytes(),
                parse_url("https://example.com/audio-fr.m3u8"),
            )
            .unwrap();
        assert!(store.has_loaded_media_playlist(MediaType::Audio));

        assert!(matches!(
            store.set_audio_track(Some(english_track_id)),
            Ok(ref changes) if changes.audio_playlist_changed
        ));
        assert_eq!(
            *store.current_media_playlist_id(MediaType::Audio).unwrap(),
            english_playlist_id
        );
        assert!(store.has_loaded_media_playlist(MediaType::Audio));
    }

    #[test]
    fn switching_back_to_a_loaded_vod_audio_playlist_keeps_its_cache() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="English",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French",AUTOSELECT=YES,URI="audio-fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#;
        let media = r#"#EXTM3U
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg.ts
#EXT-X-ENDLIST
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let english_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("English"))
            .unwrap()
            .id();
        let french_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == Some("French"))
            .unwrap()
            .id();
        let english_playlist_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();

        store
            .update_media_playlist(
                &video_id,
                media.as_bytes(),
                parse_url("https://example.com/video.m3u8"),
            )
            .unwrap();
        store
            .update_media_playlist(
                &english_playlist_id,
                media.as_bytes(),
                parse_url("https://example.com/audio-en.m3u8"),
            )
            .unwrap();

        assert!(matches!(
            store.set_audio_track(Some(french_track_id)),
            Ok(ref changes) if changes.audio_playlist_changed
        ));
        let french_playlist_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        assert_ne!(french_playlist_id, english_playlist_id);
        assert!(!store.has_loaded_media_playlist(MediaType::Audio));

        store
            .update_media_playlist(
                &french_playlist_id,
                media.as_bytes(),
                parse_url("https://example.com/audio-fr.m3u8"),
            )
            .unwrap();
        assert!(store.has_loaded_media_playlist(MediaType::Audio));

        assert!(matches!(
            store.set_audio_track(Some(english_track_id)),
            Ok(ref changes) if changes.audio_playlist_changed
        ));
        assert_eq!(
            *store.current_media_playlist_id(MediaType::Audio).unwrap(),
            english_playlist_id
        );
        assert!(store.has_loaded_media_playlist(MediaType::Audio));
    }

    #[test]
    fn live_expected_start_time_defaults_to_three_target_durations_from_end() {
        let media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg-1.ts
#EXTINF:4,
seg-2.ts
#EXTINF:4,
seg-3.ts
#EXTINF:4,
seg-4.ts
#EXTINF:4,
seg-5.ts
"#;

        let playlist =
            PlaylistStore::try_new(media.as_bytes(), parse_url("https://example.com/live.m3u8"))
                .unwrap();
        let store = playlist_store_with_variant(playlist, 10_000.);

        assert_eq!(store.expected_start_time(), 8.);
    }

    #[test]
    fn live_expected_start_time_honors_ext_x_start() {
        let media = r#"#EXTM3U
#EXT-X-START:TIME-OFFSET=-12,PRECISE=YES
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg-1.ts
#EXTINF:4,
seg-2.ts
#EXTINF:4,
seg-3.ts
#EXTINF:4,
seg-4.ts
#EXTINF:4,
seg-5.ts
"#;

        let playlist = PlaylistStore::try_new(
            media.as_bytes(),
            parse_url("https://example.com/live-start.m3u8"),
        )
        .unwrap();
        let store = playlist_store_with_variant(playlist, 10_000.);

        assert_eq!(store.expected_start_time(), 8.);
    }

    #[test]
    fn direct_terminal_event_playlist_is_finalized_without_changing_nature() {
        let media = r#"#EXTM3U
#EXT-X-PLAYLIST-TYPE:EVENT
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg-1.ts
#EXTINF:4,
seg-2.ts
#EXT-X-ENDLIST
"#;

        let playlist = PlaylistStore::try_new(
            media.as_bytes(),
            parse_url("https://example.com/event.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        store.set_external_media_info(
            ExternalMediaInfo {
                mime_type: "video/mp4".to_string(),
                media_type: MediaType::Video,
                codec: "avc1.4d401e,mp4a.40.2".to_string(),
            },
            MediaType::Video,
        );

        assert_eq!(
            store.playlist_type(),
            crate::bindings::PlaylistNature::Event
        );
        assert!(store.is_finalized());
    }

    #[test]
    fn live_expected_start_time_uses_earliest_safe_point_across_selected_renditions() {
        let multivariant = r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="Main",DEFAULT=YES,AUTOSELECT=YES,URI="audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#;
        let video = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXTINF:6,
video-1.ts
#EXTINF:6,
video-2.ts
#EXTINF:6,
video-3.ts
#EXTINF:6,
video-4.ts
"#;
        let audio = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
audio-1.ts
#EXTINF:4,
audio-2.ts
#EXTINF:4,
audio-3.ts
#EXTINF:4,
audio-4.ts
#EXTINF:4,
audio-5.ts
#EXTINF:4,
audio-6.ts
"#;

        let playlist = PlaylistStore::try_new(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let audio_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        store
            .update_media_playlist(
                &video_id,
                video.as_bytes(),
                parse_url("https://example.com/video.m3u8"),
            )
            .unwrap();
        store
            .update_media_playlist(
                &audio_id,
                audio.as_bytes(),
                parse_url("https://example.com/audio.m3u8"),
            )
            .unwrap();

        assert_eq!(store.expected_start_time(), 6.);
    }

    #[test]
    fn vod_expected_start_time_defaults_to_minimum_position_on_pdt_timeline() {
        let media = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-PROGRAM-DATE-TIME:2024-01-02T03:04:05.000Z
#EXTINF:4,
seg-1.ts
#EXTINF:4,
seg-2.ts
#EXT-X-ENDLIST
"#;

        let playlist =
            PlaylistStore::try_new(media.as_bytes(), parse_url("https://example.com/vod.m3u8"))
                .unwrap();
        let store = playlist_store_with_variant(playlist, 10_000.);

        assert_eq!(store.expected_start_time(), 1_704_164_645.,);
    }
}
