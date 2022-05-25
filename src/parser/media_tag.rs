use std::io::BufRead;
use crate::{utils::url::Url, Logger};
use super::{
    MediaPlaylist,
    utils::{
        parse_enumerated_string,
        parse_quoted_string,
        skip_attribute_list_value,
    },
    media_playlist::MediaPlaylistParsingError,
};

/// Structure describing a "Media tag" in the HLS Multivariant Playlist.
#[derive(Debug)]
pub struct MediaTag {
    /// Media Playlist associated to this media tag.
    /// `None` if it does not exists or if not yet loaded.
    pub media_playlist: Option<MediaPlaylist>,

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

    /// Stable identifier for the URI within the Multivariant Playlist.
    /// All characters in the string MUST be from the following set:
    /// [a..z], [A..Z], [0..9], '+', '/', '=', '.', '-', and '_'.
    ///
    /// The stable_rendition_id allows the URI of a Rendition to change
    /// between two distinct downloads of the Multivariant Playlist. IDs
    /// are matched using a byte-for-byte comparison.
    ///
    /// All `MediaTag` in a `MultiVariantPlaylist` with the same `url`
    /// value SHOULD use the same stable_rendition_id.
    stable_rendition_id: Option<String>,

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

#[derive(Debug, PartialEq, Eq)]
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
}

impl MediaTag {
    /// TODO real update
    pub fn update(&mut self,
        playlist: impl BufRead,
        url: Url
    ) -> Result<(), MediaPlaylistParsingError> {
        let new_mp = MediaPlaylist::create(playlist, url)?;
        self.media_playlist = Some(new_mp);
        Ok(())
    }

    pub(super) fn create(
        media_line: &str,
        multi_variant_playlist_url: &Url
    ) -> Result<Self, MediaTagParsingError> {
        let playlist_base_url = multi_variant_playlist_url.pathname();
        let mut typ : Option<MediaTagType> = None;
        let mut url : Option<Url> = None;
        let mut group_id : Option<String> = None;
        let mut language : Option<String> = None;
        let mut assoc_language : Option<String> = None;
        let mut name : Option<String> = None;
        let mut stable_rendition_id : Option<String> = None;
        let mut default = false;
        let mut autoselect = false;
        let mut forced = false;

        let channels : Option<u32> = None;

        let mut offset = "#EXT-X-MEDIA:".len();
        loop {
            if offset >= media_line.len() {
                break;
            }
            match media_line[offset..].find("=") {
                None => {
                    Logger::warn("Attribute Name not followed by equal sign");
                    break;
                }
                Some(idx) => {
                    match &media_line[offset..offset + idx] {
                        "TYPE" => {
                            let (parsed, end_offset) = parse_enumerated_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            match parsed {
                                "AUDIO" => typ = Some(MediaTagType::Audio),
                                "VIDEO" => typ = Some(MediaTagType::Video),
                                "SUBTITLES" => typ = Some(MediaTagType::Subtitles),
                                "CLOSED-CAPTIONS" => typ = Some(MediaTagType::ClosedCaptions),
                                x => {
                                    Logger::warn(&format!("Unrecognized media type: {}", x));
                                    typ = Some(MediaTagType::Other);
                                },
                            };
                        },
                        "URI" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(parsed) = parsed {
                                let base_uri = Url::new(parsed.to_owned());
                                url = if base_uri.is_absolute() {
                                    Some(base_uri)
                                } else {
                                    Some(Url::from_relative(playlist_base_url, base_uri))
                                };
                            } else {
                                Logger::warn("Unparsable URI value");
                            }
                        },
                        "GROUP-ID" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                group_id = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable GROUP-ID value");
                            }
                        },
                        "LANGUAGE" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                language = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable LANGUAGE value");
                            }
                        },
                        "ASSOC-LANGUAGE" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                assoc_language = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable ASSOC-LANGUAGE value");
                            }
                        },
                        "NAME" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                name = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable NAME value");
                            }
                        },
                        "STABLE-RENDITION-ID" => {
                            let (parsed, end_offset) = parse_quoted_string(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                stable_rendition_id = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable STABLE-RENDITION-ID value");
                            }
                        },
                        "DEFAULT" => {
                            let end_offset = skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            default = true;
                        }
                        "AUTOSELECT" => {
                            let end_offset = skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            autoselect = true;
                        }
                        "FORCED" => {
                            let end_offset = skip_attribute_list_value(media_line, offset + idx + 1);
                            offset = end_offset + 1;
                            forced = true;
                        },
                        "CHANNELS" => {
                            // TODO
                            offset = skip_attribute_list_value(media_line, offset + idx + 1) + 1;
                        },
                        _ => offset = skip_attribute_list_value(media_line, offset + idx + 1) + 1,
                    }
                }
            }
        }

        let typ = if let Some(x) = typ { x } else {
            return Err(MediaTagParsingError::MissingType);
        };
        let group_id = if let Some(x) = group_id { x } else {
            return Err(MediaTagParsingError::MissingGroupId);
        };
        let name = if let Some(x) = name { x } else {
            return Err(MediaTagParsingError::MissingName);
        };

        Ok(MediaTag {
            media_playlist: None,
            typ,
            url,
            group_id,
            language,
            assoc_language,
            name,
            stable_rendition_id,
            default,
            autoselect,
            forced,
            channels,
        })
    }

    pub fn get_url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub fn typ(&self) -> &MediaTagType {
        &self.typ
    }

    pub fn group_id(&self) -> &str {
        &self.group_id
    }

    pub fn is_autoselect(&self) -> bool {
        self.autoselect
    }

    pub fn is_default(&self) -> bool {
        self.default
    }
}
