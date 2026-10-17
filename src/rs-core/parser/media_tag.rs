use super::{
    attribute_list::{parse_enumerated_string, AttributeListIter},
    media_playlist::MediaPlaylistParsingError,
    multi_variant_playlist::MediaPlaylistContext,
    timeline_sync::TimelineReference,
    variable_substitution::{
        parse_substituted_comma_separated_list, parse_substituted_quoted_string, VariableStore,
    },
    MediaPlaylist,
};
use crate::utils::{logger::*, url::Url};
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

    /// List of Uniform Type Identifiers describing semantic characteristics of
    /// the rendition, such as accessibility or commentary-related metadata.
    characteristics: Vec<String>,

    /// Audio sample bit depth, in bits, when known.
    bit_depth: Option<u32>,

    /// Audio sample rate, in hertz, when known.
    sample_rate: Option<u32>,
    // TODO
    // instream_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendition_flags_respect_yes_no_and_absence() {
        let url = Url::new("https://example.com/master.m3u8".to_owned());
        for default in [None, Some("YES"), Some("NO")] {
            for autoselect in [None, Some("YES"), Some("NO")] {
                for forced in [None, Some("YES"), Some("NO")] {
                    let mut line =
                        "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"English\"".to_owned();
                    for (name, value) in [
                        ("DEFAULT", default),
                        ("AUTOSELECT", autoselect),
                        ("FORCED", forced),
                    ] {
                        if let Some(value) = value {
                            line.push_str(&format!(",{name}={value}"));
                        }
                    }
                    let parsed =
                        MediaTag::create(&line, &url, 0, &VariableStore::default()).unwrap();
                    assert_eq!(parsed.default, default == Some("YES"), "{line}");
                    assert_eq!(parsed.autoselect, autoselect == Some("YES"), "{line}");
                    assert_eq!(parsed.forced, forced == Some("YES"), "{line}");
                }
            }
        }
    }
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
        variable_store: &VariableStore,
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

        let mut channels: Option<u32> = None;
        let mut characteristics: Vec<String> = vec![];
        let mut bit_depth: Option<u32> = None;
        let mut sample_rate: Option<u32> = None;

        for item in AttributeListIter::new(media_line, "#EXT-X-MEDIA:".len()) {
            match item.name {
                "TYPE" => {
                    let (parsed, _) = parse_enumerated_string(media_line, item.value_start_offset);
                    match parsed {
                        "AUDIO" => typ = Some(MediaTagType::Audio),
                        "VIDEO" => typ = Some(MediaTagType::Video),
                        "SUBTITLES" => typ = Some(MediaTagType::Subtitles),
                        "CLOSED-CAPTIONS" => typ = Some(MediaTagType::ClosedCaptions),
                        x => {
                            log_warn!("Unrecognized media type: {}", x);
                            typ = Some(MediaTagType::Other);
                        }
                    };
                }
                "URI" => {
                    let parsed = parse_substituted_quoted_string(
                        media_line,
                        item.value_start_offset,
                        variable_store,
                    )
                    .map_err(|_| MediaTagParsingError::Unknown)?;
                    url = Some(Url::new(parsed.into_owned()));
                }
                "GROUP-ID" => {
                    group_id = Some(
                        parse_substituted_quoted_string(
                            media_line,
                            item.value_start_offset,
                            variable_store,
                        )
                        .map_err(|_| MediaTagParsingError::Unknown)?
                        .into_owned(),
                    );
                }
                "LANGUAGE" => {
                    language = Some(
                        parse_substituted_quoted_string(
                            media_line,
                            item.value_start_offset,
                            variable_store,
                        )
                        .map_err(|_| MediaTagParsingError::Unknown)?
                        .into_owned(),
                    );
                }
                "ASSOC-LANGUAGE" => {
                    assoc_language = Some(
                        parse_substituted_quoted_string(
                            media_line,
                            item.value_start_offset,
                            variable_store,
                        )
                        .map_err(|_| MediaTagParsingError::Unknown)?
                        .into_owned(),
                    );
                }
                "NAME" => {
                    name = Some(
                        parse_substituted_quoted_string(
                            media_line,
                            item.value_start_offset,
                            variable_store,
                        )
                        .map_err(|_| MediaTagParsingError::Unknown)?
                        .into_owned(),
                    );
                }
                "STABLE-RENDITION-ID" => {
                    stable_rendition_id = Some(
                        parse_substituted_quoted_string(
                            media_line,
                            item.value_start_offset,
                            variable_store,
                        )
                        .map_err(|_| MediaTagParsingError::Unknown)?
                        .into_owned(),
                    );
                }
                "DEFAULT" => {
                    default =
                        parse_enumerated_string(media_line, item.value_start_offset).0 == "YES";
                }
                "AUTOSELECT" => {
                    autoselect =
                        parse_enumerated_string(media_line, item.value_start_offset).0 == "YES";
                }
                "FORCED" => {
                    forced =
                        parse_enumerated_string(media_line, item.value_start_offset).0 == "YES";
                }
                "CHANNELS" => {
                    // TODO: Preserve recognition of CHANNELS parameters so selection can prefer an
                    // equivalent rendition with recognized parameters, while keeping unknown ones
                    // playable when CODECS is supported (HLS section 6.3.1 SHOULD).
                    let val = parse_substituted_quoted_string(
                        media_line,
                        item.value_start_offset,
                        variable_store,
                    )
                    .map_err(|_| MediaTagParsingError::Unknown)?;
                    match val.split('/').next().unwrap_or("").parse::<u32>() {
                        Ok(parsed_channels) => channels = Some(parsed_channels),
                        Err(_) => log_warn!("Unparsable CHANNELS value"),
                    }
                }
                "CHARACTERISTICS" => {
                    characteristics = parse_substituted_comma_separated_list(
                        media_line,
                        item.value_start_offset,
                        variable_store,
                    )
                    .map_err(|_| MediaTagParsingError::Unknown)?
                    .into_iter()
                    .map(|v| v.into_owned())
                    .collect();
                    characteristics.sort();
                    characteristics.dedup();
                }
                "BIT-DEPTH" => {
                    let (parsed, _) = parse_enumerated_string(media_line, item.value_start_offset);
                    match parsed.parse::<u32>() {
                        Ok(val) => bit_depth = Some(val),
                        Err(_) => log_warn!("Unparsable BIT-DEPTH value"),
                    }
                }
                "SAMPLE-RATE" => {
                    let (parsed, _) = parse_enumerated_string(media_line, item.value_start_offset);
                    match parsed.parse::<u32>() {
                        Ok(val) => sample_rate = Some(val),
                        Err(_) => log_warn!("Unparsable SAMPLE-RATE value"),
                    }
                }
                _ => {}
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
            characteristics,
            bit_depth,
            sample_rate,
        })
    }

    pub(super) fn media_playlist(&self) -> Option<&MediaPlaylist> {
        self.media_playlist.as_ref()
    }

    pub(super) fn media_playlist_mut(&mut self) -> Option<&mut MediaPlaylist> {
        self.media_playlist.as_mut()
    }

    pub(super) fn clear_media_playlist(&mut self) {
        self.media_playlist = None;
    }

    pub(super) fn update(
        &mut self,
        playlist: impl BufRead,
        url: Url,
        timeline_reference: Option<&TimelineReference>,
        context: &MediaPlaylistContext,
    ) -> Result<&MediaPlaylist, MediaPlaylistParsingError> {
        let new_mp = MediaPlaylist::create(
            playlist,
            url,
            self.media_playlist.as_ref(),
            timeline_reference,
            context,
        )?;
        Ok(self.media_playlist.insert(new_mp))
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

    pub(crate) fn is_forced(&self) -> bool {
        self.forced
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

    pub(crate) fn characteristics(&self) -> &[String] {
        self.characteristics.as_slice()
    }

    pub(crate) fn bit_depth(&self) -> Option<u32> {
        self.bit_depth
    }

    pub(crate) fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }
}
