use super::MediaTag;
use std::ops::{Deref, DerefMut};

pub(crate) struct AudioTrackList {
    tracks: Vec<AudioTrack>,
}

impl AudioTrackList {
    pub(super) fn new(mut audio_media: Vec<MediaTag>) -> Self {
        let mut available_audio_tracks: Vec<AudioTrack> = vec![];
        while let Some(media) = audio_media.pop() {
            let id = media.id();
            let name = media.name();
            let language = media.language();
            let assoc_language = media.assoc_language();
            let channels = media.channels();

            // Check if the track already exist in another encoding quality
            // TODO also check characteristics
            let pos_compat = available_audio_tracks.iter().position(|t| {
                t.name() == name
                    && t.language() == language
                    && t.assoc_language() == assoc_language
                    && t.channels() == channels
            });

            if let Some(pos) = pos_compat {
                available_audio_tracks[pos].media_tags.push(media);
            } else {
                available_audio_tracks.push(AudioTrack {
                    id: id.to_owned(),
                    media_tags: vec![media],
                });
            }
        }
        Self {
            tracks: available_audio_tracks,
        }
    }

    pub(super) fn groups_for(&self, id: &str) -> Vec<&str> {
        self.iter()
            .find(|t| t.id() == id)
            .map(|t| t.media_tags.iter().map(|m| m.group_id()).collect())
            .unwrap_or(vec![])
    }

    pub(super) fn track_for_media(&self, id: &str) -> Option<&AudioTrack> {
        self.iter()
            .find(|t| t.media_tags.iter().any(|m| m.id() == id))
    }

    pub(super) fn get_media(&self, id: &str) -> Option<&MediaTag> {
        self.iter()
            .find_map(|t| t.media_tags.iter().find(|m| m.id() == id))
    }

    pub(super) fn get_mut_media(&mut self, id: &str) -> Option<&mut MediaTag> {
        self.iter_mut()
            .find_map(|t| t.media_tags.iter_mut().find(|m| m.id() == id))
    }

    pub(super) fn iter_media(&self) -> impl Iterator<Item = (&AudioTrack, &MediaTag)> {
        self.iter().flat_map(|t| {
            t.media_tags
                .iter()
                .map(|m| (t, m))
                .collect::<Vec<(&AudioTrack, &MediaTag)>>()
        })
    }
}

impl Deref for AudioTrackList {
    type Target = Vec<AudioTrack>;

    fn deref(&self) -> &Self::Target {
        &self.tracks
    }
}

impl DerefMut for AudioTrackList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tracks
    }
}

pub struct AudioTrack {
    id: String,
    pub(self) media_tags: Vec<MediaTag>,
}

impl AudioTrack {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn language(&self) -> Option<&str> {
        self.media_tags.first().and_then(|t| t.language())
    }
    pub fn assoc_language(&self) -> Option<&str> {
        self.media_tags.first().and_then(|t| t.assoc_language())
    }
    pub fn name(&self) -> &str {
        self.media_tags.first().unwrap().name()
    }
    pub fn channels(&self) -> Option<u32> {
        self.media_tags.first().and_then(|t| t.channels())
    }
    pub(super) fn medias(&self) -> &[MediaTag] {
        &self.media_tags
    }
}
