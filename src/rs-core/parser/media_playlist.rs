use crate::{bindings::MediaType, utils::url::Url, Logger};
use std::{error, fmt, io::BufRead};

use super::{
    multi_variant_playlist::MediaPlaylistContext,
    utils::{
        parse_byte_range, parse_decimal_floating_point, parse_decimal_integer,
        parse_enumerated_string, parse_iso_8601_date, parse_quoted_string, parse_start_attribute,
        StartAttribute,
    },
};

pub use super::utils::ByteRange;

/// Object storing the time information on a single media segment.
#[derive(Clone, Debug)]
pub(crate) struct SegmentTimeInfo {
    /// First presentation time the segment contains media data for, in seconds.
    start: f64,
    /// Difference between the last presentation time at which the segment contains data for
    /// in seconds and `start`.
    duration: f64,
}

impl SegmentTimeInfo {
    /// Create a new `SegmentTimeInfo` with the given `start` and `duration` in seconds.
    pub(crate) fn new(start: f64, duration: f64) -> Self {
        Self { start, duration }
    }

    /// First presentation time the segment contains media data for, in seconds.
    pub(crate) fn start(&self) -> f64 {
        self.start
    }

    /// Last presentation time the segment contains media data for, in seconds.
    pub(crate) fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Difference between `self.end()` and `self.start()`
    pub(crate) fn duration(&self) -> f64 {
        self.duration
    }
}

/// List of all segments a `MediaPlaylist` is associated to.
#[derive(Clone, Debug)]
pub(crate) struct SegmentList {
    /// Initialization segments a `MediaPlaylist` is associated to, ordered chronologically
    /// (initialization linked to earlier media segments are set first).
    init: Vec<InitSegmentInfo>,
    /// Initialization segments a `MediaPlaylist` is associated to, in chronological order.
    media: Vec<MediaSegmentInfo>,
}

impl SegmentList {
    /// Create a new `SegmentList` linked to the given initialization segment list and media
    /// segment list, both in chronological order.
    fn new(init: Vec<InitSegmentInfo>, media: Vec<MediaSegmentInfo>) -> Self {
        Self { init, media }
    }

    /// Returns a reference to the potential initialization segment linked to the given media
    /// segment information.
    ///
    /// Returns `None` if the given media segment isn't linked to an initialization segment.
    pub(crate) fn init_for(&self, seg: &MediaSegmentInfo) -> Option<&InitSegmentInfo> {
        self.init
            .iter()
            .rev()
            .find(|i| i.start <= seg.time_info.start)
    }

    /// Returns the list of media segments associated to this `SegmentList` in chronological order.
    pub(crate) fn media(&self) -> &[MediaSegmentInfo] {
        self.media.as_slice()
    }
}

/// Information linked to an initialization segment.
#[derive(Clone, Debug)]
pub(crate) struct InitSegmentInfo {
    /// First segment's start time in seconds to which that initialization segment applies.
    start: f64,
    /// URL through which that initialization segment may be requested.
    url: Url,
    /// If set, byte-range to specifically request only the initialization segment at the given
    /// `url`.
    byte_range: Option<ByteRange>,
}

impl InitSegmentInfo {
    /// Returns an identifier allowing to compare the initialization segment behind this
    /// `InitSegmentInfo` to other `InitSegmentInfo` objects coming from the same `MediaPlaylist`.
    pub(crate) fn id(&self) -> f64 {
        self.start
    }

    /// If set, byte-range at which the initialization segment should be requested.
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    /// URL at which the initialization segment should be requested.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}

/// Information linked to a single media segment.
#[derive(Clone, Debug)]
pub(crate) struct MediaSegmentInfo {
    /// Information on the time boundaries of that segment.
    ///
    /// It should be exclusive to the time boundaries of all other segments in this Media Playlist.
    time_info: SegmentTimeInfo,
    /// URL through which that media segment may be requested.
    url: Url,
    /// If set, byte-range to specifically request only the media segment at the given `url`.
    byte_range: Option<ByteRange>,
}

impl MediaSegmentInfo {
    /// First presentation time the segment contains media data for, in seconds.
    pub(crate) fn start(&self) -> f64 {
        self.time_info.start()
    }

    /// Last presentation time the segment contains media data for, in seconds.
    pub(crate) fn end(&self) -> f64 {
        self.time_info.end()
    }

    /// Difference between `self.end()` and `self.start()`
    pub(crate) fn duration(&self) -> f64 {
        self.time_info.duration()
    }

