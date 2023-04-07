use std::io::BufRead;

use super::{
    media_playlist::{MediaPlaylist, MediaPlaylistParsingError},
    multi_variant_playlist::MediaPlaylistContext,
    utils::{
        parse_comma_separated_list, parse_decimal_floating_point, parse_decimal_integer,
        parse_enumerated_string, parse_quoted_string, parse_resolution, skip_attribute_list_value,
    },
};
use crate::{bindings::MediaType, utils::url::Url, Logger};

/// Stucture representing the HLS concept of a "variant stream".
#[derive(Debug)]
pub struct VariantStream {
    /// Identifier for the VariantStream unique for the current
    /// `MultivariantPlaylist` object it is a part of.
    id: u32,

    /// Stable identifier for the URI within the parent Multivariant Playlist
    /// even in the case a different Multivariant fetched in the meantime,
    /// whereas `id` may change if the Multivariant Playlist is refreshed.
    ///
    /// This identifier allows the URI of the Variant Stream to
    /// change between two distinct downloads of the Multivariant
    /// Playlist. IDs are matched using a byte-for-byte comparison.
    /// All `VariantStream` in a Multivariant Playlist with the same
    /// `url` value SHOULD use the same `id`.
    stable_id: Option<String>,

    /// Url of the "Media Playlist" corresponding to the main rendition of this
    /// variant stream.
    url: Url,

    /// Media Playlist associated to the main rendition of this variant stream.
    /// `None` if not yet loaded.
    media_playlist: Option<MediaPlaylist>,

    /// The value represents the peak segment bit rate of the Variant Stream.
    ///
    /// If all the Media Segments in a Variant Stream have already been
    /// created, this value MUST be the largest sum of peak segment bit rates
    /// that is produced by any playable combination of Renditions.
    /// (For a Variant Stream with a single Media Playlist, this is just the
    /// peak segment bit rate of that Media Playlist.)
    ///
    /// If the Multivariant Playlist is to be made available before all
    /// Media Segments in the presentation have been encoded, this
    /// value SHOULD be the bandwidth value of a representative period of
    /// similar content, encoded using the same settings.
    bandwidth: u64,

    /// The value represents the average segment bit rate of the Variant Stream.
    /// If all the Media Segments in a Variant Stream have already been
    /// created, the average_bandwidth value MUST be the largest sum of
    /// average segment bit rates that is produced by any playable
    /// combination of Renditions.  (For a Variant Stream with a single
    /// Media Playlist, this is just the average segment bit rate of that
    /// Media Playlist.).
    ///
    /// If the Multivariant Playlist is to be made available before all
    /// Media Segments in the presentation have been encoded, the average_
    /// bandwidth value SHOULD be the average_bandwidth value of a
    /// representative period of similar content, encoded using the same
    /// settings.
    average_bandwitdh: Option<u64>,

    /// This attribute is advisory and indicates the HDCP linked to the current
    /// variant stream.
    ///
    /// The hdcp_level attribute is OPTIONAL.  It SHOULD be present if any
    /// content in the Variant Stream will fail to play without HDCP.
    /// Clients without output copy protection SHOULD NOT load a Variant
    /// Stream with an hdcp_level attribute unless its value is `None`.
    hdcp_level: HdcpLevel,

    /// The dynamic range of the video of this variant stream.
    /// Clients that do not recognize the attribute value SHOULD NOT select the
    /// Variant Stream.
    video_range: VideoDynamicRange,

    /// The value is a list of formats, each associated to a `MediaType`, where
    /// each format specifies a media sample type that is present in one or
    /// more Renditions specified by the Variant Stream.
    ///
    /// Valid format identifiers are those in the ISO Base Media File
    /// Format Name Space defined by "The 'Codecs' and 'Profiles'
    /// Parameters for "Bucket" Media Types" [RFC6381].
    ///
    /// For example, a stream containing AAC low complexity (AAC-LC) audio
    /// and H.264 Main Profile Level 3.0 video would have a codecs value
    /// of "mp4a.40.2,avc1.4d401e".
    ///
    /// Note that if a Variant Stream specifies one or more Renditions
    /// that include IMSC subtitles, the codecs attribute MUST indicate
    /// this with a format identifier such as "stpp.ttml.im1t".
    codecs: Vec<(Option<MediaType>, String)>,

