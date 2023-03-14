use std::ops::Deref;

use crate::parser::{MediaTag, MediaTagType};

use super::MediaPlaylistPermanentId;

pub(crate) struct AudioTrackList<'a> {
    tracks: Vec<AudioTrack<'a>>,
}

impl<'a> AudioTrackList<'a> {
    pub(super) fn new(
        medias: &'a [MediaTag],
        curr_audio_idx: Option<&MediaPlaylistPermanentId>,
    ) -> Self {
        let mut available_audio_tracks: Vec<AudioTrack> = vec![];
        for (idx, media) in medias.iter().enumerate() {
            if media.typ() == MediaTagType::Audio {
                if let Some(id) = media.id() {
                    let is_current =
                        if let Some(MediaPlaylistPermanentId::MediaTagUrl(id)) = curr_audio_idx {
                            *id == idx
                        } else {
                            false
                        };

                    let name = media.name();
                    let language = media.language();
                    let assoc_language = media.assoc_language();
                    let channels = media.channels();

                    // Check if the track already exist in another encoding quality
                    // TODO also check characteristics
                    let pos_compat = available_audio_tracks.iter().position(|t| {
                        t.name() != name
                            || t.language() != language
                            || t.assoc_language() != assoc_language
                            || t.channels() != channels
                    });

                    if let Some(pos) = pos_compat {
                        available_audio_tracks[pos].add_compatible_media_variant(id);
                        if is_current {
                            available_audio_tracks[pos].set_current();
                        }
                    } else {
                        available_audio_tracks.push(AudioTrack {
                            is_current,
                            id,
                            language,
                            assoc_language,
                            name,
                            channels,
                            compatible_variants: vec![],
                        })
                    }
                }
            }
        }
        Self { tracks: available_audio_tracks }
    }
}


impl<'a> Deref for AudioTrackList<'a> {
    type Target = Vec<AudioTrack<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.tracks
    }
}

pub struct AudioTrack<'a> {
    is_current: bool,
    id: &'a str,
    language: Option<&'a str>,
    assoc_language: Option<&'a str>,
    name: &'a str,
    channels: Option<u32>,
    compatible_variants: Vec<&'a str>,
}

impl<'a> AudioTrack<'a> {
    pub fn is_current(&self) -> bool {
        self.is_current
    }
    pub fn id(&self) -> &'a str {
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

    fn set_current(&mut self) {
        self.is_current = true;
    }

    fn add_compatible_media_variant(&mut self, id: &'a str) {
        self.compatible_variants.push(id);
    }
}