    /// Returns reference to the whole `SegmentTimeInfo` object linked to this media segment.
    pub(crate) fn time_info(&self) -> &SegmentTimeInfo {
        &self.time_info
    }

    /// If set, byte-range at which this media segment should be requested.
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    /// URL at which this initialization segment should be requested.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}

/// Values for the "Playlist Type" as specified by the HLS specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistType {
    Event,
    VoD,
    None,
}

// #[derive(Clone, Debug)]
// pub struct ServerControl {
//     can_skip_until: Option<f64>,
//     can_skip_dateranges: bool,
//     hold_back: u32,
//     part_hold_back: Option<u32>,
//     can_block_reload: bool,
// }

/// Errors that may arise when parsing a Media Playlist
///
/// See display implementation for more information on its variants.
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

/// Structure representing the concept of the `Media Playlist` in HLS.
///
/// It lists Initialization and media segments linked to a rendition, as well as some of its
/// characteristics.
#[derive(Clone, Debug)]
pub struct MediaPlaylist {
    /// The HLS compatibility version of the corresponding Media Playlist.
    version: Option<u32>,
    /// Indicates that all media samples in a media segment can be decoded without information
    /// from other segments.
    /// It applies to every media segment in the Playlist.
    independent_segments: bool,
    /// Preferred start time when playing this media playlist.
    start: Option<StartAttribute>,
    /// Upper bound on the duration of all media segments in the Playlist, in seconds.
    target_duration: u32,
    /// Media sequence number of the first media segment in `segment_list`.
    media_sequence: u32,
    /// If `true`,  no more Media Segments will be added to the Media Playlist file.
    end_list: bool,
    /// Mutability information about the Media Playlist file.
    playlist_type: PlaylistType,
    /// If `true`, each media segment in the Playlist describes a single I-frame.
    ///
    /// I-frames are encoded video frames whose decoding does not depend on any other frame.
    /// I-frame Playlists can be used for trick play, such as fast forward, rapid reverse, and
    /// scrubbing.
    i_frames_only: bool,
    /// List all initialization segments and media segments reachable through this `MediaPlaylist`.
    segment_list: SegmentList,
    /// URL at which this Media Playlist may be updated.
    url: Url,
    // TODO
    // pub server_control: ServerControl,
    // pub part_inf: Option<f64>,