    /// The value is the optimal pixel resolution at which to display all the
    /// video in the Variant Stream.
    resolution: Option<VideoResolution>,

    /// The value uniquely identifies a particular presentation within the scope
    /// of the Playlist file.
    ///
    /// A Playlist file MAY contain multiple variant streams with the
    /// same program_id to identify different encodings of the same
    /// presentation.
    /// These variant playlists MAY contain additional EXT-X- STREAM-INF tags.
    program_id: Option<u64>,

    /// The value is an abstract, relative measure of the playback
    /// quality-of-experience of the Variant Stream.
    ///
    /// The value can be based on any metric or combination of metrics
    /// that can be consistently applied to all Variant Streams. The
    /// value SHOULD consider all media in the Variant Stream, including
    /// video, audio and subtitles. A Variant Stream with a score
    /// attribute MUST be considered by the Playlist author to be more
    /// desirable than any Variant Stream with a lower score attribute in
    /// the same Multivariant Playlist.
    ///
    /// The score attribute is OPTIONAL, but if any Variant Stream
    /// contains the score attribute, then all Variant Streams in the
    /// Multivariant Playlist SHOULD have a score attribute.
    score: Option<f64>,

    /// The value describes the maximum frame rate for all the video in the
    /// Variant Stream, rounded to three decimal places.
    ///
    /// The frame_rate attribute is OPTIONAL but is recommended if the
    /// Variant Stream includes video. The frame_rate attribute SHOULD be
    /// included if any video in a Variant Stream exceeds 30 frames per
    /// second.
    frame_rate: Option<f64>,

    /// The value match the value of the `group_id` attribute of a `Media`
    /// elsewhere in the Multivariant Playlist whose `type` attribute is
    /// `Audio`.
    /// It indicates the set of audio Renditions that SHOULD be used when
    /// playing the presentation.
    audio: Option<String>,

    /// The value match the value of the `group_id` attribute of a `MediaTag`
    /// elsewhere in the Multivariant Playlist whose `typ` attribute is
    /// `Video`.
    /// It indicates the set of video Renditions that SHOULD be used when
    /// playing the presentation.
    video: Option<String>,

    /// The value match the value of the `group_id` attribute of a `MediaTag`
    /// elsewhere in the Multivariant Playlist whose `typ` attribute is
    /// `Subtitles`.
    /// It indicates the set of subtitles Renditions that can be used when
    /// playing the presentation.
    subtitles: Option<String>,

    /// The value match the value of the `group_id` attribute of a `MediaTag`
    /// elsewhere in the Multivariant Playlist whose `typ` attribute is
    /// `ClosedCaptions`.
    /// It indicates the set of closed captions Renditions that can be used
    /// when playing the presentation.
    closed_captions: Option<String>,

    /// The value indicates that the Variant Stream belongs to the identified
    /// Content Steering Pathway.
    /// A value of `None` indicates that the Variant Stream belongs to the
    /// default Pathway, so every Variant Stream can be associated with a named
    /// Pathway.
    pathway_id: Option<String>,

    supported: Option<bool>,

    context: Option<MediaPlaylistContext>,
    // TODO
    // ALLOWED-CPC
    // SUPPLEMENTAL-CODECS
}

/// Pixel resolution of a video content
#[derive(Copy, Clone, Debug)]
pub struct VideoResolution {
    /// Height of the video in pixels
    height: u32,
    /// Width of the video in pixels
    width: u32,
}

impl VideoResolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { height, width }
    }

    pub fn height(self) -> u32 {
        self.height
    }

    pub fn width(self) -> u32 {
        self.width
    }
}

