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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MediaPlaylistPermanentId {
    VariantStreamUrl(usize),
    MediaTagUrl(usize),
}

pub struct WaspHlsContent {
    /// A struct representing the `MultiVariant Playlist`, a.k.a. `Master Playlist` of
    /// the currently loaded HLS content.
    ///
    /// `None` if either no HLS content is loaded or if its MultiVariant Playlist is
    /// not yet parsed.
    playlist: MultiVariantPlaylist,

    /// Index of the variant currently chosen variant, in terms of its index in the
    /// MultiVariantPlaylist's `variants` slice.
    /// Has to be watched closely to avoid out-of-bounds and de-synchronizations.
    curr_variant_idx: Option<usize>,

    curr_video_idx: Option<MediaPlaylistPermanentId>,
    curr_audio_idx: Option<MediaPlaylistPermanentId>,
}

pub enum MediaPlaylistLoadedState {
    None,
    Loaded,
    NotLoaded,
}

impl WaspHlsContent {
    pub(crate) fn new(playlist: MultiVariantPlaylist) -> Self {
        Self {
            playlist,
            curr_variant_idx: None,
            curr_audio_idx: None,
            curr_video_idx: None,
        }
    }

    pub(crate) fn all_curr_media_playlists_ready(&self) -> bool {
        self.curr_media_playlist_ready(MediaType::Audio) &&
            self.curr_media_playlist_ready(MediaType::Video)
    }

    pub(crate) fn curr_media_playlist_ready(&self, media_type: MediaType) -> bool {
        let idx = if media_type == MediaType::Audio {
            &self.curr_audio_idx
        } else {
            &self.curr_video_idx
        };
        match idx {
            None => true,
            Some(MediaPlaylistPermanentId::VariantStreamUrl(idx)) =>
                if let Some(m) = self.playlist.get_variant(*idx) {
                    m.media_playlist.is_some()
                } else {
                    false
                }
            Some(MediaPlaylistPermanentId::MediaTagUrl(idx)) =>
                if let Some(m) = self.playlist.get_media(*idx) {
                    m.media_playlist.is_some()
                } else {
                    false
                }
        }
    }

    pub(crate) fn has_media_type(&self, media_type: MediaType) -> bool {
        match media_type {
            MediaType::Audio => self.curr_audio_idx.is_some(),
            MediaType::Video => self.curr_video_idx.is_some(),
        }
    }


    pub(crate) fn update_media_playlist(&mut self,
        variant_idx: &MediaPlaylistPermanentId,
        media_playlist_data: impl BufRead,
        url: Url
    ) -> Result<(), MediaPlaylistUpdateError> {
        match variant_idx {
            MediaPlaylistPermanentId::VariantStreamUrl(idx) =>
                self.playlist.update_variant_media_playlist(*idx, media_playlist_data, url),
            MediaPlaylistPermanentId::MediaTagUrl(idx) =>
                self.playlist.update_media_tag_media_playlist(*idx, media_playlist_data, url),
        }
    }

    pub(crate) fn variants(&self) -> &[VariantStream] {
        self.playlist.variants()
    }

    pub(crate) fn set_curr_variant(&mut self, variant_idx: usize) {
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
                if x.1.typ() == &MediaTagType::Audio && x.1.group_id() == group_id && x.1.is_autoselect() {
                    if acc.is_none() || x.1.is_default() {
                        return Some(x.0);
                    }
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

    pub(crate) fn curr_variant(&self) -> Option<&VariantStream> {
        self.playlist.variants().get(self.curr_variant_idx?)
    }

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

    pub(crate) fn curr_init_segment(&self, media_type: MediaType) -> Option<&Url> {
        self.curr_media_playlist(media_type).as_ref()?.init_segment().map(|i| { &i.uri })
    }
}

