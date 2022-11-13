use std::io::BufRead;
use crate::{
    bindings::MediaType,
    parser::{
        VariantStream,
        MediaPlaylist,
        MultiVariantPlaylist,
        MediaTagType,
        MediaPlaylistUpdateError,
    },
    utils::url::Url,
};

/// Stores information about the current loaded playlist:
///   - The playlist itself.
///   - The current variant selected.
///   - The different audio and video media playlist selected.
pub struct ContentTracker {
    /// A struct representing the `MultiVariant Playlist`, a.k.a. `Master Playlist` of
    /// the currently loaded HLS content.
    playlist: MultiVariantPlaylist,

    /// Index of the currently chosen variant, in terms of its index in the
    /// MultiVariantPlaylist's `variants` slice.
    /// Has to be watched closely to avoid out-of-bounds and de-synchronizations.
    curr_variant_idx: Option<usize>,

    /// Chosen playlist for video.
    ///
    /// Also concerns playlist containing both audio and video. 
    ///
    /// Set to `None` if no video playlist is chosen. 
    curr_video_idx: Option<MediaPlaylistPermanentId>,

    /// Chosen playlist for audio.
    ///
    /// Set to `None` if no audio playlist is chosen. 
    curr_audio_idx: Option<MediaPlaylistPermanentId>,
}

impl ContentTracker {
    /// Create a new `ContentTracker` based on the given parsed MultiVariantPlaylist.
    pub(crate) fn new(playlist: MultiVariantPlaylist) -> Self {
        Self {
            playlist,
            curr_variant_idx: None,
            curr_audio_idx: None,
            curr_video_idx: None,
        }
    }

    /// Returns the list of tuple listing loaded media playlists.
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
    pub(crate) fn curr_media_playlist_ready(&self, media_type: MediaType) -> bool {
        self.curr_media_playlist(media_type).is_some()
    }

    /// Returns `true` only if all media playlists currently selected have been loaded.
    pub(crate) fn all_curr_media_playlists_ready(&self) -> bool {
        if self.has_media_type(MediaType::Audio) &&
            !self.curr_media_playlist_ready(MediaType::Audio)
        {
            false
        } else if self.has_media_type(MediaType::Video) &&
            !self.curr_media_playlist_ready(MediaType::Video)
        {
            false
        } else {
            true
        }

    }