    // ignored
    // pub discontinuity_sequence: Option<u32>;
}

impl MediaPlaylist {
    /// Create a new `MediaPlaylist` object, by giving it a `BufRead` reading into its
    /// corresponding Media Playlist file from its very beginning.
    pub(crate) fn create(
        playlist: impl BufRead,
        url: Url,
        prev_playlist: Option<&MediaPlaylist>,
        context: &MediaPlaylistContext,
    ) -> Result<Self, MediaPlaylistParsingError> {
        let mut version: Option<u32> = None;
        let mut independent_segments = false;
        let mut target_duration: Option<u32> = None;
        let mut media_sequence = 0;
        let mut end_list = false;
        let mut playlist_type = PlaylistType::None;
        let mut i_frames_only = false;
        let mut last_incomplete_map = None;
        let mut maps_info: Vec<InitSegmentInfo> = vec![];
        let mut start = None;
        let mut skip_next_segment = false;

        let playlist_base_url = url.pathname();

        let mut curr_start_time = 0.;
        let mut media_segments: Vec<MediaSegmentInfo> = vec![];
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
                    "-X-GAP" => {
                        skip_next_segment = true;
                    }
                    "-X-ENDLIST" => end_list = true,
                    "-X-INDEPENDENT-SEGMENTS" => independent_segments = true,
                    "-X-START" => match parse_start_attribute(&str_line) {
                        Ok(st) => {
                            start = Some(st);
                        }
                        _ => {
                            Logger::warn("Parser: Failed to parse `EXT-X-START` attribute");
                        }
                    },
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
                            last_incomplete_map = Some((url, map_info_byte_range));
                        } else {
                            return Err(MediaPlaylistParsingError::UriMissingInMap);
                        }
                    }
                    "M3U" => {}
                    x => Logger::debug(&format!("Unrecognized tag: \"{}\"", x)),
                }
            } else if str_line.starts_with('#') {
                continue;
            } else if skip_next_segment {
                skip_next_segment = false;
                if let Some(duration) = next_segment_duration {
                    curr_start_time += duration;
                    next_segment_duration = None;
                    next_segment_byte_range = None;
                } else {
                    return Err(MediaPlaylistParsingError::UriWithoutExtInf);
                }
            } else {
                // URI
                let seg_url = Url::new(str_line);
                let seg_url = if seg_url.is_absolute() {
                    seg_url
                } else {
                    Url::from_relative(playlist_base_url, seg_url)
                };
                if let Some(duration) = next_segment_duration {
                    let seg = MediaSegmentInfo {
                        time_info: SegmentTimeInfo::new(curr_start_time, duration),
                        byte_range: next_segment_byte_range,
                        url: seg_url,
                    };
                    if let Some((url, byte_range)) = last_incomplete_map {
                        last_incomplete_map = None;
                        let init_start = prev_playlist
                            .and_then(|p| {
                                p.segment_list
                                    .init
                                    .iter()
                                    .find(|s| s.url == url && s.byte_range == byte_range)
                                    .map(|i| i.start)
                            })
                            .unwrap_or(curr_start_time);
                        maps_info.push(InitSegmentInfo {
                            start: init_start,
                            url,
                            byte_range,
                        });
                    }
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

        // Turns out that MultivariantPlaylist attibutes have priority here
        if let Some(st) = context.start() {
            start = Some(st.clone());
        }
        if let Some(indep) = context.independent_segments() {
            independent_segments = indep;
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
            segment_list: SegmentList::new(maps_info, media_segments),
            url,
            // TODO
            // server_control,
            // part_inf,
        })
    }

    /// Returns the second, in playlist time, at which the current content should be started at
    /// according to the Media Playlist, or `None` if the Media Playlist doesn't have any
    /// preference.
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

    /// Returns the amount of milliseconds at which the Media Playlist should probably be requested.
    ///
    /// This value may change each time the Media Playlist is updated.
    ///
    /// Returns `None` if this `MediaPlaylist` should never be refreshed.
    pub(crate) fn refresh_interval(&self) -> Option<f64> {
        if self.may_be_refreshed() {
            Some(
                self.segment_list
                    .media()
                    .last()
                    .map(|s| s.duration() * 1.1)
                    .unwrap_or(f64::from(self.target_duration / 2))
                    * 1000.,
            )
        } else {
            None
        }
    }

    /// Returns `SegmentList` associated to this `MediaPlaylist` allowing to check which
    /// initialization and media segments have to be loaded next.
    pub(crate) fn segment_list(&self) -> &SegmentList {
        &self.segment_list
    }

    /// Returns the upper bound on the duration of all media segments in the Playlist, in seconds.
    pub(crate) fn target_duration(&self) -> f64 {
        self.target_duration as f64
    }

    /// Returns the start time of the first media segment referenced in that `MediaPlaylist`, in
    /// seconds.
    pub(crate) fn beginning(&self) -> Option<f64> {
        self.segment_list.media().first().map(|s| s.start())
    }

    /// Returns the ending time of the last media segment referenced in that `MediaPlaylist`, in
    /// seconds.
    pub(crate) fn ending(&self) -> Option<f64> {
        self.segment_list.media().last().map(|s| s.end())
    }

    /// Returns `true` if the `MediaPlaylist` may need to be refreshed later, `false` if it should
    /// not.
    pub(crate) fn may_be_refreshed(&self) -> bool {
        !self.end_list && self.playlist_type != PlaylistType::VoD
    }

    /// Returns `true` if the `MediaPlaylist` is linked to a "live content" which is an unfinished
    /// content that may need to be played close to its maximum position.
    pub(crate) fn is_live(&self) -> bool {
        !self.end_list && self.playlist_type == PlaylistType::None
    }

    /// Returns `true` if the last segment referenced in this `MediaPlaylist` can be assumed to be
    /// the last chronological one.
    pub(crate) fn is_ended(&self) -> bool {
        self.end_list
    }

    /// Return Mime-type associated to this MediaPlaylist.
    ///
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

    /// Returns the URL at which this Media Playlist may be requested.
    pub(super) fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the "extension" part of the media segments referenced in this Media Playlist (e.g.
    /// `"mp4"` for `.mp4` files).
    ///
    /// Returns `None` if unknown.
    fn extension(&self) -> Option<&str> {
        self.segment_list.media().get(0).map(|s| s.url.extension())
    }

    /// Returns information on the segment including the position given, in seconds.
    ///
    /// Returns `None` if no such media segment is found.
    fn segment_from_pos(&self, pos: f64) -> Option<&MediaSegmentInfo> {
        self.segment_list
            .media()
            .iter()
            .find(|s| s.end() > pos && s.start() <= pos)
    }
}
