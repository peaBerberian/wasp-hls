use crate::{
    bindings::{jsIsTypeSupported, MediaType},
    media_element::SegmentQualityContext,
    parser::{
        AudioTrack, MediaPlaylist, MediaPlaylistUpdateError, MultivariantPlaylist, SegmentList,
        VariantStream,
    },
    utils::url::Url,
    Logger,
};
use std::{cmp::Ordering, io::BufRead};

pub(crate) use crate::parser::MediaPlaylistPermanentId;

/// Stores information about the current loaded Multivariant Playlist and its sub-playlists:
///   - Information on the Multivariant Playlist itself.
///   - On the current variant selected.
///   - Information on the different audio and video Media Playlists selected.
pub(crate) struct PlaylistStore {
    /// A struct representing the "Multivariant Playlist", a.k.a. "Master Playlist" of
    /// the currently loaded HLS content.
    playlist: MultivariantPlaylist,

    /// `id` of the currently chosen variant.
    curr_variant_id: u32,

    /// Chosen playlist for video.
    ///
    /// Also concerns playlist containing both audio and video.
    ///
    /// Set to `None` if no video playlist is chosen.
    curr_video_id: Option<MediaPlaylistPermanentId>,

    /// Chosen playlist for audio.
    ///
    /// Set to `None` if no audio playlist is chosen.
    curr_audio_id: Option<MediaPlaylistPermanentId>,

    /// `id` identifier for the currently-selected audio track. `None` if no audio track is
    /// explicitely selected.
    ///
    /// Note that unlike `curr_video_id` and `curr_audio_id`, this identifier is not linked to a
    /// Playlist but to the `id` of the track itself.
    curr_audio_track: Option<u32>,

    /// If `true` a variant is being manually locked and as such, cannot change.
    is_variant_locked: bool,

    /// Store the last communicated bandwidth
    last_bandwidth: f64,

    /// Before actually playing a content, supported codecs need to be checked
    /// to avoid mistakenly choosing an unsupported codec.
    ///
    /// This bool is set to `true` only once ALL codecs in the
    /// `MultivariantPlaylist` have been properly checked.
    codecs_checked: bool,
}

impl PlaylistStore {
    /// Create a new `PlaylistStore` based on the given parsed `MultivariantPlaylist`.
    ///
    /// Automatically selects the variant with the highest quality (or score if defined) on call.
    /// Please call `update_curr_bandwidth` to select a variant based on an actual criteria.
    pub(crate) fn try_new(
        playlist: MultivariantPlaylist,
        initial_bandwidth: f64,
    ) -> Result<Self, PlaylistStoreError> {
        Logger::debug(&format!(
            "PS: Creating new PlaylistStore (bw: {initial_bandwidth})"
        ));
        let variants = playlist.all_variants();
        let initial_variant =
            if let Some(variant_id) = best_variant_id(variants.iter(), initial_bandwidth) {
                playlist.variant(variant_id).unwrap()
            } else if let Some(variant_id) = fallback_variant_id(variants.iter()) {
                Logger::info("PS: Found no bandwidth-compatible variant amongst all variants");
                playlist.variant(variant_id).unwrap()
            } else {
                Logger::error("PS: Found no variant in the given MultivariantPlaylist");
                return Err(PlaylistStoreError::NoInitialVariant);
            };
        let curr_variant_id = initial_variant.id();
        let curr_video_id = playlist.video_media_playlist_id_for(initial_variant);
        let curr_audio_id = playlist.audio_media_playlist_id_for(initial_variant, None);

        Ok(Self {
            playlist,
            curr_variant_id,
            curr_audio_id,
            curr_video_id,
            curr_audio_track: None,
            is_variant_locked: false,
            last_bandwidth: 0.,
            codecs_checked: false,
        })
    }

    /// Returns a reference to the `Url` to the Multivariant Playlist stored by this
    /// `PlaylistStore`.
    pub(crate) fn url(&self) -> &Url {
        self.playlist.url()
    }

