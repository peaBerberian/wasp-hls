use crate::{
    bindings::{jsIsTypeSupported, MediaType, PlaylistNature},
    media_element::SegmentQualityContext,
    parser::{
        AudioTrack, ExternalMediaInfo, MediaPlaylist, MediaPlaylistUpdateError, SegmentList,
        SegmentTimeInfo, TopLevelPlaylist, TopLevelPlaylistParsingError, VariantStream,
    },
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

    /// Maximum positive, finite target duration observed for this content's audio/video
    /// playlists since this store was created. Retained when playlists are deselected
    /// or their cached contents are invalidated.
    max_observed_target_duration: Option<f64>,
}

impl PlaylistStore {
    /// Parse the downloaded top-level playlist and create its store.
    pub(crate) fn try_new(data: &[u8], url: Url) -> Result<Self, PlaylistStoreError> {
        let playlist = TopLevelPlaylist::parse(data, url).map_err(PlaylistStoreError::Parsing)?;
        if matches!(&playlist, TopLevelPlaylist::Multivariant(pl) if pl.all_variants().is_empty()) {
            return Err(PlaylistStoreError::NoInitialVariant);
        }

        let max_observed_target_duration = match &playlist {
            TopLevelPlaylist::DirectMedia(playlist) => Some(playlist.playlist().target_duration())
                .filter(|duration| duration.is_finite() && *duration > 0.),
            TopLevelPlaylist::Multivariant(_) => None,
        };

        Ok(Self {
            playlist,
            current_variant_id: None,
            current_audio_id: None,
            current_video_id: None,
            fixed_audio_track: None,
            is_variant_locked: false,
            selected_media_support_resolved: false,
            variant_support: HashMap::new(),
            multivariant_media_info: HashMap::new(),
            max_observed_target_duration,
        })
    }

    /// Returns a reference to the `Url` to the top-level Playlist stored by this
    /// `PlaylistStore`.
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