/// Indicate the HDCP level typically enforced by the concerned content.
#[derive(Copy, Clone, Debug)]
enum HdcpLevel {
    /// Indicates that the corresponding content could fail to play unless the
    /// output is protected by High-bandwidth Digital Content Protection (HDCP)
    /// Type 0 [HDCP] or equivalent
    Type0,
    /// Indicates that the corresponding content could fail to play unless the
    /// output is protected by High-bandwidth Digital Content Protection (HDCP)
    /// Type 1 [HDCP] or equivalent
    Type1,
    /// The content does not require output copy protection.
    None,
    /// The HdcpLevel is any other value.
    Unknown,
}

/// Indicate the dynamic range of the video track(s) of the concerned content.
#[derive(Copy, Clone, Debug)]
enum VideoDynamicRange {
    /// The video in the corresponding content is encoded using one of the
    /// following reference opto-electronic transfer characteristic functions
    /// specified by the TransferCharacteristics code point: [CICP] 1, 6, 13,
    /// 14, 15.
    /// Note that different TransferCharacteristics code points can use the
    /// same transfer function.
    Sdr,

    /// The video in the corresponding content is encoded using a reference
    /// opto-electronic transfer characteristic function specified by the
    /// TransferCharacteristics code point 18, or consists of such video mixed
    /// with video qualifying as `Sdr` (see above).
    Hlg,

    /// The video in the corresponding content is encoded using a reference
    /// opto-electronic transfer characteristic function specified by the
    /// TransferCharacteristics code point 16, or consists of such video mixed
    /// with video qualifying as Sdr or Hlg (see above).
    Pq,

    /// The video dynamic range of the current content is any other.
    Unknown,
}

#[derive(Debug)]
pub enum VariantParsingError {
    MissingBandwidth,
    InvalidDecimalInteger,
}

impl VariantStream {
    pub(crate) fn supported(&self) -> Option<bool> {
        self.supported
    }

    pub(crate) fn update_support(&mut self, supported: bool) {
        self.supported = Some(supported);
    }

    pub(crate) fn has_type(&self, media_type: MediaType) -> bool {
        self.codecs
            .iter()
            .any(|c| matches!(c.0, Some(x) if x == media_type))
    }

    pub(crate) fn codecs(&self, media_type: MediaType) -> Option<String> {
        let concerned_codecs = self
            .codecs
            .iter()
            .filter(|c| match c.0 {
                Some(MediaType::Audio) => media_type == MediaType::Audio || self.audio.is_none(),
                Some(MediaType::Video) => media_type == MediaType::Video,
                _ => false,
            })
            .map(|c| c.1.as_ref())
            .collect::<Vec<&str>>();
        if concerned_codecs.is_empty() {
            None
        } else {
            Some(concerned_codecs.join(","))
        }
    }

    pub(crate) fn resolution(&self) -> Option<&VideoResolution> {
        self.resolution.as_ref()
    }