    /// Returns true if a MediaPlaylist for the given `MediaType` has been selected, regardless if
    /// that playlist has been loaded or not. 
    pub(crate) fn has_media_type(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.curr_audio_idx.is_some(),
            MediaType::Video => self.curr_video_idx.is_some(),
        }
    }

    /// Initialize or update a `MediaPlaylist`, based on its `MediaPlaylistPermanentId`.
    pub(crate) fn update_media_playlist(&mut self,
        id: &MediaPlaylistPermanentId,
        media_playlist_data: impl BufRead,
        url: Url
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        match id {
            MediaPlaylistPermanentId::VariantStreamUrl(idx) =>
                self.playlist.update_variant_media_playlist(*idx, media_playlist_data, url),
            MediaPlaylistPermanentId::MediaTagUrl(idx) =>
                self.playlist.update_media_tag_media_playlist(*idx, media_playlist_data, url),
        }
    }

    /// Returns vec describing all available variant streams in the current MultiVariantPlaylist.
    pub(crate) fn variants(&self) -> &[VariantStream] {
        self.playlist.variants()
    }

    /// Estimates the duration of the current content based on the currently selected audio and
    /// video media playlists.
    ///
    /// Returns `None` if there's not enough data to produce that estimate (e.g. no audio or video
    /// media playlist selected or they are not loaded).
    pub(crate) fn curr_duration(&self) -> Option<f64> {
        let audio_duration = match self.curr_media_playlist(MediaType::Audio) {
            None => None,
            Some(m) => m.duration(),
        };
        let video_duration = match self.curr_media_playlist(MediaType::Video) {
            None => None,
            Some(m) => m.duration(),
        };
        match (audio_duration, video_duration) {
            (None, None) => None,
            (Some(a), Some(v)) => Some(f64::min(a, v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v)
        }
    }

    /// Returns a reference to the `VariantStream` currently selected. You can influence the
    /// variant currently selected by e.g. calling the `update_curr_bandwidth` method.
    pub(crate) fn curr_variant(&self) -> Option<&VariantStream> {
        self.playlist.variants().get(self.curr_variant_idx?)
    }

    /// Optionally update currently-selected variant by communicating the last bandwidth estimate.
    ///
    /// Returns a vec of `MediaType` corresponding to the MediaPlaylists that have been in
    /// consequence updated.
    /// Returns an empty vec if this new bandwidth estimate did not have any effect on any selected
    /// MediaPlaylist.
    pub(crate) fn update_curr_bandwidth(
        &mut self, bandwidth: f64
    ) -> Vec<MediaType> {
        let variants = self.playlist.variants();
        let best_variant_idx = variants.iter().position(|x| {
            (x.bandwidth as f64) > bandwidth
        }).or_else(|| {
            if variants.len() == 0 {
                None
            } else {
                Some(variants.len() - 1)
            }
        });

        if best_variant_idx != self.curr_variant_idx {
            if let Some(idx) = best_variant_idx {
                let prev_audio_idx = self.curr_audio_idx.clone();
                let prev_video_idx = self.curr_video_idx.clone();
                self.set_curr_variant(idx);

                let mut updates = vec![];
                if self.curr_audio_idx != prev_audio_idx {
                    updates.push(MediaType::Audio);
                }
                if self.curr_video_idx != prev_video_idx {
                    updates.push(MediaType::Video);
                }
                updates
            } else {
                self.curr_variant_idx = None;
                self.curr_video_idx = None;
                self.curr_audio_idx = None;
                vec![MediaType::Audio, MediaType::Video]
            }
        } else {
            vec![]
        }
    }

    /// Returns the metadata allowing to load and the update the MediaPlaylist of the given
    /// `MediaType`.
    pub(crate) fn curr_media_playlist_request_info(&self,
        media_type: MediaType
    ) -> Option<(&Url, &MediaPlaylistPermanentId)> {
        let wanted_idx = match media_type {
            MediaType::Video => &self.curr_video_idx,
            MediaType::Audio => &self.curr_audio_idx,
        };
        match wanted_idx {
            Some(MediaPlaylistPermanentId::VariantStreamUrl(idx)) => {
                Some((
                    self.playlist.get_variant(*idx)?.get_url(),
                    wanted_idx.as_ref().unwrap()
                ))
            },
            Some(MediaPlaylistPermanentId::MediaTagUrl(idx)) => {
                Some((
                    self.playlist.get_media(*idx)?.get_url()?,
                    wanted_idx.as_ref().unwrap()
                ))
            },
            None => None,
        }
    }

    /// Returns a reference to the MediaPlaylist currently loaded for the given `MediaType`.
    ///
    /// Returns `None` either if there's no MediaPlaylist selected for that `MediaType` or if the
    /// MediaPlaylist is not yet loaded.
    pub(crate) fn curr_media_playlist(&self,
        media_type: MediaType
    ) -> Option<&MediaPlaylist> {
        let wanted_idx = match media_type {
            MediaType::Video => &self.curr_video_idx,
            MediaType::Audio => &self.curr_audio_idx,
        };
        match wanted_idx {
            Some(MediaPlaylistPermanentId::VariantStreamUrl(idx)) => {
                Some(self.playlist.get_variant(*idx)?.media_playlist.as_ref()?)
            },
            Some(MediaPlaylistPermanentId::MediaTagUrl(idx)) => {
                Some(self.playlist.get_media(*idx)?.media_playlist.as_ref()?)
            },
            None => None,
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
        self.curr_media_playlist(media_type).as_ref()?.init_segment().map(|i| { &i.uri })
    }

    /// Returns currently estimated start time in seconds at which to begin playing the content.
    pub(crate) fn get_expected_start_time(&self) -> f64 {
        let media_playlists = self.curr_media_playlists();
        if media_playlists.is_empty() ||
            media_playlists.iter().any(|p| { p.1.end_list })
        {
            0.
        } else {
            let initial_dur: Option<f64> = None;
            let min_duration = media_playlists.iter().fold(initial_dur, |acc, p| {
                let duration = p.1.duration();
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
        }
    }

    // pub(crate) fn refresh_time(&self) -> Option<f64> {
    //     let media_playlists = self.curr_media_playlists();
    //     if media_playlists.is_empty() ||
    //         media_playlists.iter().all(|p| { p.1.end_list })
    //     {
    //         None
    //     } else {
    //         media_playlists.iter().fold(None, |acc, p| {
    //             let target_duration = p.1.target_duration();
    //             if let Some(acc_dur) = acc {
    //                 if let Some(p_dur) = target_duration {
    //                     Some(acc_dur.min(p_dur))
    //                 } else {
    //                     Some(acc_dur)
    //                 }
    //             } else {
    //                 target_duration
    //             }
    //         })
    //     }
    // }

    fn set_curr_variant(&mut self, variant_idx: usize) {
        let variants = self.playlist.variants();
        if variant_idx >= variants.len() {
            panic!("Variant index provided is out of bounds.");
        }
        self.curr_variant_idx = Some(variant_idx);

        let variant = self.playlist.get_variant(variant_idx).unwrap();
        if variant.has_type(MediaType::Video) {
            self.curr_video_idx = Some(MediaPlaylistPermanentId::VariantStreamUrl(variant_idx));
        } else {
            self.curr_video_idx = None;
        }

        if let Some(group_id) = variant.audio.as_ref() {
            let best_audio = self.playlist.medias().iter().enumerate().fold(None, |acc, x| {
                if x.1.typ() == MediaTagType::Audio && x.1.group_id() == group_id &&
                    x.1.is_autoselect() && (acc.is_none() || x.1.is_default())
                {
                    return Some(x.0);
                }
                acc
            });
            if let Some(idx) = best_audio {
                self.curr_audio_idx = Some(MediaPlaylistPermanentId::MediaTagUrl(idx));
            } else {
                self.curr_audio_idx = None;
            }
        } else {
            self.curr_audio_idx = None;
        }
    }

    // TODO Should not be relied on for now, still working out the details
    pub(crate) fn todo_get_available_audio_tracks(&self) -> Vec<AvailableAudioTrack> {
        let mut available_audio_tracks : Vec<AvailableAudioTrack> = vec![];
        for (idx, media) in self.playlist.medias().iter().enumerate() {
            if media.typ() == MediaTagType::Audio {
                // TODO Implementing this method might actually be harder when considering
                // multiple audio media tags with the same characteristics but used in different
                // variants.
                let is_current =  if let Some(MediaPlaylistPermanentId::MediaTagUrl(id)) = self.curr_audio_idx {
                    id == idx
                } else {
                    false
                };
                available_audio_tracks.push(AvailableAudioTrack {
                    is_current,
                    id: idx,
                    language: media.language(),
                    assoc_language: media.assoc_language(),
                    name: media.name(),
                    channels: media.channels()
                })
            }
        }
        available_audio_tracks
    }
}

/// Identifier allowing to identify a given MediaPlaylist based on its index in the array of
/// variants and media tags in the MultiVariantPlaylist.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MediaPlaylistPermanentId {
    /// This Media Playlist is defined by a variant stream definition in the MediaPlaylist whose
    /// index is its argument.
    VariantStreamUrl(usize),
    /// This Media Playlist is defined by a media tag definition in the MediaPlaylist whose
    /// index is its argument.
    MediaTagUrl(usize),
}

/// Current state a given Media Playlist, defined either by a veriant stream or a media tag in the
/// MultiVariantPlaylist is loaded or not.
pub enum MediaPlaylistLoadedState {
    None,
    Loaded,
    NotLoaded,
}

pub struct AvailableAudioTrack<'a> {
    is_current: bool,
    id: usize,
    language: Option<&'a str>,
    assoc_language: Option<&'a str>,
    name: &'a str,
    channels: Option<u32>,
}

impl<'a> AvailableAudioTrack<'a> {
   pub fn is_current(&self) -> bool {
       self.is_current
   }
   pub fn id(&self) -> usize {
       self.id
   }
   pub fn language(&self) -> Option<&'a str> {
       self.language
   }
   pub fn assoc_language(&self) -> Option<&'a str> {
       self.assoc_language
   }
   pub fn name(&self) -> &'a str {
       self.name
   }
   pub fn channels(&self) -> Option<u32> {
       self.channels
   }
}