    /// Check which codecs present in the `MultivariantPlaylist` are supported.
    ///
    /// This allows the playlist store to know which variant can actually be relied on.
    /// As such you should be extra careful when using the `PlaylistStore` before that check has
    /// been completely done.
    ///
    /// Returns `true` if all codecs in the `MultivariantPlaylist` could have been checked or
    /// `false` if it still await a response from JavaScript. As that response can be asynchronous
    /// it is given back to the corresponding Dispatcher's event listener function.
    ///
    /// Once that even listener has been called, `check_codecs` can be called again, until it
    /// returns `true`.
    pub(crate) fn check_codecs(&mut self) -> Result<bool, PlaylistStoreError> {
        if self.codecs_checked {
            return Ok(true);
        }

        let mut are_all_codecs_checked = true;
        self.playlist.variants_mut().iter_mut().for_each(|v| {
            if v.supported().is_some() {
                return;
            }
            [MediaType::Video, MediaType::Audio]
                .into_iter()
                .for_each(|mt| {
                    if let Some(codec) = v.codecs(mt) {
                        if let Some(is_supported) = jsIsTypeSupported(mt, &codec) {
                            v.update_support(is_supported);
                        } else {
                            are_all_codecs_checked = false;
                        }
                    }
                });
        });
        self.codecs_checked = are_all_codecs_checked;

        if are_all_codecs_checked {
            Logger::info("PS: All codecs have been checked");
            let curr_variant_still_here = self
                .playlist
                .supported_variants()
                .iter()
                .any(|v| v.id() == self.curr_variant_id);

            if !curr_variant_still_here {
                let new_variant_id = self.playlist.supported_variants().get(0).map(|v| v.id());
                if let Some(variant_id) = new_variant_id {
                    self.set_curr_variant_and_media_id(variant_id);
                } else {
                    Logger::error("PS: No supported variant in the given MultivariantPlaylist");
                    return Err(PlaylistStoreError::NoSupportedVariant);
                }
            }
        } else {
            Logger::info("PS: Some Playlist codecs need to be asynchronously checked");
        }
        Ok(are_all_codecs_checked)
    }

    /// Returns the list of tuples listing loaded media playlists.
    ///
    /// The tuples are defined as such:
    ///   - first, the `MediaType`, each `MediaType` can be in the array only once at most.
    ///   - Second, a reference to the parsed `MediaPlaylist`
    pub(crate) fn curr_media_playlists(&self) -> Vec<(MediaType, &MediaPlaylist)> {
        let mut ret = vec![];
        if let Some(pl) = self.curr_media_playlist(MediaType::Audio) {
            ret.push((MediaType::Audio, pl));
        }
        if let Some(pl) = self.curr_media_playlist(MediaType::Video) {
            ret.push((MediaType::Video, pl));
        }
        ret
    }

    pub(crate) fn is_curr_media_playlist(&self, id: &MediaPlaylistPermanentId) -> bool {
        Some(id) == self.curr_audio_id.as_ref() || Some(id) == self.curr_video_id.as_ref()
    }

    /// Returns `true` if the current playlist linked to the given `MediaType` has been loaded.
    ///
    /// Returns `false` either if it has not been loaded yet or if there's no media playlist for
    /// that `MediaType`.
    /// You can call `has_media_type` to know if there's a playlist to load for a given
    /// `MediaType`.
    fn is_media_playlist_ready(&self, media_type: MediaType) -> bool {
        self.curr_media_playlist(media_type).is_some()
    }

    /// Returns `true` only if all media playlists currently selected have been loaded.
    pub(crate) fn are_playlists_ready(&self) -> bool {
        [MediaType::Audio, MediaType::Video]
            .into_iter()
            .all(|t| !self.has_media_type(t) || self.is_media_playlist_ready(t))
    }