    pub(crate) fn frame_rate(&self) -> Option<f64> {
        self.frame_rate
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn stable_id(&self) -> Option<&str> {
        self.stable_id.as_deref()
    }

    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    pub(crate) fn score(&self) -> Option<f64> {
        self.score
    }

    pub(crate) fn bandwidth(&self) -> u64 {
        self.bandwidth
    }

    pub(super) fn media_playlist(&self) -> Option<&MediaPlaylist> {
        self.media_playlist.as_ref()
    }

    pub(super) fn update_media_playlist(
        &mut self,
        playlist: impl BufRead,
        url: Url,
        context: Option<&MediaPlaylistContext>,
    ) -> Result<&MediaPlaylist, MediaPlaylistParsingError> {
        let new_mp = MediaPlaylist::create(playlist, url, self.media_playlist.as_ref(), context)?;
        self.media_playlist = Some(new_mp);
        Ok(self.media_playlist.as_ref().unwrap())
    }

    pub(super) fn get_url(&self) -> &Url {
        &self.url
    }

    pub(super) fn audio_group(&self) -> Option<&str> {
        self.audio.as_deref()
    }

    pub(super) fn create_from_stream_inf(
        variant_line: &str,
        url: Url,
        base_uri: &str,
        id: u32,
    ) -> Result<Self, VariantParsingError> {
        let mut bandwidth: Option<u64> = None;
        let mut resolution: Option<VideoResolution> = None;
        let mut average_bandwitdh: Option<u64> = None;
        let mut codecs: Vec<(Option<MediaType>, String)> = vec![];
        let mut hdcp_level: HdcpLevel = HdcpLevel::None;
        let mut video_range: VideoDynamicRange = VideoDynamicRange::Sdr;
        let mut program_id: Option<u64> = None;
        let mut score: Option<f64> = None;
        let mut frame_rate: Option<f64> = None;
        let mut stable_variant_id: Option<String> = None;
        let mut audio: Option<String> = None;
        let mut video: Option<String> = None;
        let mut subtitles: Option<String> = None;
        let mut closed_captions: Option<String> = None;
        let mut pathway_id: Option<String> = None;

        let mut offset = "#EXT-X-STREAM-INF:".len();
        loop {
            if offset >= variant_line.len() {
                break;
            }
            // #EXT-X-STREAM-INF:AVERAGE-BANDWIDTH=746000,BANDWIDTH=886211,RESOLUTION=512x288,FRAME-RATE=25.000,CODECS="avc1.4D4015,mp4a.40.2",CLOSED-CAPTIONS=NONE,AUDIO="AUDIO_96000"
            match variant_line[offset..].find('=') {
                None => {
                    Logger::warn("Attribute Name not followed by equal sign");
                    break;
                }
                Some(idx) => match &variant_line[offset..offset + idx] {
                    "AVERAGE-BANDWIDTH" => {
                        let (parsed, end_offset) =
                            parse_decimal_integer(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            average_bandwitdh = Some(val);
                        } else {
                            Logger::warn("Unparsable AVERAGE-BANDWIDTH value");
                        }
                    }
                    "BANDWIDTH" => {
                        let (parsed, end_offset) =
                            parse_decimal_integer(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            bandwidth = Some(val);
                        } else {
                            Logger::warn("Unparsable BANDWIDTH value");
                        }
                    }
                    "CODECS" => {
                        let (parsed, end_offset) =
                            parse_comma_separated_list(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            codecs = val
                                .iter()
                                .map(|c| (guess_media_type_from_codec(c), (*c).to_owned()))
                                .collect();
                        } else {
                            Logger::warn("Unparsable CODECS value");
                        }
                    }
                    "FRAME-RATE" => {
                        let (parsed, end_offset) =
                            parse_decimal_floating_point(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            frame_rate = Some(val);
                        } else {
                            Logger::warn("Unparsable FRAME-RATE value");
                        }
                    }
                    "HDCP-LEVEL" => {
                        let (parsed, end_offset) =
                            parse_enumerated_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        hdcp_level = match parsed {
                            "TYPE-0" => HdcpLevel::Type0,
                            "TYPE-1" => HdcpLevel::Type1,
                            "NONE" => HdcpLevel::None,
                            _ => HdcpLevel::Unknown,
                        };
                    }
                    "PROGRAM-ID" => {
                        let (parsed, end_offset) =
                            parse_decimal_integer(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            program_id = Some(val);
                        } else {
                            Logger::warn("Unparsable PROGRAM-ID value");
                        }
                    }
                    "RESOLUTION" => {
                        let (parsed, end_offset) = parse_resolution(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(res) = parsed {
                            resolution = Some(VideoResolution {
                                height: res.height,
                                width: res.width,
                            });
                        } else {
                            Logger::warn("Unparsable RESOLUTION value");
                        }
                    }
                    "SCORE" => {
                        let (parsed, end_offset) =
                            parse_decimal_floating_point(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            score = Some(val);
                        } else {
                            Logger::warn("Unparsable SCORE value");
                        }
                    }
                    "STABLE-VARIANT-ID" => {
                        let (parsed, end_offset) =
                            parse_quoted_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            stable_variant_id = Some(val.to_owned());
                        } else {
                            Logger::warn("Unparsable STABLE-VARIANT-ID value");
                        }
                    }
                    "AUDIO" => {
                        let (parsed, end_offset) =
                            parse_quoted_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            audio = Some(val.to_owned());
                        } else {
                            Logger::warn("Unparsable AUDIO value");
                        }
                    }
                    "VIDEO" => {
                        let (parsed, end_offset) =
                            parse_quoted_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            video = Some(val.to_owned());
                        } else {
                            Logger::warn("Unparsable VIDEO value");
                        }
                    }
                    "SUBTITLES" => {
                        let (parsed, end_offset) =
                            parse_quoted_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            subtitles = Some(val.to_owned());
                        } else {
                            Logger::warn("Unparsable SUBTITLES value");
                        }
                    }
                    "CLOSED-CAPTIONS" => {
                        if variant_line[offset + idx + 1..].starts_with("NONE") {
                            offset = skip_attribute_list_value(variant_line, offset + idx + 1);
                        } else {
                            let (parsed, end_offset) =
                                parse_quoted_string(variant_line, offset + idx + 1);
                            offset = end_offset + 1;
                            if let Ok(val) = parsed {
                                closed_captions = Some(val.to_owned());
                            } else {
                                Logger::warn("Unparsable CLOSED-CAPTIONS value");
                            }
                        }
                    }
                    "VIDEO-RANGE" => {
                        let (parsed, end_offset) =
                            parse_enumerated_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        video_range = match parsed {
                            "SDR" => VideoDynamicRange::Sdr,
                            "HLG" => VideoDynamicRange::Hlg,
                            "PQ" => VideoDynamicRange::Pq,
                            _ => VideoDynamicRange::Unknown,
                        };
                    }
                    "PATHWAY-ID" => {
                        let (parsed, end_offset) =
                            parse_quoted_string(variant_line, offset + idx + 1);
                        offset = end_offset + 1;
                        if let Ok(val) = parsed {
                            pathway_id = Some(val.to_owned());
                        } else {
                            Logger::warn("Unparsable PATHWAY-ID value");
                        }
                    }
                    _ => {
                        offset = skip_attribute_list_value(variant_line, offset + idx + 1) + 1;
                    }
                },
            }
        }

        let url = if url.is_absolute() {
            url
        } else {
            Url::from_relative(base_uri, url)
        };
        if let Some(bandwidth) = bandwidth {
            Ok(Self {
                id,
                stable_id: stable_variant_id,
                audio,
                average_bandwitdh,
                bandwidth,
                closed_captions,
                codecs,
                frame_rate,
                hdcp_level,
                media_playlist: None,
                pathway_id,
                program_id,
                resolution,
                score,
                subtitles,
                url,
                video,
                video_range,
                context: None,
                supported: None,
            })
        } else {
            Err(VariantParsingError::MissingBandwidth)
        }
    }

    pub(super) fn communicate_context(&mut self, context: MediaPlaylistContext) {
        self.context = Some(context);
    }
}

fn guess_media_type_from_codec(codec: &str) -> Option<MediaType> {
    let (base, _) = split_codec(codec);

    match base {
        "mp4a" => Some(MediaType::Audio),
        "ec-3" | "ac-3" => Some(MediaType::Audio),
        "avc1" | "avc3" => Some(MediaType::Video),
        "hvc1" | "hev1" => Some(MediaType::Video),
        "dvh1" | "dvhe" => Some(MediaType::Video),
        _ => None,
    }
}

fn split_codec(codec: &str) -> (&str, &str) {
    let position = codec.find('.');
    if let Some(position) = position {
        let both_parts = codec.split_at(position);
        (both_parts.0, both_parts.1.split_at(0).1)
    } else {
        (codec, "")
    }
}
