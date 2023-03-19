use crate::{
    bindings::{jsIsTypeSupported, MediaType},
    media_element::SegmentQualityContext,
    parser::{
        AudioTrack, MediaPlaylist, MediaPlaylistUpdateError, MultiVariantPlaylist, VariantStream,
    },
    utils::url::Url,
};
use std::{cmp::Ordering, io::BufRead};

pub(crate) use crate::parser::MediaPlaylistPermanentId;

/// Stores information about the current loaded MultiVariant Playlist and its sub-playlists:
///   - Information on the MultiVariant Playlist itself.
///   - On the current variant selected.
///   - Information on the different audio and video Media Playlists selected.
pub(crate) struct PlaylistStore {
    /// A struct representing the "MultiVariant Playlist", a.k.a. "Master Playlist" of
    /// the currently loaded HLS content.
    playlist: MultiVariantPlaylist,

    /// `id` of the currently chosen variant.
    curr_variant_id: Option<u32>,

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

    curr_audio_track: Option<u32>,

    /// If `true` a variant is being manually locked and as such, cannot change.
    is_variant_locked: bool,

    /// Store the last communicated bandwidth
    last_bandwidth: f64,

    /// Before actually playing a content, supported codecs need to be checked
    /// to avoid mistakenly choosing an unsupported codec.
    /// This bool is set to `true` only once ALL codecs in the
    /// `MultiVariantPlaylist` have been properly checked.
    codecs_checked: bool,
}

impl PlaylistStore {
    /// Create a new `PlaylistStore` based on the given parsed `MultiVariantPlaylist`.
    pub(crate) fn new(playlist: MultiVariantPlaylist) -> Self {
        Self {
            playlist,
            curr_variant_id: None,
            curr_audio_id: None,
            curr_video_id: None,
            curr_audio_track: None,
            is_variant_locked: false,
            last_bandwidth: 0.,
            codecs_checked: false,
        }
    }

    /// Returns a reference to the `Url` to the MultiVariant Playlist stored by this
    /// `PlaylistStore`.
    pub(crate) fn url(&self) -> &Url {
        self.playlist.url()
    }

    /// Returns `true` once all codecs present in the `MultiVariantPlaylist` object
    /// have been checked.
    ///
    /// Until then, unchecked `MediaPlaylist` objects, its properties, and segments should not be
    /// relied on.
    ///
    /// To allow all codecs to be checked, call the `check_codecs` method.
    pub(crate) fn are_codecs_checked(&self) -> bool {
        self.codecs_checked
    }

    /// Check which codecs present in the `MultiVariantPlaylist` are supported.
    ///
    /// This allows the playlist store to know which variant can actually be relied on.
    /// As such you should be extra careful when using the `PlaylistStore` before that check has
    /// been completely done.
    ///
    /// Returns `true` if all codecs in the `MultiVariantPlaylist` could have been checked or
    /// `false` if it still await a response from JavaScript. As that response can be asynchronous
    /// it is given back to the corresponding Dispatcher's event listener function.
    ///
    /// Once that even listener has been called, `check_codecs` can be called again, until it
    /// returns `true`.
    pub(crate) fn check_codecs(&mut self) -> bool {
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
                            jsIsTypeSupported(mt, &codec);
                        }
                    }
                });
        });
        self.codecs_checked = are_all_codecs_checked;
        are_all_codecs_checked
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

    /// Returns vec describing all available variant streams in the current MultiVariantPlaylist.
    pub(crate) fn all_variants(&self) -> Vec<&VariantStream> {
        self.playlist.supported_variants()
    }

    /// Returns vec describing all available variant streams in the current MultiVariantPlaylist.
    pub(crate) fn variants_for_curr_track(&self) -> Vec<&VariantStream> {
        if let Some(track_id) = self.curr_audio_track {
            self.playlist.supported_variants_for_audio(track_id)
        } else if let Some(track_id) = self.curr_audio_track_id() {
            self.playlist.supported_variants_for_audio(track_id)
        } else {
            self.all_variants()
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
        self.playlist.variant(self.curr_variant_id?)
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
        let variants = self.all_variants();
        let pos = variants.iter().find(|x| x.id() == variant_id);

        if pos.is_some() {
            self.is_variant_locked = true;

            // TODO better change detection
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

    pub(crate) fn curr_context(&self, media_type: MediaType) -> SegmentQualityContext {
        let score = self
            .curr_variant_id
            .and_then(|id| self.playlist.variant(id).map(|v| v.bandwidth() as f64));
        let quality_id = self.curr_media_playlist_id(media_type).map(|m| m.as_u32());
        SegmentQualityContext::new(score, quality_id)
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

    /// Returns `Url` to the initialization segment of the MediaPlaylist corresponding to the given
    /// `MediaType`.
    ///
    /// Returns `None` if any of the following is true:
    ///   - There's no MediaPlaylist for that given `MediaType`.
    ///   - The MediaPlaylist for that given `MediaType` is not yet loaded.
    ///   - There's no initialization segment for the MediaPlaylist of that given `MediaType`.
    pub(crate) fn curr_init_segment(&self, media_type: MediaType) -> Option<&Url> {
        self.curr_media_playlist(media_type)
            .as_ref()?
            .init_segment()
            .map(|i| &i.uri)
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
            Some(id)
        } else {
            let wanted_variants = &self.variants_for_curr_track();
            best_variant_id(wanted_variants, self.last_bandwidth)
        };
        if new_id != self.curr_variant_id {
            if let Some(id) = new_id {
                let prev_bandwidth = self.curr_variant().map(|v| v.bandwidth());
                let new_bandwidth = self.playlist.variant(id).map(|v| v.bandwidth());
                let prev_audio_id = self.curr_audio_id.clone();
                let prev_video_id = self.curr_video_id.clone();
                self.set_curr_variant_and_media_id(id.to_owned());

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
                self.curr_variant_id = None;
                self.curr_video_id = None;
                self.curr_audio_id = None;
                VariantUpdateResult::EqualOrUnknown(vec![MediaType::Audio, MediaType::Video])
            }
        } else {
            VariantUpdateResult::Unchanged
        }
    }

    /// Internally update the current variant chosen as well as its corresponding other media.
    fn set_curr_variant_and_media_id(&mut self, variant_id: u32) {
        let variant = self.playlist.variant(variant_id).unwrap();
        self.curr_variant_id = Some(variant_id);
        self.curr_video_id = self.playlist.video_media_playlist_id_for(variant);
        self.curr_audio_id = self
            .playlist
            .audio_media_playlist_id_for(variant, self.curr_audio_track);
    }
}

/// In the slice of references to `VariantStream`s ordered by bandwidth ascending, find the closest
/// `VariantStream` with a lower or equal bandwidth to the one given and returns its index.
fn best_variant_id(variants: &[&VariantStream], bandwidth: f64) -> Option<u32> {
    variants
        .iter()
        .find(|x| (x.bandwidth() as f64) > bandwidth)
        .or(variants.last())
        .map(|v| v.id())
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