    /// Returns true if a MediaPlaylist for the given `MediaType` has been selected, regardless if
    /// that playlist has been loaded or not.
    pub(crate) fn has_media_type(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.curr_audio_id.is_some(),
            MediaType::Video => self.curr_video_id.is_some(),
        }
    }

    /// Initialize or update a `MediaPlaylist`, based on its `MediaPlaylistPermanentId`.
    pub(crate) fn update_media_playlist(
        &mut self,
        id: &MediaPlaylistPermanentId,
        media_playlist_data: impl BufRead,
        url: Url,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        self.playlist
            .update_media_playlist(id, media_playlist_data, url)
    }

    /// Returns vec describing all available variant streams in the current MultivariantPlaylist.
    pub(crate) fn supported_variants(&self) -> Vec<&VariantStream> {
        self.playlist.supported_variants()
    }

    /// Returns vec describing all available variant streams in the current MultivariantPlaylist.
    pub(crate) fn variants_for_curr_track(&self) -> Vec<&VariantStream> {
        if let Some(track_id) = self.curr_audio_track {
            self.playlist.supported_variants_for_audio(track_id)
        } else if let Some(track_id) = self.curr_audio_track_id() {
            self.playlist.supported_variants_for_audio(track_id)
        } else {
            self.supported_variants()
        }
    }

    /// Estimates the duration of the current content based on the currently selected audio and
    /// video media playlists.
    ///
    /// Returns `None` if there's not enough data to produce that estimate (e.g. no audio or video
    /// media playlist selected or they are not loaded).
    pub(crate) fn curr_duration(&self) -> Option<f64> {
        let audio_duration = self
            .curr_media_playlist(MediaType::Audio)
            .and_then(|a| a.ending());
        let video_duration = self
            .curr_media_playlist(MediaType::Video)
            .and_then(|v| v.ending());
        match (audio_duration, video_duration) {
            (None, None) => None,
            (Some(a), Some(v)) => Some(f64::min(a, v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
        }
    }

    pub(crate) fn segment_target_duration(&self) -> Option<f64> {
        let audio_td = self
            .curr_media_playlist(MediaType::Audio)
            .map(|a| a.target_duration());
        let video_td = self
            .curr_media_playlist(MediaType::Video)
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
    pub(crate) fn curr_min_position(&self) -> Option<f64> {
        let audio_duration = self
            .curr_media_playlist(MediaType::Audio)
            .and_then(|a| a.beginning());
        let video_duration = self
            .curr_media_playlist(MediaType::Video)
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
    pub(crate) fn curr_max_position(&self) -> Option<f64> {
        self.curr_duration()
    }

    /// Returns a reference to the `VariantStream` currently selected. You can influence the
    /// variant currently selected by e.g. calling the `update_curr_bandwidth` method.
    pub(crate) fn curr_variant(&self) -> Option<&VariantStream> {
        self.playlist.variant(self.curr_variant_id)
    }

    /// Optionally update currently-selected variant by communicating the last bandwidth estimate.
    ///
    /// Returns a vec of `MediaType` corresponding to the MediaPlaylists that have been in
    /// consequence updated.
    /// Returns an empty vec if this new bandwidth estimate did not have any effect on any selected
    /// MediaPlaylist.
    pub(crate) fn update_curr_bandwidth(&mut self, bandwidth: f64) -> VariantUpdateResult {
        self.last_bandwidth = bandwidth;
        if self.is_variant_locked() {
            VariantUpdateResult::Unchanged
        } else {
            self.update_variant(None)
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
        let variants = self.supported_variants();
        let pos = variants.iter().find(|x| x.id() == variant_id);

        if pos.is_some() {
            self.is_variant_locked = true;
            let prev_track_id = self.curr_audio_track.or_else(|| self.curr_audio_track_id());
            let updates = self.update_variant(Some(variant_id));
            let new_track_id = self.curr_audio_track.or_else(|| self.curr_audio_track_id());
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
    pub(crate) fn unlock_variant(&mut self) -> VariantUpdateResult {
        self.is_variant_locked = false;
        self.update_variant(None)
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
    /// Both are probably an error as a `MediaPlaylistPermanentId` should always identify a
    /// `MediaPlaylist` .
    pub(crate) fn media_playlist_url(&self, wanted_id: &MediaPlaylistPermanentId) -> Option<&Url> {
        self.playlist.media_playlist_url(wanted_id)
    }

    /// Returns the `MediaPlaylistPermanentId` of the MediaPlaylist linked to the media
    /// of the given `MediaType`.
    ///
    /// Returns `None` if there's no choosen MediaPlaylist for the given MediaType.
    pub(crate) fn curr_media_playlist_id(
        &self,
        media_type: MediaType,
    ) -> Option<&MediaPlaylistPermanentId> {
        match media_type {
            MediaType::Video => self.curr_video_id.as_ref(),
            MediaType::Audio => self.curr_audio_id.as_ref(),
        }
    }

    /// Returns a reference to the MediaPlaylist currently loaded for the given `MediaType`.
    ///
    /// Returns `None` either if there's no MediaPlaylist selected for that `MediaType` or if the
    /// MediaPlaylist is not yet loaded.
    pub(crate) fn curr_media_playlist(&self, media_type: MediaType) -> Option<&MediaPlaylist> {
        if let Some(wanted_id) = match media_type {
            MediaType::Video => &self.curr_video_id,
            MediaType::Audio => &self.curr_audio_id,
        } {
            self.playlist.media_playlist(wanted_id)
        } else {
            None
        }
    }

    pub(crate) fn curr_media_playlist_segment_info(
        &self,
        media_type: MediaType,
    ) -> Option<(&SegmentList, SegmentQualityContext)> {
        if let Some(wanted_id) = match media_type {
            MediaType::Video => &self.curr_video_id,
            MediaType::Audio => &self.curr_audio_id,
        } {
            self.playlist.media_playlist(wanted_id).map(|m| {
                let score: f64 = self
                    .playlist
                    .variant(self.curr_variant_id)
                    .map(|v| v.score().unwrap_or(v.bandwidth() as f64))
                    .unwrap();

                let context = SegmentQualityContext::new(score, wanted_id.as_u32());
                (m.segment_list(), context)
            })
        } else {
            None
        }
    }

    /// Returns currently estimated start time in seconds at which to begin playing the content.
    ///
    /// This value may change depending on the chosen MediaPlaylist that are also loaded.
    pub(crate) fn expected_start_time(&self) -> f64 {
        let media_playlists = self.curr_media_playlists();
        if media_playlists.is_empty() {
            0.
        } else if media_playlists.iter().all(|p| p.1.is_live()) {
            let initial_dur: Option<f64> = None;
            let min_duration = media_playlists.iter().fold(initial_dur, |acc, p| {
                let duration = p.1.ending();
                if let Some(acc_dur) = acc {
                    if let Some(p_dur) = duration {
                        Some(acc_dur.min(p_dur))
                    } else {
                        Some(acc_dur)
                    }
                } else {
                    duration
                }
            });
            if let Some(min_duration) = min_duration {
                (min_duration - 10.).max(0.)
            } else {
                0.
            }
        } else {
            media_playlists
                .iter()
                .find_map(|p| p.1.wanted_start())
                .unwrap_or(0.)
        }
    }

    /// Returns the `id` of the `AudioTrack` object which is associated to the current audio
    /// media loaded.
    ///
    /// Returns `None` if no current audio media is known currently or if no `AudioTrack` is
    /// linked to it.
    pub(crate) fn curr_audio_track_id(&self) -> Option<u32> {
        self.playlist
            .audio_track_for_media_id(self.curr_audio_id.as_ref()?)
            .map(|p| p.id())
    }

    /// Returns the `id` of the `AudioTrack` object explicitely selected through the
    /// `set_audio_track` API.
    ///
    /// Returns `None` if no audio track is currently selected.
    pub(crate) fn selected_audio_track_id(&self) -> Option<u32> {
        self.curr_audio_track
    }

    /// Returns the list of available audio tracks on the current content
    pub(crate) fn audio_tracks(&self) -> &[AudioTrack] {
        self.playlist.audio_tracks()
    }

    /// Explicitely select an `AudioTrack` based on its `id` property or disable the explicit
    /// selection of one (by giving `None` as argument).
    ///
    /// Returns `true` if this call led to a changement for the Audio Media Playlist.
    pub(crate) fn set_audio_track(&mut self, track_id: Option<u32>) -> SetAudioTrackResponse {
        self.curr_audio_track = track_id;

        if let Some(variant) = self.curr_variant() {
            let new_audio_id = self
                .playlist
                .audio_media_playlist_id_for(variant, self.curr_audio_track);

            if new_audio_id.is_none() && self.curr_audio_id.is_some() {
                // We may be in a case where the choosen track is not available in the
                // current variant, re-check the best variant to have with the new track.
                let old_variant_locked = self.is_variant_locked;
                self.is_variant_locked = false;
                let variant_update = self.update_variant(None);
                SetAudioTrackResponse::VariantUpdate {
                    updates: variant_update,
                    unlocked_variant: old_variant_locked,
                }
            } else if new_audio_id != self.curr_audio_id {
                self.curr_audio_id = new_audio_id;
                SetAudioTrackResponse::AudioMediaUpdate
            } else {
                SetAudioTrackResponse::NoUpdate
            }
        } else {
            SetAudioTrackResponse::NoUpdate
        }
    }

    /// Select the best variant available according to your bandwidth and track choice
    fn update_variant(&mut self, variant_id: Option<u32>) -> VariantUpdateResult {
        let new_id = if let Some(id) = variant_id {
            id
        } else {
            let wanted_variants = self.variants_for_curr_track();
            if let Some(id) = best_variant_id(wanted_variants.into_iter(), self.last_bandwidth) {
                id
            } else if let Some(id) = fallback_variant_id(self.variants_for_curr_track().into_iter())
            {
                Logger::info(
                    "PS: Found no bandwidth-compatible variant amongst supported variants",
                );
                id
            } else {
                panic!("No variant to choose from. This should be impossible.");
            }
        };
        if new_id != self.curr_variant_id {
            let prev_bandwidth = self.curr_variant().map(|v| v.bandwidth());
            let new_bandwidth = self.playlist.variant(new_id).map(|v| v.bandwidth());
            let prev_audio_id = self.curr_audio_id.clone();
            let prev_video_id = self.curr_video_id.clone();
            self.set_curr_variant_and_media_id(new_id.to_owned());

            let mut updates = vec![];
            if self.curr_audio_id != prev_audio_id {
                updates.push(MediaType::Audio);
            }
            if self.curr_video_id != prev_video_id {
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
        let variant = self.playlist.variant(variant_id).unwrap();
        self.curr_variant_id = variant_id;
        self.curr_video_id = self.playlist.video_media_playlist_id_for(variant);
        self.curr_audio_id = self
            .playlist
            .audio_media_playlist_id_for(variant, self.curr_audio_track);
    }
}

/// From a `DoubleEndedIterator` of references to `VariantStream`s ordered first by `score` then
/// `bandwidth` ascending, find the best `VariantStream` which is compatible with the given
/// bandwidth and returns its `id` property.
fn best_variant_id<'a>(
    variants: impl DoubleEndedIterator<Item = &'a VariantStream>,
    bandwidth: f64,
) -> Option<u32> {
    variants
        .rev()
        .find(|x| (x.bandwidth() as f64) <= bandwidth)
        .map(|v| v.id())
}

/// From an `Iterator` of references to `VariantStream`s ordered first by `score` then
/// `bandwidth` ascending, find the one we should fallback to if none is compatible with our
/// current bandwidth.
///
/// That fallback value is the one of the lowest bandwidth with the highest score.
fn fallback_variant_id<'a>(variants: impl Iterator<Item = &'a VariantStream>) -> Option<u32> {
    variants
        .fold(None, |acc, v| {
            if let Some((bandwidth, _)) = acc {
                if v.bandwidth() <= bandwidth {
                    Some((v.bandwidth(), v.id()))
                } else {
                    acc
                }
            } else {
                Some((v.bandwidth(), v.id()))
            }
        })
        .map(|r| r.1)
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
    /// The audio track change led to a change of the Media Playlist for the audio.
    ///
    /// Because variants may be or not be linked to a given audio track it is also possible that
    /// the list of currently adaptively switchable variants has changed.
    AudioMediaUpdate,

    /// The audio track change led to a change for the currently-chosen variant due to the previous
    /// one not being compatible with the new chosen audio track.
    ///
    /// Because variants may be or not be linked to a given audio track it is also possible that the
    /// list of currently adaptively switchable variants has changed.
    ///
    /// The `updates` element of the associated struct is the result of such update, the
    /// `unlocked_variant`
    /// element is whether or not the previous variant was previously "locked" in place, in which
    /// case the lock has been completely disabled.
    VariantUpdate {
        updates: VariantUpdateResult,
        unlocked_variant: bool,
    },

    /// No Media Playlist nor the current variant were changed due to this track change.
    ///
    /// Because variants may be or not be linked to a given audio track it is however possible that
    /// the list of currently adaptively switchable variants has changed.
    NoUpdate,
}

pub(crate) enum LockVariantResponse {
    NoVariantWithId,
    VariantLocked {
        updates: VariantUpdateResult,
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
}
