use super::{
    media_playlist::MediaPlaylistParsingError,
    multi_variant_playlist::MediaPlaylistContext,
    utils::{parse_enumerated_string, parse_quoted_string, skip_attribute_list_value},
    MediaPlaylist,
};
use crate::{utils::url::Url, Logger};
use std::io::BufRead;

/// Structure describing a "Media tag" in the HLS Multivariant Playlist.
#[derive(Debug)]
pub struct MediaTag {
    /// Identifier for the MediaTag unique for the current
    /// `MultivariantPlaylist` object it is a part of.
    id: u32,

    /// Stable identifier for the URI within the parent Multivariant Playlist
    /// even in the case a different Multivariant fetched in the meantime,
    /// whereas `id` may change if the Multivariant Playlist is refreshed.
    ///
    /// This identifier allows the URI of the Variant Stream to
    /// change between two distinct downloads of the Multivariant
    /// Playlist. IDs are matched using a byte-for-byte comparison.
    stable_id: Option<String>,

    /// Media Playlist associated to this media tag.
    /// `None` if it does not exists or if not yet loaded.
    media_playlist: Option<MediaPlaylist>,

    /// Identify the underlying type of media
    typ: MediaTagType,

    /// Url at which the Media Playlist file linked to that MediaTag can be
    /// found.
    url: Option<Url>,

    /// The group to which the Rendition belongs, linked to corresponding
    /// variants.
    group_id: String,

    /// Contains one of the standard Tags for Identifying Languages [RFC5646],
    /// which identifies the primary language used in the Rendition.
    /// `None` if it does not apply or if unknown.
    language: Option<String>,

    /// Contains a language tag [RFC5646] that identifies a language that is
    /// associated with the Rendition.
    /// An associated language is often used in a different role than the
    /// language specified by the `language` attribute (e.g., written versus
    /// spoken, or a fallback dialect).
    assoc_language: Option<String>,

    /// Contains a human-readable description of the Rendition.
    /// If the `language` attribute is present, then this description SHOULD be
    /// in that language.
    name: String,

    /// If `true`, then the client SHOULD play this Rendition of the content in
    /// the absence of information from the user indicating a different choice.
    default: bool,

    /// If `true`, then the client MAY choose to play this Rendition in the
    /// absence of explicit user preference because it matches the current
    /// playback environment, such as chosen system language.
    /// If the autoselect attribute is present, its value MUST be `true` if
    /// the value of the default attribute is `true`.
    autoselect: bool,

    /// The forced attribute MUST NOT be present unless the `type` is
    /// `Subtitles`.
    ///
    /// `true` indicates that the Rendition contains content that is considered
    /// essential to play.  When selecting a forced Rendition, a client SHOULD
    /// choose the one that best matches the current playback environment (e.g.,
    /// language).
    forced: bool,

