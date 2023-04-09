use super::MediaTag;
use std::ops::{Deref, DerefMut};

/// Allows to translate various `EXT-X-MEDIA` tag found inside a Multivariant Playlist into well
/// defined audio tracks that make more sense in a player API.
pub(crate) struct AudioTrackList {
    /// List of audio tracks as constructed by this `AudioTrackList`.
    tracks: Vec<AudioTrack>,
}

impl AudioTrackList {
    /// Creates a new `AudioTrackList` from the given `Vec<MediaTag>`, which should contain
    /// information on all media tags of type `"AUDIO"`.
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

    /// Returns values of all of the HLS `GROUP-ID` attributes who are linked to the given track's
    /// `id`.
    ///
    /// This method can be called for example to check if a given HLS variant is compatible to the
    /// given track's id.
    pub(super) fn groups_for_track_id(&self, id: u32) -> Vec<&str> {
        self.iter()
            .find(|t| t.id() == id)
            .map(|t| t.media_tags.iter().map(|m| m.group_id()).collect())
            .unwrap_or(vec![])
    }

    /// Returns `AudioTrack` object associated to the given `MediaTag`'s id.
    ///
    /// Returns `None` if none is found.
    pub(super) fn track_for_media_tag(&self, id: u32) -> Option<&AudioTrack> {
        self.iter()
            .find(|t| t.media_tags.iter().any(|m| m.id() == id))
    }

    /// Returns reference too `MediaTag` object associated to the given `MediaTag`'s id, stored in
    /// the `AudioTrackList`
    ///
    /// Returns `None` if none is found.
    pub(super) fn media_tag(&self, id: u32) -> Option<&MediaTag> {
        self.iter()
            .find_map(|t| t.media_tags.iter().find(|m| m.id() == id))
    }

    /// Returns mutable reference too `MediaTag` object associated to the given `MediaTag`'s id,
    /// stored in the `AudioTrackList`
    ///
    /// Returns `None` if none is found.
    pub(super) fn media_tag_mut(&mut self, id: u32) -> Option<&mut MediaTag> {
        self.iter_mut()
            .find_map(|t| t.media_tags.iter_mut().find(|m| m.id() == id))
    }

    /// Returns an iterator implementation on tuples of `AudioTrack` and inner `MediaTag` objects
    /// found in this `AudioTrackList`.
    pub(super) fn iter_tracks_media(&self) -> impl Iterator<Item = (&AudioTrack, &MediaTag)> {
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

/// Describe a single audio track object with its own characteristics.
pub struct AudioTrack {
    /// Unique identifier for that audio track in the parent `AudioTrackList`.
    id: u32,
    /// `MediaTag` objects associated to this audio track.
    ///
    /// These are usually several qualities representing the same content.
    pub(self) media_tags: Vec<MediaTag>,
}

impl AudioTrack {
    /// Unique identifier for that audio track in the parent `AudioTrackList`.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Returns reference to the audio language linked to that audio track.
    pub fn language(&self) -> Option<&str> {
        self.media_tags.first().and_then(|t| t.language())
    }

    /// Returns reference to the associated (secondary) audio language linked to that audio track.
    pub fn assoc_language(&self) -> Option<&str> {
        self.media_tags.first().and_then(|t| t.assoc_language())
    }

    /// Returns human-readable name associated with that audio track.
    pub fn name(&self) -> &str {
        self.media_tags.first().unwrap().name()
    }

    /// Returns the number of channels that audio track outputs
    pub fn channels(&self) -> Option<u32> {
        self.media_tags.first().and_then(|t| t.channels())
    }

    /// Returns slice of the various `MediaTag` objects that audio track is associated with.
    ///
    /// These are usually several qualities representing the same content.
    pub(super) fn medias(&self) -> &[MediaTag] {
        &self.media_tags
    }
}