    /// Returns the list of tuples listing loaded media playlists.
    ///
    /// The tuples are defined as such:
    ///   - first, the `MediaType`, each `MediaType` can be in the array only once at most.
    ///   - Second, a reference to the parsed `MediaPlaylist`
    pub(crate) fn current_media_playlists(&self) -> Vec<(MediaType, &MediaPlaylist)> {
        let mut ret = vec![];
        if let Some(pl) = self.current_media_playlist(MediaType::Audio) {
            ret.push((MediaType::Audio, pl));
        }
        if let Some(pl) = self.current_media_playlist(MediaType::Video) {
            ret.push((MediaType::Video, pl));
        }
        ret
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
        Some(id) == self.current_audio_id.as_ref() || Some(id) == self.current_video_id.as_ref()
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
                Some(MediaType::Video) | None => [self.current_video_id, self.current_audio_id],
            };
            references
                .into_iter()
                .flatten()
                .find(|reference| reference != id && self.media_playlist(reference).is_some())
        };
        let media_playlist = match &mut self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => {
                playlist.update_media_playlist(id, media_playlist_data, url, sync_playlist_id)
            }
            TopLevelPlaylist::DirectMedia(playlist) => {
                playlist.update_media_playlist(id, media_playlist_data, url, sync_playlist_id)
            }
        }?;
        let duration = media_playlist.target_duration();
        if duration.is_finite() && duration > 0. {
            self.max_observed_target_duration = Some(
                self.max_observed_target_duration
                    .map_or(duration, |known| known.max(duration)),
            );
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
                    if self.compatible_variants().is_empty() {
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
                match jsIsTypeSupported(media_info.media_type, &media_info.codec) {
                    Some(true) => Ok(StartupStatus::Ready),
                    Some(false) => Err(PlaylistStoreError::UnsupportedStartupStream),
                    None => Ok(StartupStatus::AwaitingSupportCheck),
                }
            }
            TopLevelPlaylist::Multivariant(_) => match self.resolve_multivariant_support()? {
                MultivariantStartupStatus::Ready => Ok(StartupStatus::Ready),
                MultivariantStartupStatus::AwaitingSupportCheck => {
                    Ok(StartupStatus::AwaitingSupportCheck)
                }
                MultivariantStartupStatus::VariantSelectionNeeded => {
                    Ok(StartupStatus::VariantSelectionNeeded)
                }
            },
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
    ) {
        match &mut self.playlist {
            TopLevelPlaylist::DirectMedia(playlist) => {
                let direct_media_id = *playlist.id();
                playlist.set_external_media_info(media_info);
                let next_ids = match media_type {
                    MediaType::Audio => (Some(direct_media_id), None),
                    MediaType::Video => (None, Some(direct_media_id)),
                };
                self.current_audio_id = next_ids.0;
                self.current_video_id = next_ids.1;
            }
            TopLevelPlaylist::Multivariant(_) => {
                let wanted_id = match media_type {
                    MediaType::Audio => self.current_audio_id.as_ref(),
                    MediaType::Video => self.current_video_id.as_ref(),
                };

                let Some(wanted_id) = wanted_id else {
                    return; // There's no track choosen for that type
                };
                self.multivariant_media_info.insert(*wanted_id, media_info);
            }
        }
    }

    /// Look up a variant independently of the current track choice or support filtering.
    pub(crate) fn variant(&self, variant_id: u32) -> Option<&VariantStream> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.variant(variant_id),
            TopLevelPlaylist::DirectMedia(_) => None,
        }
    }

    /// Returns vec describing all variant streams in the current `MultivariantPlaylist`,
    /// excluding variants known to be unsupported.
    pub(crate) fn available_variants(&self) -> Vec<&VariantStream> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist
                .all_variants()
                .iter()
                .filter(|v| self.variant_support(v.id()) != Some(false))
                .collect(),
            TopLevelPlaylist::DirectMedia(_) => vec![],
        }
    }

    /// Returns variants compatible with the current track choice, excluding known unsupported
    /// variants. A variant lock does not change this pool.
    pub(crate) fn compatible_variants(&self) -> Vec<&VariantStream> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => {
                if let Some(track_id) = self.fixed_audio_track {
                    // There's an explicitly set audio track id, get variants linked to it
                    playlist
                        .variants_for_audio(track_id)
                        .into_iter()
                        .filter(|v| self.variant_support(v.id()) != Some(false))
                        .collect()
                } else if let Some(track_id) = self.current_audio_track_id() {
                    // Else do in function of the current audio track
                    playlist
                        .variants_for_audio(track_id)
                        .into_iter()
                        .filter(|v| self.variant_support(v.id()) != Some(false))
                        .collect()
                } else {
                    // No current audio track: choose from anything
                    playlist
                        .all_variants()
                        .iter()
                        .filter(|v| self.variant_support(v.id()) != Some(false))
                        .collect()
                }
            }
            TopLevelPlaylist::DirectMedia(_) => vec![],
        }
    }

    /// Estimates the duration of the current content based on the currently selected audio and
    /// video media playlists.
    ///
    /// Returns `None` if there's not enough data to produce that estimate (e.g. no audio or video
    /// media playlist selected or they are not loaded).
    pub(crate) fn current_estimated_duration(&self) -> Option<f64> {
        let audio_duration = self
            .current_media_playlist(MediaType::Audio)
            .and_then(|a| a.ending());
        let video_duration = self
            .current_media_playlist(MediaType::Video)
            .and_then(|v| v.ending());
        match (audio_duration, video_duration) {
            (None, None) => None,
            (Some(a), Some(v)) => Some(f64::min(a, v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
        }
    }

    /// Maximum positive, finite target duration, in seconds, encountered across this
    /// content's audio/video playlists since this store was created.
    /// Returns `None` until a valid duration has been observed. Deselection and cache
    /// invalidation do not discard previously observed durations.
    pub(crate) fn max_observed_target_duration(&self) -> Option<f64> {
        self.max_observed_target_duration
    }

    /// Get the maximum target duration considered currently for both audio+video.
    pub(crate) fn segment_target_duration(&self) -> Option<f64> {
        let audio_td = self
            .current_media_playlist(MediaType::Audio)
            .map(|a| a.target_duration());
        let video_td = self
            .current_media_playlist(MediaType::Video)
            .map(|v| v.target_duration());
        match (audio_td, video_td) {
            (None, None) => None,
            (Some(a), Some(v)) => Some(f64::max(a, v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
        }
    }

    /// Returns the minimum reachable position seen in the last fetched media playlist.
    ///
    /// This function actually defines the minimum position as the maximum of the
    /// minimum positions reachable through all media playlists.
    ///
    /// Returns `None` if there's not enough data to produce that value (e.g. no audio
    /// or video media playlist selected or they are not loaded).
    pub(crate) fn current_estimated_minimum_position(&self) -> Option<f64> {
        let audio_duration = self
            .current_media_playlist(MediaType::Audio)
            .and_then(|a| a.beginning());
        let video_duration = self
            .current_media_playlist(MediaType::Video)
            .and_then(|v| v.beginning());
        match (audio_duration, video_duration) {
            (None, None) => None,
            (Some(a), Some(v)) => Some(f64::max(a, v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
        }
    }

    /// Returns the maximum reachable position seen in the last fetched media playlist.
    ///
    /// This function actually defines the maximum position as the minimum of the
    /// maximum positions reachable through all media playlists.
    ///
    /// Returns `None` if there's not enough data to produce that value (e.g. no audio
    /// or video media playlist selected or they are not loaded).
    pub(crate) fn current_estimated_maximum_position(&self) -> Option<f64> {
        self.current_estimated_duration()
    }

    /// Returns `true` if the currently selected media playlists use a timeline
    /// based on `EXT-X-PROGRAM-DATE-TIME`.
    pub(crate) fn uses_program_date_time(&self) -> bool {
        self.current_media_playlists()
            .iter()
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
    ///
    /// To be able to change again the variant, you can call `lock_variant` again or
    /// you can call the `unlock_variant` method.
    ///
    /// Returns whether the variant was newly locked, was already locked, or could not be found.
    pub(crate) fn lock_variant(&mut self, variant_id: u32) -> LockVariantResponse {
        let Some(current_variant_id) = self.current_variant_id else {
            return LockVariantResponse::NoVariantWithId;
        };
        let variants = self.compatible_variants();
        let pos = variants.iter().find(|x| x.id() == variant_id);

        if pos.is_some() {
            if self.is_variant_locked && current_variant_id == variant_id {
                return LockVariantResponse::AlreadyLocked;
            }
            self.is_variant_locked = true;
            let prev_track_id = self
                .fixed_audio_track
                .or_else(|| self.current_audio_track_id());
            let changed_media_types = self.set_variant(variant_id);
            let new_track_id = self
                .fixed_audio_track
                .or_else(|| self.current_audio_track_id());
            let audio_track_change = if prev_track_id == new_track_id {
                AudioTrackChange::Unchanged
            } else {
                AudioTrackChange::Changed(new_track_id)
            };
            LockVariantResponse::VariantLocked {
                changed_media_types,
                audio_track_change,
            }
        } else {
            self.is_variant_locked = false;
            LockVariantResponse::NoVariantWithId
        }
    }

    /// Disable a variant lock, previously created through the `lock_variant` method, to
    /// let adaptive streaming choose the right one instead.
    pub(crate) fn unlock_variant(&mut self) {
        self.is_variant_locked = false;
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

    /// Get segment Metadata linked to the currently loaded media playlist of the given type.
    /// To use e.g. when you want to start loading its segments.
    pub(crate) fn loaded_media_playlist_segment_info(
        &self,
        media_type: MediaType,
    ) -> Option<(&SegmentList, SegmentQualityContext)> {
        if let Some(wanted_id) = match media_type {
            MediaType::Video => &self.current_video_id,
            MediaType::Audio => &self.current_audio_id,
        } {
            self.current_media_playlist(media_type).map(|m| {
                let score = self
                    .current_variant()
                    .map(|v| v.score().unwrap_or(v.bandwidth() as f64))
                    .unwrap_or(0.);

                let context = SegmentQualityContext::new(score, *wanted_id);
                (m.segment_list(), context)
            })
        } else {
            None
        }
    }

    /// Build the segment quality context that a variant would use for the given media type.
    pub(crate) fn variant_segment_quality_context(
        &self,
        variant_id: u32,
        media_type: MediaType,
    ) -> Option<SegmentQualityContext> {
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return None;
        };
        let variant = playlist.variant(variant_id)?;
        let playlist_id = self.media_playlist_id_for_variant(variant_id, media_type)?;
        let score = variant.score().unwrap_or(variant.bandwidth() as f64);
        Some(SegmentQualityContext::new(score, playlist_id))
    }

    /// Resolve a candidate's media playlist without changing the selected variant or track.
    pub(crate) fn media_playlist_id_for_variant(
        &self,
        variant_id: u32,
        media_type: MediaType,
    ) -> Option<MediaPlaylistPermanentId> {
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return None;
        };
        let variant = playlist.variant(variant_id)?;
        let (audio_id, video_id) = Self::normalize_current_media_ids(
            playlist.audio_media_playlist_id_for(variant, self.fixed_audio_track),
            playlist.video_media_playlist_id_for(variant),
        );
        match media_type {
            MediaType::Audio => audio_id,
            MediaType::Video => video_id,
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
        media_playlists
            .iter()
            .fold(PlaylistNature::Unknown, |acc, p| match acc {
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
                let media_playlists = self.current_media_playlists();
                !media_playlists.is_empty()
                    && media_playlists.iter().all(|(_, p)| !p.may_be_refreshed())
            }
        }
    }

    /// Returns currently estimated start time in seconds at which to begin playing the content.
    ///
    /// This value may change depending on the chosen MediaPlaylist that are also loaded.
    pub(crate) fn expected_start_time(&self) -> f64 {
        let media_playlists = self.current_media_playlists();
        if media_playlists.is_empty() {
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

        if let Some(wanted_start) = media_playlists.iter().find_map(|p| p.1.wanted_start()) {
            return wanted_start;
        }

        if media_playlists.iter().all(|p| p.1.is_live()) {
            return media_playlists
                .iter()
                .filter_map(|(_, playlist)| {
                    playlist
                        .ending()
                        .map(|end| (end - (3. * playlist.target_duration())).max(0.))
                })
                .reduce(f64::min)
                .unwrap_or(0.);
        }

        self.current_estimated_minimum_position().unwrap_or(0.)
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

    /// Returns the `id` of the `AudioTrack` object explicitely selected through the
    /// `set_audio_track` API.
    ///
    /// Returns `None` if no audio track is currently selected.
    pub(crate) fn fixed_audio_track_id(&self) -> Option<u32> {
        self.fixed_audio_track
    }

    /// Returns the list of available audio tracks on the current content
    pub(crate) fn audio_tracks(&self) -> &[AudioTrack] {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.audio_tracks(),
            TopLevelPlaylist::DirectMedia(_) => &[],
        }
    }

    /// Explicitely select an `AudioTrack` based on its `id` property or disable the explicit
    /// selection of one (by giving `None` as argument).
    ///
    /// Rejects a runtime selection with no linked variant without changing any state.
    /// TODO: The name of this method is poor, as it doesn't communicate well the place that it
    /// "locks-in" an audio track, e.g. unlike `set_variant`
    pub(crate) fn set_audio_track(&mut self, track_id: Option<u32>) -> SetAudioTrackResponse {
        if self.current_variant_id.is_none() {
            self.fixed_audio_track = track_id;
            return SetAudioTrackResponse::NoUpdate;
        }
        if let (TopLevelPlaylist::Multivariant(playlist), Some(track_id)) =
            (&self.playlist, track_id)
        {
            // Support depends on the chosen track and will be checked again after a change.
            // Here, only reject tracks for which the manifest provides no candidate at all.
            if playlist.variants_for_audio(track_id).is_empty() {
                return SetAudioTrackResponse::NoCompatibleVariant;
            }
        }
        self.fixed_audio_track = track_id;
        self.clear_variant_supports();

        if let Some(variant) = self.current_variant() {
            let raw_new_audio_id = match &self.playlist {
                TopLevelPlaylist::Multivariant(playlist) => {
                    playlist.audio_media_playlist_id_for(variant, self.fixed_audio_track)
                }
                TopLevelPlaylist::DirectMedia(_) => None,
            };
            let (new_audio_id, _) =
                Self::normalize_current_media_ids(raw_new_audio_id, self.current_video_id);

            if new_audio_id.is_none() && self.current_audio_id.is_some() {
                // We may be in a case where the choosen track is not available in the
                // current variant, re-check the best variant to have with the new track.
                let variant_lock_removed = self.is_variant_locked;
                self.is_variant_locked = false;
                SetAudioTrackResponse::VariantSelectionNeeded {
                    variant_lock_removed,
                }
            } else if new_audio_id != self.current_audio_id {
                self.current_audio_id = new_audio_id;
                SetAudioTrackResponse::AudioMediaUpdate
            } else {
                SetAudioTrackResponse::NoUpdate
            }
        } else {
            SetAudioTrackResponse::NoUpdate
        }
    }

    /// Apply a specific variant selection and return the media types whose selected playlist
    /// changed as a consequence.
    pub(crate) fn set_variant(&mut self, variant_id: u32) -> Vec<MediaType> {
        let prev_audio_id = self.current_audio_id;
        let prev_video_id = self.current_video_id;
        self.set_curr_variant_and_media_id(variant_id);

        let mut changed_media_types = Vec::new();
        if self.current_audio_id != prev_audio_id {
            changed_media_types.push(MediaType::Audio);
        }
        if self.current_video_id != prev_video_id {
            changed_media_types.push(MediaType::Video);
        }
        changed_media_types
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
    fn set_curr_variant_and_media_id(&mut self, variant_id: u32) {
        let playlist = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist,
            TopLevelPlaylist::DirectMedia(_) => return,
        };
        let variant = playlist.variant(variant_id).unwrap();
        self.current_variant_id = Some(variant_id);
        let (current_audio_id, current_video_id) = Self::normalize_current_media_ids(
            playlist.audio_media_playlist_id_for(variant, self.fixed_audio_track),
            playlist.video_media_playlist_id_for(variant),
        );
        self.current_audio_id = current_audio_id;
        self.current_video_id = current_video_id;
        self.selected_media_support_resolved = false;
    }

    fn clear_variant_supports(&mut self) {
        let TopLevelPlaylist::Multivariant(_) = &self.playlist else {
            return;
        };
        self.variant_support.clear();
        self.selected_media_support_resolved = false;
    }

    fn variant_support(&self, variant_id: u32) -> Option<bool> {
        self.variant_support.get(&variant_id).copied()
    }

    #[cfg(test)]
    pub(crate) fn assume_variant_supported_for_test(&mut self, variant_id: u32) {
        self.set_variant_support(variant_id, true);
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

    /// Resolve which variants in the `MultivariantPlaylist` are supported.
    ///
    /// This allows the playlist store to know which variant can actually be relied on before
    /// playback starts.
    ///
    /// Returns `true` if support for every relevant variant could be resolved or `false` if it is
    /// still awaiting a response from JavaScript. As that response can be asynchronous it is given
    /// back to the corresponding Dispatcher's event listener function.
    ///
    /// Once that event listener has been called, `resolve_multivariant_support` can be called
    /// again, until it returns `true`.
    fn resolve_multivariant_support(
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

            let mut variant_is_supported = false;
            let is_current_variant = Some(v.id()) == current_variant_id;
            let mut saw_codec = false;

            for mt in [MediaType::Video, MediaType::Audio] {
                let codec = v.codecs(mt).or_else(|| {
                    if is_current_variant {
                        match mt {
                            MediaType::Audio => inferred_audio_codec.clone(),
                            MediaType::Video => inferred_video_codec.clone(),
                        }
                    } else {
                        None
                    }
                });

                let Some(codec) = codec else {
                    let media_required = is_current_variant
                        && match mt {
                            MediaType::Audio => current_audio_required,
                            MediaType::Video => current_video_required,
                        };
                    if media_required {
                        current_variant_resolved = false;
                        continue 'variant;
                    }
                    continue;
                };
                saw_codec = true;

                match jsIsTypeSupported(mt, &codec) {
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
            if !self.compatible_variants().is_empty() {
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

    /// Returns a reference to the MediaPlaylist currently loaded for the given `MediaType`.
    ///
    /// Returns `None` either if there's no MediaPlaylist selected for that `MediaType` or if the
    /// MediaPlaylist is not yet loaded.
    fn current_media_playlist(&self, media_type: MediaType) -> Option<&MediaPlaylist> {
        if let Some(wanted_id) = match media_type {
            MediaType::Video => &self.current_video_id,
            MediaType::Audio => &self.current_audio_id,
        } {
            match &self.playlist {
                TopLevelPlaylist::Multivariant(playlist) => playlist.media_playlist(wanted_id),
                TopLevelPlaylist::DirectMedia(playlist) => playlist.media_playlist(wanted_id),
            }
        } else {
            None
        }
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

/// Result of calling the `set_audio_track` `PlaylistStore`'s method
#[allow(clippy::enum_variant_names)]
pub(crate) enum SetAudioTrackResponse {
    /// No variant can provide the requested track. The store was left unchanged.
    NoCompatibleVariant,

    /// The audio track change led to a change of the Media Playlist for the audio.
    ///
    /// Because variants may be or not be linked to a given audio track it is also possible that
    /// the list of currently adaptively switchable variants has changed.
    AudioMediaUpdate,

    /// The current variant cannot provide the chosen track and needs reselection.
    /// `variant_lock_removed` is true when this track change removed an existing variant lock.
    VariantSelectionNeeded { variant_lock_removed: bool },

    /// No Media Playlist nor the current variant were changed due to this track change.
    ///
    /// Because variants may be or not be linked to a given audio track it is however possible that
    /// the list of currently adaptively switchable variants has changed.
    NoUpdate,
}

/// Potential audio track change caused by locking a variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioTrackChange {
    /// The audio track stayed unchanged.
    Unchanged,
    /// The audio track changed to the given track, or to no track when set to `None`.
    Changed(Option<u32>),
}

/// Return value for the `lock_variant` API.
pub(crate) enum LockVariantResponse {
    /// Error status for when no variant with the given id was found.
    NoVariantWithId,
    /// The exact same lock was already there.
    AlreadyLocked,
    /// The variant has been locked.
    VariantLocked {
        /// Media types whose selected playlist changed.
        changed_media_types: Vec<MediaType>,
        /// Whether locking changed the audio track.
        audio_track_change: AudioTrackChange,
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
}

#[cfg(test)]
mod tests {
    use super::{
        AudioTrackChange, LockVariantResponse, MultivariantStartupStatus, PlaylistStore,
        PlaylistStoreError, SetAudioTrackResponse, StartupStatus,
    };
    use crate::{
        adaptive::{AdaptiveQualitySelector, PlaybackConditions},
        bindings::MediaType,
        parser::{ExternalMediaInfo, TopLevelPlaylist},
        utils::url::Url,
    };

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
        let low = store.available_variants()[0].id();
        let high = store.available_variants()[1].id();

        assert_eq!(store.set_variant(low), vec![MediaType::Audio]);
        let low_playlist = *store.current_media_playlist_id(MediaType::Audio).unwrap();
        assert!(store.current_media_playlist_id(MediaType::Video).is_none());
        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::AwaitingPlaylists)
        ));

        assert_eq!(store.set_variant(high), vec![MediaType::Audio]);
        assert_ne!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&low_playlist)
        );
        assert!(store.current_media_playlist_id(MediaType::Video).is_none());
        assert_eq!(store.set_variant(low), vec![MediaType::Audio]);
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
        let first = store.available_variants()[0].id();
        let second = store.available_variants()[1].id();
        store.set_variant(first);
        let video_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let audio_id = *store.current_media_playlist_id(MediaType::Audio).unwrap();

        assert_eq!(store.set_variant(second), vec![MediaType::Audio]);
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
        let low = store.available_variants()[0].id();
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
            let high = store.available_variants()[1].id();
            let id = store
                .media_playlist_id_for_variant(high, media_type)
                .unwrap();
            if selected {
                // Isolate reference selection: keep the opposite playlist as the reference.
                match media_type {
                    MediaType::Video => store.current_video_id = Some(id),
                    MediaType::Audio => store.current_audio_id = Some(id),
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
            let unknown = store.available_variants()[2].id();
            let id = store
                .media_playlist_id_for_variant(unknown, MediaType::Video)
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
    fn startup_rejects_an_initial_audio_track_without_compatible_variants() {
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
        store.set_audio_track(Some(track_id));

        assert!(store.current_variant_id().is_none());
        assert!(store.compatible_variants().is_empty());
        assert!(matches!(
            store.startup_status(0.),
            Err(PlaylistStoreError::NoSupportedVariant)
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
            &store.compatible_variants(),
            store.current_variant_id(),
            &PlaybackConditions {
                buffer_level: Some(0.),
                buffer_goal: 30.,
                playback_speed: 1.,
                abr_reference_segment_duration: store.max_observed_target_duration(),
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
        let low_id = store.available_variants()[0].id();
        let high_id = store.available_variants()[1].id();
        let low_playlist_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        assert_eq!(store.max_observed_target_duration(), None);

        assert!(store
            .update_media_playlist(
                &low_playlist_id,
                b"invalid playlist".as_slice(),
                parse_url("https://example.com/low.m3u8"),
            )
            .is_err());
        assert_eq!(store.max_observed_target_duration(), None);

        store
            .update_media_playlist(
                &low_playlist_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nseg.ts\n".as_slice(),
                parse_url("https://example.com/low.m3u8"),
            )
            .unwrap();
        assert_eq!(store.max_observed_target_duration(), Some(6.));

        store.set_variant(high_id);
        assert_eq!(store.segment_target_duration(), None);
        assert_eq!(store.max_observed_target_duration(), Some(6.));
        let high_playlist_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        store
            .update_media_playlist(
                &high_playlist_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n".as_slice(),
                parse_url("https://example.com/high.m3u8"),
            )
            .unwrap();
        assert_eq!(store.segment_target_duration(), Some(4.));
        assert_eq!(store.max_observed_target_duration(), Some(6.));

        store.set_variant(low_id);
        assert!(store.has_loaded_media_playlist(MediaType::Video));
        assert_eq!(store.segment_target_duration(), Some(6.));
        assert_eq!(store.max_observed_target_duration(), Some(6.));
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
            assert_eq!(store.segment_target_duration(), Some(duration as f64));
            assert_eq!(store.max_observed_target_duration(), Some(8.));
        }
    }

    #[test]
    fn maximum_observed_target_duration_is_initialized_and_scoped_to_its_content() {
        let store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n",
            parse_url("https://example.com/direct.m3u8"),
        )
        .unwrap();
        assert_eq!(store.max_observed_target_duration(), Some(4.));
        assert_eq!(
            store.direct_media_playlist().unwrap().1.target_duration(),
            4.
        );

        let new_store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        assert_eq!(new_store.max_observed_target_duration(), None);
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
        let variants = store.available_variants();
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
        assert_eq!(store.compatible_variants().len(), 2);
        // Automatic recommendations respect the lock; explicit selection happens through locking.

        assert_eq!(store.current_variant_id(), Some(high_id));

        store.unlock_variant();
        assert!(!store.is_variant_locked());
        assert_eq!(store.current_variant_id(), Some(high_id));
        store.set_variant(low_id);
        assert_eq!(store.current_variant_id(), Some(low_id));
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
        let low_id = store.available_variants()[0].id();
        let high_id = store.available_variants()[1].id();
        let french_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == "French")
            .unwrap()
            .id();
        store.set_variant(low_id);

        let LockVariantResponse::VariantLocked {
            audio_track_change, ..
        } = store.lock_variant(high_id)
        else {
            panic!("expected the high variant to be locked");
        };
        assert_eq!(
            audio_track_change,
            AudioTrackChange::Changed(Some(french_id))
        );

        store.unlock_variant();
        store.set_audio_track(Some(french_id));
        let LockVariantResponse::VariantLocked {
            audio_track_change, ..
        } = store.lock_variant(high_id)
        else {
            panic!("expected the high variant to be locked");
        };
        assert_eq!(audio_track_change, AudioTrackChange::Unchanged);
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
        let separate_id = store.available_variants()[0].id();
        let muxed_id = store.available_variants()[1].id();
        store.set_variant(separate_id);
        assert!(store.current_audio_track_id().is_some());

        let LockVariantResponse::VariantLocked {
            audio_track_change, ..
        } = store.lock_variant(muxed_id)
        else {
            panic!("expected the muxed variant to be locked");
        };
        assert_eq!(audio_track_change, AudioTrackChange::Changed(None));
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
            .find(|track| track.name() == "English")
            .unwrap()
            .id();
        let french_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == "French")
            .unwrap()
            .id();
        store.set_audio_track(Some(english_id));
        let current_variant_id = store.current_variant_id().unwrap();
        assert!(matches!(
            store.lock_variant(current_variant_id),
            LockVariantResponse::VariantLocked { .. }
        ));
        let variant_ids: Vec<_> = store.available_variants().iter().map(|v| v.id()).collect();
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
                SetAudioTrackResponse::NoCompatibleVariant
            ));
            assert_eq!(store.fixed_audio_track_id(), Some(english_id));
            assert_eq!(store.current_audio_track_id(), Some(english_id));
            assert_eq!(store.current_variant_id(), Some(current_variant_id));
            assert_eq!(store.current_audio_id, previous_audio_id);
            assert_eq!(store.current_video_id, previous_video_id);
            assert!(store.is_variant_locked());
            assert_eq!(store.variant_support, previous_support);
            assert!(store.selected_media_support_resolved);
            assert_eq!(store.compatible_variants().len(), 1);
        }
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
            .find(|track| track.name() == "French")
            .unwrap()
            .id();

        // Results cached for the previous track must not reject a structurally valid change.
        let other_variant_ids: Vec<_> = store
            .available_variants()
            .iter()
            .filter(|variant| variant.id() != current_id)
            .map(|variant| variant.id())
            .collect();
        for variant_id in other_variant_ids {
            store.set_variant_support(variant_id, false);
        }

        assert!(matches!(
            store.set_audio_track(Some(french_id)),
            SetAudioTrackResponse::VariantSelectionNeeded {
                variant_lock_removed: true
            }
        ));
        assert_eq!(store.current_variant_id(), Some(current_id));
        assert_eq!(
            store.current_media_playlist_id(MediaType::Audio),
            Some(&current_audio_id)
        );
        assert!(!store.is_variant_locked());

        let compatible_variants = store.compatible_variants();
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
                    abr_reference_segment_duration: store.max_observed_target_duration(),
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
            .iter()
            .map(|variant| variant.id())
            .collect();
        for id in ids {
            store.set_variant_support(id, id != unsupported_id);
        }
        assert!(matches!(
            store.resolve_multivariant_support(),
            Ok(MultivariantStartupStatus::VariantSelectionNeeded)
        ));
        assert_eq!(store.current_variant_id(), Some(unsupported_id));

        let compatible_variants = store.compatible_variants();
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
                    abr_reference_segment_duration: store.max_observed_target_duration(),
                },
            )
            .unwrap();
        store.set_variant(selection.best_variant_id);
        assert_eq!(store.current_variant().unwrap().bandwidth(), 2000);
        assert!(store.is_variant_locked());

        store.set_variant_support(selection.best_variant_id, false);
        let last_id = store.compatible_variants()[0].id();
        store.set_variant_support(last_id, false);
        assert!(matches!(
            store.resolve_multivariant_support(),
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
            .find(|track| track.name() == "English")
            .unwrap()
            .id();
        let french_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == "French")
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
            SetAudioTrackResponse::AudioMediaUpdate
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
            SetAudioTrackResponse::AudioMediaUpdate
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
            .find(|track| track.name() == "English")
            .unwrap()
            .id();
        let french_track_id = store
            .audio_tracks()
            .iter()
            .find(|track| track.name() == "French")
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
            SetAudioTrackResponse::AudioMediaUpdate
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
            SetAudioTrackResponse::AudioMediaUpdate
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
