use super::MediaTag;
use std::ops::Deref;

/// Groups equivalent subtitle renditions into selectable tracks.
pub(crate) struct SubtitleTrackList {
    tracks: Vec<SubtitleTrack>,
}

impl SubtitleTrackList {
    pub(super) fn new(subtitle_media: Vec<MediaTag>) -> Self {
        let mut tracks: Vec<SubtitleTrack> = Vec::new();
        for media in subtitle_media
            .into_iter()
            .filter(|media| media.url().is_some())
        {
            if let Some(track) = tracks.iter_mut().find(|track| {
                track.name() == media.name()
                    && track.language() == media.language()
                    && track.assoc_language() == media.assoc_language()
                    && track.is_forced() == media.is_forced()
                    && track.characteristics() == media.characteristics()
            }) {
                track.media_tags.push(media);
            } else {
                tracks.push(SubtitleTrack {
                    id: media.id(),
                    media_tags: vec![media],
                });
            }
        }
        Self { tracks }
    }

    /// Groups with a playlist-backed rendition for the chosen track.
    pub(super) fn groups_for_track_id(&self, id: u32) -> Vec<&str> {
        self.iter()
            .find(|track| track.id() == id)
            .map(|track| {
                track
                    .medias()
                    .iter()
                    .filter(|media| media.url().is_some())
                    .map(|media| media.group_id())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn track_for_media_tag(&self, id: u32) -> Option<&SubtitleTrack> {
        self.iter()
            .find(|track| track.medias().iter().any(|media| media.id() == id))
    }

    pub(super) fn media_tag(&self, id: u32) -> Option<&MediaTag> {
        self.iter()
            .find_map(|track| track.medias().iter().find(|media| media.id() == id))
    }

    pub(super) fn media_tag_mut(&mut self, id: u32) -> Option<&mut MediaTag> {
        self.tracks
            .iter_mut()
            .find_map(|track| track.media_tags.iter_mut().find(|media| media.id() == id))
    }

    pub(super) fn iter_tracks_media(&self) -> impl Iterator<Item = (&SubtitleTrack, &MediaTag)> {
        self.iter()
            .flat_map(|track| track.medias().iter().map(move |media| (track, media)))
    }
}

impl Deref for SubtitleTrackList {
    type Target = Vec<SubtitleTrack>;

    fn deref(&self) -> &Self::Target {
        &self.tracks
    }
}

/// A selectable subtitle track and its backing HLS renditions.
pub(crate) struct SubtitleTrack {
    /// Unique identifier for this track in its parent list.
    id: u32,
    /// Backing renditions, kept nonempty by `SubtitleTrackList::new`.
    media_tags: Vec<MediaTag>,
}

impl SubtitleTrack {
    pub(crate) fn id(&self) -> u32 {
        self.id
    }
    pub(crate) fn name(&self) -> Option<&str> {
        self.media_tags.first().and_then(MediaTag::name)
    }
    pub(crate) fn language(&self) -> Option<&str> {
        self.media_tags.first().and_then(MediaTag::language)
    }
    pub(crate) fn assoc_language(&self) -> Option<&str> {
        self.media_tags.first().and_then(MediaTag::assoc_language)
    }
    pub(crate) fn is_forced(&self) -> bool {
        self.media_tags.first().is_some_and(MediaTag::is_forced)
    }
    pub(crate) fn characteristics(&self) -> &[String] {
        self.media_tags
            .first()
            .map_or(&[], |media| media.characteristics())
    }
    pub(super) fn medias(&self) -> &[MediaTag] {
        &self.media_tags
    }
}
