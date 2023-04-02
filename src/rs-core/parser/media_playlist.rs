use crate::{bindings::MediaType, utils::url::Url, Logger};
use std::{error, fmt, io::BufRead};

use super::{
    multi_variant_playlist::MediaPlaylistContext,
    utils::{
        parse_byte_range, parse_decimal_floating_point, parse_decimal_integer,
        parse_enumerated_string, parse_iso_8601_date, parse_quoted_string,
    },
};

pub use super::utils::ByteRange;

#[derive(Clone, Debug)]
pub(crate) struct SegmentTimeInfo {
    start: f64,
    duration: f64,
}

impl SegmentTimeInfo {
    pub(crate) fn new(start: f64, duration: f64) -> Self {
        Self { start, duration }
    }

    pub(crate) fn start(&self) -> f64 {
        self.start
    }

    pub(crate) fn end(&self) -> f64 {
        self.start + self.duration
    }

    pub(crate) fn duration(&self) -> f64 {
        self.duration
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SegmentList {
    init: Option<InitSegmentInfo>,
    media: Vec<SegmentInfo>,
}

impl SegmentList {
    fn new(init: Option<InitSegmentInfo>, media: Vec<SegmentInfo>) -> Self {
        Self { init, media }
    }

    pub(crate) fn init(&self) -> Option<&InitSegmentInfo> {
        self.init.as_ref()
    }

    pub(crate) fn media(&self) -> &[SegmentInfo] {
        self.media.as_slice()
    }
}

/// Structure representing the concept of the `Media Playlist` in HLS.
#[derive(Clone, Debug)]
pub struct MediaPlaylist {
    version: Option<u32>,
    independent_segments: bool,
    start: Option<StartAttribute>,
    target_duration: u32,
    media_sequence: u32,
    end_list: bool,
    playlist_type: PlaylistType,
    i_frames_only: bool,
    segment_list: SegmentList,
    url: Url,
    // TODO
    // pub server_control: ServerControl,
    // pub part_inf: Option<f64>,

    // ignored
    // pub discontinuity_sequence: Option<u32>;
}

#[derive(Clone, Debug)]
pub(crate) struct InitSegmentInfo {
    uri: Url,
    byte_range: Option<ByteRange>,
}

impl InitSegmentInfo {
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SegmentInfo {
    time_info: SegmentTimeInfo,
    byte_range: Option<ByteRange>,
    url: Url,
}

impl SegmentInfo {
    pub(crate) fn start(&self) -> f64 {
        self.time_info.start()
    }

    pub(crate) fn end(&self) -> f64 {
        self.time_info.end()
    }

    pub(crate) fn duration(&self) -> f64 {
        self.time_info.duration()
    }

    pub(crate) fn time_info(&self) -> &SegmentTimeInfo {
        &self.time_info
    }

    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistType {
    Event,
    VoD,
    None,
}

#[derive(Clone, Debug)]
pub struct StartAttribute {
    time_offset: f64,
    precise: bool,
}

// #[derive(Clone, Debug)]
// pub struct ServerControl {
//     can_skip_until: Option<f64>,
//     can_skip_dateranges: bool,
//     hold_back: u32,
//     part_hold_back: Option<u32>,
//     can_block_reload: bool,
// }

#[derive(Debug)]
pub enum MediaPlaylistParsingError {
    UnparsableExtInf,
    UnparsableByteRange,
    UriMissingInMap,
    MissingTargetDuration,
    UriWithoutExtInf,
}

impl fmt::Display for MediaPlaylistParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MediaPlaylistParsingError::UnparsableExtInf => {
                write!(f, "One of the #EXTINF value could not be parsed")
            }
            MediaPlaylistParsingError::UriMissingInMap => {
                write!(f, "An #EXT-X-MAP was missing its mandatory URI attribute")
            }
            MediaPlaylistParsingError::MissingTargetDuration => {
                write!(f, "Missing mandatory TARGETDURATION attribute")
            }
            MediaPlaylistParsingError::UriWithoutExtInf => {
                write!(f, "One of the uri was not linked to any #EXTINF")
            }
            MediaPlaylistParsingError::UnparsableByteRange => {
                write!(f, "One of the uri had an Unparsable BYTERANGE")
            }
        }
    }
}

impl error::Error for MediaPlaylistParsingError {}

impl MediaPlaylist {
    pub(crate) fn create(
        playlist: impl BufRead,
        url: Url,
        context: Option<&MediaPlaylistContext>,
    ) -> Result<Self, MediaPlaylistParsingError> {
        let mut version: Option<u32> = None;
        let mut independent_segments = false;
        let mut target_duration: Option<u32> = None;
        let mut media_sequence = 0;
        let mut end_list = false;
        let mut playlist_type = PlaylistType::None;
        let mut i_frames_only = false;
        let mut map: Option<InitSegmentInfo> = None;
        let mut start = None;

        let playlist_base_url = url.pathname();

        let mut curr_start_time = 0.;
        let mut media_segments: Vec<SegmentInfo> = vec![];
        let mut next_segment_duration: Option<f64> = None;
        let mut current_byte: Option<usize> = None;
        let mut next_segment_byte_range: Option<ByteRange> = None;

        let lines = playlist.lines();
        for line in lines {
            let str_line = line.unwrap();
            if str_line.is_empty() {
                continue;
            } else if let Some(stripped) = str_line.strip_prefix("#EXT") {
                let colon_idx = match stripped.find(':') {
                    None => str_line.len(),
                    Some(idx) => idx + 4,
                };

                match &str_line[4..colon_idx] {
                    "-X-VERSION" => match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                        Ok(v) if v <= (u32::MAX as u64) => version = Some(v as u32),
                        _ => Logger::warn("Unparsable VERSION value"),
                    },
                    "-X-TARGETDURATION" => {
                        match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                            Ok(t) if t <= (u32::MAX as u64) => target_duration = Some(t as u32),
                            _ => Logger::warn("Unparsable TARGETDURATION value"),
                        }
                    }
                    "-X-ENDLIST" => end_list = true,
                    "-X-INDEPENDENT-SEGMENTS" => independent_segments = true,
                    "-X-START:" =>
                        /* TODO */
                        {}
                    "INF" => match parse_decimal_floating_point(&str_line, 4 + "INF:".len()).0 {
                        Ok(d) => next_segment_duration = Some(d),
                        Err(_) => return Err(MediaPlaylistParsingError::UnparsableExtInf),
                    },
                    "-X-BYTERANGE" => {
                        match parse_byte_range(&str_line, 5 + "-X-BYTERANGE".len(), current_byte) {
                            Some(br) => {
                                current_byte = Some(br.last_byte + 1);
                                next_segment_byte_range = Some(br);
                            }
                            _ => {
                                return Err(MediaPlaylistParsingError::UnparsableByteRange);
                            }
                        }
                    }
                    "-X-MEDIA-SEQUENCE" => {
                        match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                            Ok(s) if s <= (u32::MAX as u64) => media_sequence = s as u32,
                            _ => Logger::warn("Unparsable MEDIA-SEQUENCE value"),
                        }
                    }
                    "-X-PLAYLIST-TYPE" => match parse_enumerated_string(&str_line, colon_idx + 1).0
                    {
                        "EVENT" => playlist_type = PlaylistType::Event,
                        "VOD" => playlist_type = PlaylistType::VoD,
                        x => {
                            Logger::warn(&format!("Unrecognized playlist type: {}", x));
                            playlist_type = PlaylistType::None;
                        }
                    },
                    "-X-PROGRAM-DATE-TIME" => {
                        if let Some(date) = parse_iso_8601_date(&str_line, colon_idx + 1) {
                            curr_start_time = date;
                        }
                    }
                    "-X-I-FRAMES-ONLY" => i_frames_only = true,
                    "-X-MAP" => {
                        let mut map_info_url: Option<Url> = None;
                        let mut map_info_byte_range: Option<ByteRange> = None;
                        let mut base_offset = colon_idx + 1;
                        loop {
                            if base_offset >= str_line.len() {
                                break;
                            }
                            match str_line[base_offset..].find('=') {
                                None => {
                                    Logger::warn("Attribute Name not followed by equal sign");
                                    break;
                                }
                                Some(idx) => match &str_line[base_offset..base_offset + idx] {
                                    "URI" => {
                                        let (parsed, end_offset) =
                                            parse_quoted_string(&str_line, base_offset + idx + 1);
                                        base_offset = end_offset + 1;
                                        if let Ok(val) = parsed {
                                            let init_url = Url::new(val.to_owned());
                                            let init_url = if init_url.is_absolute() {
                                                init_url
                                            } else {
                                                Url::from_relative(playlist_base_url, init_url)
                                            };
                                            map_info_url = Some(init_url);
                                        }
                                    }

                                    "BYTERANGE" => {
                                        let (parsed, end_offset) =
                                            parse_quoted_string(&str_line, base_offset + idx + 1);
                                        base_offset = end_offset + 1;
                                        if let Ok(val) = parsed {
                                            match parse_byte_range(val, 0, None) {
                                                Some(br) => {
                                                    current_byte = Some(br.last_byte + 1);
                                                    map_info_byte_range = Some(br);
                                                }
                                                _ => return Err(
                                                    MediaPlaylistParsingError::UnparsableByteRange,
                                                ),
                                            };
                                        }
                                    }
                                    _ => {}
                                },
                            }
                        }
                        if let Some(url) = map_info_url {
                            map = Some(InitSegmentInfo {
                                uri: url,
                                byte_range: map_info_byte_range,
                            });
                        } else {
                            return Err(MediaPlaylistParsingError::UriMissingInMap);
                        }
                    }
                    "M3U" => {}
                    x => Logger::debug(&format!("Unrecognized tag: \"{}\"", x)),
                }
            } else if str_line.starts_with('#') {
                continue;
            } else {
                // URI
                let seg_url = Url::new(str_line);
                let seg_url = if seg_url.is_absolute() {
                    seg_url
                } else {
                    Url::from_relative(playlist_base_url, seg_url)
                };
                if let Some(duration) = next_segment_duration {
                    let seg = SegmentInfo {
                        time_info: SegmentTimeInfo::new(curr_start_time, duration),
                        byte_range: next_segment_byte_range,
                        url: seg_url,
                    };
                    media_segments.push(seg);
                    curr_start_time += duration;
                    next_segment_duration = None;
                    next_segment_byte_range = None;
                } else {
                    return Err(MediaPlaylistParsingError::UriWithoutExtInf);
                }
            }
        }

        let target_duration = match target_duration {
            Some(target_duration) => target_duration,
            None => return Err(MediaPlaylistParsingError::MissingTargetDuration),
        };
        if start.is_none() {
            start = context.and_then(|x| x.start().cloned())
        }
        if !independent_segments {
            independent_segments = context
                .and_then(|x| x.independent_segments())
                .unwrap_or(false);
        }
        Ok(MediaPlaylist {
            version,
            independent_segments,
            start,
            target_duration,
            media_sequence,
            end_list,
            playlist_type,
            i_frames_only,
            segment_list: SegmentList::new(map, media_segments),
            url,
            // TODO
            // server_control,
            // part_inf,
        })
    }

    pub(crate) fn wanted_start(&self) -> Option<f64> {
        self.start
            .as_ref()
            .and_then(|start| {
                let actual_offset = if start.time_offset < 0. {
                    self.segment_list
                        .media()
                        .last()
                        .map(|s| s.end() + start.time_offset)
                } else {
                    self.segment_list
                        .media()
                        .first()
                        .map(|s| s.start() + start.time_offset)
                };
                actual_offset.map(|a| (a, start.precise))
            })
            .and_then(|(actual_time, is_precise)| {
                if is_precise {
                    Some(actual_time)
                } else {
                    self.segment_from_pos(actual_time).map(|s| s.start())
                }
            })
    }

    pub(crate) fn refresh_interval(&self) -> Option<f64> {
        if self.may_be_refreshed() {
            Some(f64::from(self.target_duration) * 1000.)
        } else {
            None
        }
    }

    pub(crate) fn segment_list(&self) -> &SegmentList {
        &self.segment_list
    }

    pub(crate) fn target_duration(&self) -> f64 {
        self.target_duration as f64
    }

    //     pub(crate) fn init_segment(&self) -> Option<&InitSegmentInfo> {
    //         self.map.as_ref()
    //     }

    //     pub(crate) fn segment_list(&self) -> &[SegmentInfo] {
    //         &self.segment_list
    //     }

    pub(crate) fn beginning(&self) -> Option<f64> {
        self.segment_list.media().first().map(|s| s.start())
    }

    pub(crate) fn ending(&self) -> Option<f64> {
        self.segment_list.media().last().map(|s| s.end())
    }

    pub(crate) fn may_be_refreshed(&self) -> bool {
        !self.end_list && self.playlist_type != PlaylistType::VoD
    }

    pub(crate) fn is_live(&self) -> bool {
        !self.end_list && self.playlist_type == PlaylistType::None
    }

    pub(crate) fn is_ended(&self) -> bool {
        self.end_list
    }

    /// TODO kind of weird to give the MediaType here
    pub(crate) fn mime_type(&self, media_type: MediaType) -> Option<&str> {
        match media_type {
            MediaType::Audio => match self.extension() {
                Some("mp4") => Some("audio/mp4"),
                Some("mp4a") => Some("audio/mp4"),
                Some("m4s") => Some("audio/mp4"),
                Some("m4i") => Some("audio/mp4"),
                Some("m4a") => Some("audio/mp4"),
                Some("m4f") => Some("audio/mp4"),
                Some("cmfa") => Some("audio/mp4"),
                Some("aac") => Some("audio/aac"),
                Some("ac3") => Some("audio/ac3"),
                Some("ec3") => Some("audio/ec3"),
                Some("mp3") => Some("audio/mpeg"),

                // MPEG2-TS also uses video/ for audio
                Some("ts") => Some("video/mp2t"),
                _ => None,
            },
            MediaType::Video => match self.extension() {
                Some("mp4") => Some("video/mp4"),
                Some("mp4v") => Some("video/mp4"),
                Some("m4s") => Some("video/mp4"),
                Some("m4i") => Some("video/mp4"),
                Some("m4v") => Some("video/mp4"),
                Some("m4f") => Some("video/mp4"),
                Some("cmfv") => Some("video/mp4"),
                Some("ts") => Some("video/mp2t"),
                _ => None,
            },
        }
    }

    pub(super) fn url(&self) -> &Url {
        &self.url
    }

    fn extension(&self) -> Option<&str> {
        self.segment_list.media().get(0).map(|s| s.url.extension())
    }

    fn segment_from_pos(&self, pos: f64) -> Option<&SegmentInfo> {
        self.segment_list
            .media()
            .iter()
            .find(|s| s.end() > pos && s.start() <= pos)
    }
}