    /// If the `typ` attribute is Audio, then it is the count of audio
    /// channels indicating the maximum number of independent, simultaneous
    /// audio channels present in any Media Segment in the Rendition.
    /// For example, an AC-3 5.1 Rendition would have a CHANNELS="6" attribute.
    ///
    /// All audio `MediaTag` SHOULD have a channels attribute. If a
    /// Multivariant Playlist contains two Renditions with the same NAME
    /// encoded with the same codec but a different number of channels,
    /// then the `channels` attribute is REQUIRED; otherwise, it is
    /// OPTIONAL.
    channels: Option<u32>,
    // TODO
    // instream_id
    // characteristics
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaTagType {
    Audio,
    Video,
    Subtitles,
    ClosedCaptions,
    Other,
}

#[derive(Debug)]
pub enum MediaTagParsingError {
    MissingType,
    MissingGroupId,
    MissingName,
    Unknown,
}

impl MediaTag {
    pub(super) fn create(
        media_line: &str,
        multi_variant_playlist_url: &Url,
        id: u32,
    ) -> Result<Self, MediaTagParsingError> {
        let playlist_base_url = multi_variant_playlist_url.pathname();
        let mut typ: Option<MediaTagType> = None;
        let mut url: Option<Url> = None;
        let mut group_id: Option<String> = None;
        let mut language: Option<String> = None;
        let mut assoc_language: Option<String> = None;
        let mut name: Option<String> = None;
        let mut stable_rendition_id: Option<String> = None;
        let mut default = false;
        let mut autoselect = false;
        let mut forced = false;

        let channels: Option<u32> = None;

        let mut offset = "#EXT-X-MEDIA:".len();
        loop {
            if offset >= media_line.len() {
                break;
            }
            match media_line[offset..].find('=') {
                None => {
                    Logger::warn("Attribute Name not followed by equal sign");
                    break;
                }
                Some(idx) => {
                    match &media_line[offset..offset + idx] {
                        "TYPE" => {
                            let (parsed, end_offset) =
                                parse_enumerated_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            match parsed {
                                "AUDIO" => typ = Some(MediaTagType::Audio),
                                "VIDEO" => typ = Some(MediaTagType::Video),
                                "SUBTITLES" => typ = Some(MediaTagType::Subtitles),
                                "CLOSED-CAPTIONS" => typ = Some(MediaTagType::ClosedCaptions),
                                x => {
                                    Logger::warn(&format!("Unrecognized media type: {}", x));
                                    typ = Some(MediaTagType::Other);
                                }
                            };
                        }
                        "URI" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(parsed) = parsed {
                                url = Some(Url::new(parsed.to_owned()));
                            } else {
                                Logger::warn("Unparsable URI value");
                            }
                        }
                        "GROUP-ID" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                group_id = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable GROUP-ID value");
                            }
                        }
                        "LANGUAGE" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                language = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable LANGUAGE value");
                            }
                        }
                        "ASSOC-LANGUAGE" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                assoc_language = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable ASSOC-LANGUAGE value");
                            }
                        }
                        "NAME" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                name = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable NAME value");
                            }
                        }
                        "STABLE-RENDITION-ID" => {
                            let (parsed, end_offset) =
                                parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                stable_rendition_id = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable STABLE-RENDITION-ID value");
                            }
                        }
                        "DEFAULT" => {
                            let end_offset =
                                skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            default = true;
                        }
                        "AUTOSELECT" => {
                            let end_offset =
                                skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            autoselect = true;
                        }
                        "FORCED" => {
                            let end_offset =
                                skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            forced = true;
                        }
                        "CHANNELS" => {
                            // TODO
                            offset = skip_attribute_list_value(media_line, offset + idx + 1) + 1;
                        }
                        _ => offset = skip_attribute_list_value(media_line, offset + idx + 1) + 1,
                    }
                }
            }
        }

        let typ = if let Some(x) = typ {
            x
        } else {
            return Err(MediaTagParsingError::MissingType);
        };
        let group_id = if let Some(x) = group_id {
            x
        } else {
            return Err(MediaTagParsingError::MissingGroupId);
        };
        let name = if let Some(x) = name {
            x
        } else {
            return Err(MediaTagParsingError::MissingName);
        };

        url = url.map(|u| {
            if u.is_absolute() {
                u
            } else {
                Url::from_relative(playlist_base_url, u)
            }
        });
        Ok(MediaTag {
            id,
            stable_id: stable_rendition_id,
            media_playlist: None,
            typ,
            url,
            group_id,
            language,
            assoc_language,
            name,
            default,
            autoselect,
            forced,
            channels,
        })
    }

    pub(super) fn media_playlist(&self) -> Option<&MediaPlaylist> {
        self.media_playlist.as_ref()
    }

    pub(crate) fn update(
        &mut self,
        playlist: impl BufRead,
        url: Url,
        context: &MediaPlaylistContext,
    ) -> Result<&MediaPlaylist, MediaPlaylistParsingError> {
        let new_mp = MediaPlaylist::create(playlist, url, self.media_playlist.as_ref(), context)?;
        self.media_playlist = Some(new_mp);
        Ok(self.media_playlist.as_ref().unwrap())
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn stable_id(&self) -> Option<&str> {
        self.stable_id.as_deref()
    }

    pub(crate) fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub(crate) fn typ(&self) -> MediaTagType {
        self.typ
    }

    pub(crate) fn group_id(&self) -> &str {
        &self.group_id
    }

    pub(crate) fn is_autoselect(&self) -> bool {
        self.autoselect
    }

    pub(crate) fn is_default(&self) -> bool {
        self.default
    }

    pub(crate) fn language(&self) -> Option<&str> {
        let l = self.language.as_ref()?;
        Some(l.as_str())
    }

    pub(crate) fn assoc_language(&self) -> Option<&str> {
        let l = self.assoc_language.as_ref()?;
        Some(l.as_str())
    }

    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn channels(&self) -> Option<u32> {
        self.channels
    }
}
