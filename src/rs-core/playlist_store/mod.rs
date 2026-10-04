use crate::{
    bindings::{jsIsTypeSupported, MediaType, PlaylistNature},
    media_element::SegmentQualityContext,
    parser::{
        AudioTrack, ExternalMediaInfo, MediaPlaylist, MediaPlaylistUpdateError, SegmentList,
        SegmentTimeInfo, TopLevelPlaylist, VariantStream,
    },
    utils::{logger::*, url::Url},
};
use std::{cmp::Ordering, collections::HashMap, io::BufRead};

use crate::parser::ByteRange;
pub(crate) use crate::parser::MediaPlaylistPermanentId;

/// Stores information about the current loaded Multivariant Playlist and its sub-playlists:
///   - Information on the Multivariant Playlist itself.
///   - On the current variant selected.
///   - Information on the different audio and video Media Playlists selected.
pub(crate) struct PlaylistStore {
    /// Representation of the top-level playlist of the currently loaded HLS content.
    playlist: TopLevelPlaylist,

    /// `id` of the currently chosen variant.
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

    /// Before actually playing a multivariant content, supported codecs need to be checked
    /// to avoid mistakenly choosing an unsupported variant.
    ///
    /// This bool is set to `true` once the currently selected media has had its support
    /// resolved.
    multivariant_support_resolved: bool,

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
    /// Create a `PlaylistStore`.
    pub(crate) fn try_new(playlist: TopLevelPlaylist) -> Result<Self, PlaylistStoreError> {
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
            multivariant_support_resolved: false,
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
    pub(crate) fn media_type_for(&self, id: &MediaPlaylistPermanentId) -> Option<MediaType> {
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
        self.update_media_playlist_with_reference(id, media_playlist_data, url, None)
    }

    /// Use an already active request source to align a newly loaded live playlist.
    pub(crate) fn update_media_playlist_with_reference(
        &mut self,
        id: &MediaPlaylistPermanentId,
        media_playlist_data: impl BufRead,
        url: Url,
        reference: Option<MediaPlaylistPermanentId>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        // Update logic may optionally need a reference for Audio/Video sync
        let sync_playlist_id = reference.or_else(|| {
            if !self.is_current_media_playlist(id) {
                // An alternate request source must share the active playback timeline.
                self.current_video_id.or(self.current_audio_id)
            } else {
                match self.media_type_for(id) {
                    Some(MediaType::Audio)
                        if self
                            .current_video_id
                            .as_ref()
                            .is_some_and(|video_id| video_id != id) =>
                    {
                        self.current_video_id
                    }
                    Some(MediaType::Video)
                        if self
                            .current_audio_id
                            .as_ref()
                            .is_some_and(|audio_id| audio_id != id) =>
                    {
                        self.current_audio_id
                    }
                    _ => None,
                }
            }
        });
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
                        self.current_media_playlist(media_type).map(|playlist| {
                            playlist.mime_type(media_type).unwrap_or("").to_string()
                        })
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
                            && self.current_codec(*media_type).is_none()
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
                self.set_audio_id(next_ids.0);
                self.set_video_id(next_ids.1);
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

    /// Apply an automatic variant choice unless a variant is manually locked.
    pub(crate) fn update_adaptive_variant(&mut self, variant_id: u32) -> VariantUpdateResult {
        if self.is_variant_locked() {
            VariantUpdateResult::Unchanged
        } else {
            self.update_variant(variant_id)
        }
    }

    /// Force a given variant and prevent it from changing, by communicating its `id`.
    ///
    /// To be able to change again the variant, you can call `lock_variant` again or
    /// you can call the `unlock_variant` method.
    ///
    /// The returned option is `None` if the `variant_id` given is not found to correspond
    /// to any existing variant. It contains the corresponding update when set to the `Some`
    /// variant.
    pub(crate) fn lock_variant(&mut self, variant_id: u32) -> LockVariantResponse {
        if self.current_variant_id.is_none() {
            return LockVariantResponse::NoVariantWithId;
        }
        let variants = self.compatible_variants();
        let pos = variants.iter().find(|x| x.id() == variant_id);

        if pos.is_some() {
            self.is_variant_locked = true;
            let prev_track_id = self
                .fixed_audio_track
                .or_else(|| self.current_audio_track_id());
            let updates = self.update_variant(variant_id);
            let new_track_id = self
                .fixed_audio_track
                .or_else(|| self.current_audio_track_id());
            let audio_track_change = match (prev_track_id, new_track_id) {
                (Some(prev_id), Some(new_id)) => {
                    if prev_id == new_id {
                        Some(new_id)
                    } else {
                        None
                    }
                }
                (None, Some(new_id)) => Some(new_id),
                _ => None,
            };
            LockVariantResponse::VariantLocked {
                updates,
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
    pub(crate) fn media_playlist_id_for(
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

    /// Returns `true` if `seg_start` corresponds to the last segment of the currently loaded
    /// playlist for the given `MediaType`.
    pub(crate) fn is_last_media_segment(&self, media_type: MediaType, seg_start: f64) -> bool {
        self.current_media_playlist(media_type)
            .is_some_and(|playlist| {
                playlist.is_ended()
                    && playlist
                        .segment_list()
                        .media()
                        .last()
                        .map(|segment| segment.start() == seg_start)
                        .unwrap_or(false)
            })
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

                let context = SegmentQualityContext::new(score, wanted_id.as_u32());
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
        let (audio_id, video_id) = Self::normalize_current_media_ids(
            playlist.audio_media_playlist_id_for(variant, self.fixed_audio_track),
            playlist.video_media_playlist_id_for(variant),
        );
        let media_id = match media_type {
            MediaType::Audio => audio_id,
            MediaType::Video => video_id,
        }?;
        let score = variant.score().unwrap_or(variant.bandwidth() as f64);
        Some(SegmentQualityContext::new(score, media_id.as_u32()))
    }

    /// Resolve a request source without changing the preferred variant or track selection.
    pub(crate) fn variant_media_playlist_id_for(
        &self,
        variant_id: u32,
        media_type: MediaType,
    ) -> Option<MediaPlaylistPermanentId> {
        let TopLevelPlaylist::Multivariant(playlist) = &self.playlist else {
            return None;
        };
        let variant = playlist.variant(variant_id)?;
        let (audio, video) = Self::normalize_current_media_ids(
            playlist.audio_media_playlist_id_for(variant, self.fixed_audio_track),
            playlist.video_media_playlist_id_for(variant),
        );
        match media_type {
            MediaType::Audio => audio,
            MediaType::Video => video,
        }
    }

    pub(crate) fn media_playlist_by_id(
        &self,
        id: &MediaPlaylistPermanentId,
    ) -> Option<&MediaPlaylist> {
        match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist.media_playlist(id),
            TopLevelPlaylist::DirectMedia(playlist) => playlist.media_playlist(id),
        }
    }

    pub(crate) fn is_known_supported_variant(&self, id: u32) -> bool {
        self.variant_support(id) == Some(true)
    }

    /// A live playlist that stopped being maintained must be fetched before reuse.
    pub(crate) fn invalidate_inactive_live_playlist(&mut self, id: MediaPlaylistPermanentId) {
        if self.is_current_media_playlist(&id) {
            return;
        }
        if let TopLevelPlaylist::Multivariant(playlist) = &mut self.playlist {
            if playlist
                .media_playlist(&id)
                .is_some_and(MediaPlaylist::is_live)
            {
                playlist.clear_media_playlist(&id);
            }
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
                self.set_audio_id(new_audio_id);
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

    /// When switching back to a cached live sliding media playlist that we stopped refreshing,
    /// force a reload instead of relying on its potentially stale value.
    ///
    /// WHY: We do that to simplify state handling for the rest of the player.
    /// If we kept its old value while it refreshed, many of its current attributes like the
    /// availability of each segments, the minimum/maximum positions of a playlist etc. would have
    /// ro be checked for trustedness first (e.g. "are they old or fresh values?").
    /// It would be doable, but clearing up stale playlist is just a much easier state to manage
    /// for what is an infrequent event anyway.
    /// We do miss potential context about an older load of it, but that's rarely useful.
    fn check_new_playlist_for_staleness(
        &mut self,
        previous_id: Option<MediaPlaylistPermanentId>,
        next_id: Option<MediaPlaylistPermanentId>,
    ) {
        let Some(id) = next_id else {
            return;
        };
        if next_id == previous_id {
            return;
        }

        match &mut self.playlist {
            TopLevelPlaylist::Multivariant(playlist)
                if playlist
                    .media_playlist(&id)
                    .is_some_and(MediaPlaylist::is_live) =>
            {
                playlist.clear_media_playlist(&id);
            }
            TopLevelPlaylist::Multivariant(_) | TopLevelPlaylist::DirectMedia(_) => {}
        }
    }

    fn set_audio_id(&mut self, new_audio_id: Option<MediaPlaylistPermanentId>) {
        let prev_audio_id = self.current_audio_id;
        self.current_audio_id = new_audio_id;
        self.check_new_playlist_for_staleness(prev_audio_id, self.current_audio_id);
    }

    fn set_video_id(&mut self, new_video_id: Option<MediaPlaylistPermanentId>) {
        let prev_video_id = self.current_video_id;
        self.current_video_id = new_video_id;
        self.check_new_playlist_for_staleness(prev_video_id, self.current_video_id);
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

    /// Apply a chosen variant and describe the resulting playlist changes.
    fn update_variant(&mut self, new_id: u32) -> VariantUpdateResult {
        let playlist = match &self.playlist {
            TopLevelPlaylist::Multivariant(playlist) => playlist,
            TopLevelPlaylist::DirectMedia(_) => return VariantUpdateResult::Unchanged,
        };
        if Some(new_id) != self.current_variant_id {
            let prev_bandwidth = self.current_variant().map(|v| v.bandwidth());
            let new_bandwidth = playlist.variant(new_id).map(|v| v.bandwidth());
            let prev_audio_id = self.current_audio_id;
            let prev_video_id = self.current_video_id;
            self.set_curr_variant_and_media_id(new_id.to_owned());

            let mut updates = vec![];
            if self.current_audio_id != prev_audio_id {
                updates.push(MediaType::Audio);
            }
            if self.current_video_id != prev_video_id {
                updates.push(MediaType::Video);
            }
            match (prev_bandwidth, new_bandwidth) {
                (Some(p), Some(n)) => match p.cmp(&n) {
                    Ordering::Greater => VariantUpdateResult::Worsened(updates),
                    Ordering::Equal => VariantUpdateResult::EqualOrUnknown(updates),
                    Ordering::Less => VariantUpdateResult::Improved(updates),
                },
                _ => VariantUpdateResult::EqualOrUnknown(updates),
            }
        } else {
            VariantUpdateResult::Unchanged
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
        self.set_audio_id(current_audio_id);
        self.set_video_id(current_video_id);
        self.multivariant_support_resolved = false;
    }

    fn clear_variant_supports(&mut self) {
        let TopLevelPlaylist::Multivariant(_) = &self.playlist else {
            return;
        };
        self.variant_support.clear();
        self.multivariant_support_resolved = false;
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
        let playlist_id = self.media_playlist_id_for(media_type)?;
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
        if self.multivariant_support_resolved {
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
                self.multivariant_support_resolved = false;
                return Ok(MultivariantStartupStatus::VariantSelectionNeeded);
            } else {
                log_error!("PS: No supported variant in the given MultivariantPlaylist");
                return Err(PlaylistStoreError::NoSupportedVariant);
            }
        }

        let is_multivariant_support_resolved =
            current_variant_resolved && curr_variant_support == Some(true);

        self.multivariant_support_resolved = is_multivariant_support_resolved;

        if is_multivariant_support_resolved {
            log_info!("PS: Support has been resolved for the current multivariant startup path");
        } else {
            log_info!("PS: Current multivariant startup path still needs support resolution");
        }
        if is_multivariant_support_resolved {
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

/// Response returned by `PlaylistStore` method which may update the current
/// variant and as a consequence, linked media playlists.
pub enum VariantUpdateResult {
    /// No MediaPlaylist was updated
    Unchanged,

    /// At least one MediaPlaylist was updated for a better one.
    ///
    /// The `MediaType` in argument designates the media type whose playlist
    /// was updated. There can only be one item of the same type in that
    /// vector.
    Improved(Vec<MediaType>),

    /// At least one MediaPlaylist was updated for a worse one.
    ///
    /// The `MediaType` in argument designates the media type whose playlist
    /// was updated. There can only be one item of the same type in that
    /// vector.
    Worsened(Vec<MediaType>),

    /// At least one MediaPlaylist was updated, but for either an as-good or
    /// for a quality that could not be compared.
    ///
    /// The `MediaType` in argument designates the media type whose playlist
    /// was updated. There can only be one item of the same type in that
    /// vector.
    EqualOrUnknown(Vec<MediaType>),
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

/// Return value for the `lock_variant` API.
pub(crate) enum LockVariantResponse {
    /// Error status for when no variant with the given id was found.
    NoVariantWithId,
    /// The variant has been locked.
    VariantLocked {
        /// Side-effects of that change, in terms of media playlists loaded in consequence
        updates: VariantUpdateResult,
        /// If `Some(true)`, the new audio media playlist is considered as not part of the same
        /// "audio track" as the previous one.
        audio_track_change: Option<u32>,
    },
}

use thiserror::Error;

/// Error encountered when creating/updating a PlaylistStore
#[derive(Error, Debug)]
pub(crate) enum PlaylistStoreError {
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
        LockVariantResponse, MultivariantStartupStatus, PlaylistStore, PlaylistStoreError,
        SetAudioTrackResponse, StartupStatus,
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
    fn alternate_request_sources_preserve_selection_and_align_to_the_active_live_timeline() {
        let parsed = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS=\"avc1.42E01E\"\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS=\"avc1.42E01E\"\nhigh.m3u8\n".as_slice(),
            parse_url("https://example.com/master.m3u8"),
        ).unwrap();
        let mut store = PlaylistStore::try_new(parsed).unwrap();
        let low = store.available_variants()[0].id();
        let high = store.available_variants()[1].id();
        store.set_variant(low);
        let low_id = store
            .variant_media_playlist_id_for(low, MediaType::Video)
            .unwrap();
        let high_id = store
            .variant_media_playlist_id_for(high, MediaType::Video)
            .unwrap();
        store.update_media_playlist(&low_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:100\n#EXTINF:4,\na.m4s\n#EXTINF:4,\nb.m4s\n".as_slice(),
            parse_url("https://example.com/low.m3u8")).unwrap();
        store.update_media_playlist(&low_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:101\n#EXTINF:4,\nb.m4s\n#EXTINF:4,\nc.m4s\n".as_slice(),
            parse_url("https://example.com/low.m3u8")).unwrap();
        store.set_variant(high);
        store.update_media_playlist_with_reference(&high_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:101\n#EXTINF:4,\nb-high.m4s\n#EXTINF:4,\nc-high.m4s\n".as_slice(),
            parse_url("https://example.com/high.m3u8"), Some(low_id)).unwrap();
        assert_eq!(
            store
                .media_playlist_by_id(&high_id)
                .unwrap()
                .segment_list()
                .media()[0]
                .start(),
            4.
        );
        assert_eq!(
            store
                .media_playlist_by_id(&low_id)
                .unwrap()
                .segment_list()
                .media()[0]
                .start(),
            4.
        );
        store.lock_variant(high);
        assert_eq!(
            store.variant_media_playlist_id_for(low, MediaType::Video),
            Some(low_id)
        );
        assert_eq!(store.current_variant_id(), Some(high));
        assert!(store.is_variant_locked());
        store.invalidate_inactive_live_playlist(low_id);
        assert!(store.media_playlist_by_id(&low_id).is_none());
        assert!(store.media_playlist_by_id(&high_id).is_some());
    }

    #[test]
    fn startup_rejects_an_initial_audio_track_without_compatible_variants() {
        let playlist = TopLevelPlaylist::parse(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"
video.m3u8
"#
            .as_slice(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = PlaylistStore::try_new(playlist).unwrap();
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
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = PlaylistStore::try_new(playlist).unwrap();

        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::VariantSelectionNeeded)
        ));
    }

    fn playlist_store_with_variant(
        playlist: TopLevelPlaylist,
        initial_bandwidth: f64,
    ) -> PlaylistStore {
        let mut store = PlaylistStore::try_new(playlist).unwrap();
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
            store.update_adaptive_variant(selection.best_variant_id);
        }
        store
    }

    #[test]
    fn maximum_observed_target_duration_survives_live_cache_invalidation() {
        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000\nhigh.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 1_500.);
        let low_id = store.available_variants()[0].id();
        let high_id = store.available_variants()[1].id();
        let low_playlist_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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
        let high_playlist_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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
        assert!(!store.has_loaded_media_playlist(MediaType::Video));
        assert_eq!(store.segment_target_duration(), None);
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
        let direct_playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n",
            parse_url("https://example.com/direct.m3u8"),
        )
        .unwrap();
        let store = PlaylistStore::try_new(direct_playlist).unwrap();
        assert_eq!(store.max_observed_target_duration(), Some(4.));
        assert_eq!(
            store.direct_media_playlist().unwrap().1.target_duration(),
            4.
        );

        let playlist = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n",
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let new_store = PlaylistStore::try_new(playlist).unwrap();
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
        let playlist = TopLevelPlaylist::parse(
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
        assert_eq!(store.compatible_variants().len(), 2);
        store.update_adaptive_variant(low_id);

        assert_eq!(store.current_variant_id(), Some(high_id));

        store.unlock_variant();
        assert!(!store.is_variant_locked());
        assert_eq!(store.current_variant_id(), Some(high_id));
        store.update_adaptive_variant(low_id);
        assert_eq!(store.current_variant_id(), Some(low_id));
    }

    #[test]
    fn unavailable_audio_track_changes_preserve_selection_lock_and_support() {
        let playlist = TopLevelPlaylist::parse(
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
        store.multivariant_support_resolved = true;
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
            assert!(store.multivariant_support_resolved);
            assert_eq!(store.compatible_variants().len(), 1);
        }
    }

    #[test]
    fn incompatible_audio_track_requests_selection_without_choosing_a_variant() {
        let playlist = TopLevelPlaylist::parse(
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
        let current_audio_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();
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
            store.media_playlist_id_for(MediaType::Audio),
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
        store.update_adaptive_variant(selection.best_variant_id);
        assert_eq!(store.current_variant().unwrap().bandwidth(), 2000);
        assert_eq!(store.current_audio_track_id(), Some(french_id));
        assert_ne!(
            store.media_playlist_id_for(MediaType::Audio),
            Some(&current_audio_id)
        );
    }

    #[test]
    fn unsupported_variant_requests_selection_from_remaining_candidates() {
        let playlist = TopLevelPlaylist::parse(
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        assert_eq!(store.media_playlist_id_for(MediaType::Audio), None);
        let shared_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        assert!(store.media_playlist_id_for(MediaType::Audio).is_some());
        assert!(store.media_playlist_id_for(MediaType::Video).is_some());
        assert!(matches!(
            store.startup_status(0.),
            Ok(StartupStatus::AwaitingPlaylists)
        ));
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);
        let video_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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
    fn switching_back_to_a_loaded_live_audio_playlist_invalidates_its_cache() {
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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
        let english_playlist_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();

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
        let french_playlist_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();
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
            *store.media_playlist_id_for(MediaType::Audio).unwrap(),
            english_playlist_id
        );
        assert!(!store.has_loaded_media_playlist(MediaType::Audio));
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
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
        let english_playlist_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();

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
        let french_playlist_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();
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
            *store.media_playlist_id_for(MediaType::Audio).unwrap(),
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
            TopLevelPlaylist::parse(media.as_bytes(), parse_url("https://example.com/live.m3u8"))
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

        let playlist = TopLevelPlaylist::parse(
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

        let playlist = TopLevelPlaylist::parse(
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

        let playlist = TopLevelPlaylist::parse(
            multivariant.as_bytes(),
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let mut store = playlist_store_with_variant(playlist, 10_000.);

        let video_id = *store.media_playlist_id_for(MediaType::Video).unwrap();
        let audio_id = *store.media_playlist_id_for(MediaType::Audio).unwrap();
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
            TopLevelPlaylist::parse(media.as_bytes(), parse_url("https://example.com/vod.m3u8"))
                .unwrap();
        let store = playlist_store_with_variant(playlist, 10_000.);

        assert_eq!(store.expected_start_time(), 1_704_164_645.,);
    }
}
