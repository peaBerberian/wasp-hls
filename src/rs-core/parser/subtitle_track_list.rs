use super::{MediaTag, MediaTagType, VariantStream};

pub(crate) struct SubtitleTrackList {
    tracks: Vec<SubtitleTrack>,
}

impl SubtitleTrackList {
    pub(super) fn new(media: &[MediaTag], variants: &[VariantStream]) -> Self {
        let mut tracks: Vec<SubtitleTrack> = Vec::new();
        for rendition in media.iter().filter(|media| {
            media.typ() == MediaTagType::Subtitles
                && media.url().is_some()
                && variants
                    .iter()
                    .any(|variant| variant.subtitle_group() == Some(media.group_id()))
        }) {
            if let Some(track) = tracks.iter_mut().find(|track| {
                track.name == rendition.name()
                    && track.language.as_deref() == rendition.language()
                    && track.assoc_language.as_deref() == rendition.assoc_language()
                    && track.forced == rendition.is_forced()
                    && track.characteristics == rendition.characteristics()
            }) {
                track.rendition_ids.push(rendition.id());
            } else {
                tracks.push(SubtitleTrack {
                    id: rendition.id(),
                    rendition_ids: vec![rendition.id()],
                    name: rendition.name().to_owned(),
                    language: rendition.language().map(str::to_owned),
                    assoc_language: rendition.assoc_language().map(str::to_owned),
                    forced: rendition.is_forced(),
                    characteristics: rendition.characteristics().to_vec(),
                });
            }
        }
        Self { tracks }
    }

    pub(super) fn tracks(&self) -> &[SubtitleTrack] {
        &self.tracks
    }

    pub(super) fn track(&self, id: u32) -> Option<&SubtitleTrack> {
        self.tracks.iter().find(|track| track.id == id)
    }

    pub(super) fn track_for_media(&self, id: u32) -> Option<&SubtitleTrack> {
        self.tracks
            .iter()
            .find(|track| track.rendition_ids.contains(&id))
    }
}

pub(crate) struct SubtitleTrack {
    id: u32,
    rendition_ids: Vec<u32>,
    name: String,
    language: Option<String>,
    assoc_language: Option<String>,
    forced: bool,
    characteristics: Vec<String>,
}

impl SubtitleTrack {
    pub(crate) fn id(&self) -> u32 {
        self.id
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    pub(crate) fn assoc_language(&self) -> Option<&str> {
        self.assoc_language.as_deref()
    }
    pub(crate) fn is_forced(&self) -> bool {
        self.forced
    }
    pub(crate) fn characteristics(&self) -> &[String] {
        &self.characteristics
    }
    pub(super) fn contains_media(&self, id: u32) -> bool {
        self.rendition_ids.contains(&id)
    }
}
