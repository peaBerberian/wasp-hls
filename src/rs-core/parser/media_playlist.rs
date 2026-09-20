use super::{
    attribute_list::{parse_enumerated_string, AttributeListIter},
    multi_variant_playlist::MediaPlaylistContext,
    playlist_lines::read_playlist_line,
    segment_list::{InitSegmentInfo, MediaSegmentInfo, SegmentList, SegmentTimeInfo},
    timeline_sync::TimelineReference,
    top_level_playlist::ExternalMediaInfo,
    top_level_playlist::{
        is_multivariant_playlist_tag_name, media_playlist_singleton_tag_name, tag_name,
    },
    value_parsers::{
        parse_byte_range, parse_decimal_floating_point, parse_decimal_integer,
        parse_start_attribute, ByteRange,
    },
    value_parsers::{parse_iso_8601_date, StartAttribute},
    variable_substitution::VariableDefinition,
    variable_substitution::{
        parse_define_tag, parse_substituted_quoted_string, VariableDefinitionError, VariableStore,
    },
};
use crate::{
    bindings::{MediaType, PlaylistNature},
    utils::{logger::*, url::Url},
};
use std::collections::HashSet;
use std::{error, fmt, io::BufRead};

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
pub(crate) enum MediaPlaylistParsingError {
    UnparsableExtInf,
    UnparsableByteRange,
    UriMissingInMap,
    MissingTargetDuration,
    UriWithoutExtInf,
    DuplicateTag,
    ConflictingPlaylistTagTypes,
    UnableToReadLine,
    VariableDefinition(VariableDefinitionError),
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
            MediaPlaylistParsingError::VariableDefinition(err) => {
                write!(
                    f,
                    "Invalid EXT-X-DEFINE usage in the Media Playlist: {:?}",
                    err
                )
            }
            MediaPlaylistParsingError::DuplicateTag => {
                write!(f, "A singleton tag was duplicated in the Media Playlist")
            }
            MediaPlaylistParsingError::ConflictingPlaylistTagTypes => {
                write!(
                    f,
                    "The Media Playlist contains Multivariant Playlist tags and must fail to parse"
                )
            }
            MediaPlaylistParsingError::UnableToReadLine => {
                write!(f, "Unable to read a line in the Media Playlist")
            }
        }
    }
}

impl error::Error for MediaPlaylistParsingError {}

impl From<VariableDefinitionError> for MediaPlaylistParsingError {
    fn from(err: VariableDefinitionError) -> MediaPlaylistParsingError {
        MediaPlaylistParsingError::VariableDefinition(err)
    }
}

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
    playlist_type: PlaylistNature,
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
    /// Metadata inferred from one of this playlist's segments when HLS signaling is incomplete.
    external_media_info: Option<ExternalMediaInfo>,
    /// Whether at least one EXT-X-PROGRAM-DATE-TIME tag was found in the playlist.
    pub(super) has_program_date_time: bool,
    /// Whether this playlist's effective timeline is based on
    /// `EXT-X-PROGRAM-DATE-TIME`.
    pub(super) uses_program_date_time: bool,
    // TODO
    // pub server_control: ServerControl,
    // pub part_inf: Option<f64>,
}

impl MediaPlaylist {
    /// Create a new `MediaPlaylist` object, by giving it a `BufRead` reading into its
    /// corresponding Media Playlist file from its very beginning.
    pub(super) fn create(
        mut playlist: impl BufRead,
        url: Url,
        prev_playlist: Option<&MediaPlaylist>,
        timeline_reference: Option<&TimelineReference>,
        context: &MediaPlaylistContext,
    ) -> Result<Self, MediaPlaylistParsingError> {
        let mut version: Option<u32> = None;
        let mut independent_segments = false;
        let mut target_duration: Option<u32> = None;
        let mut media_sequence = 0;
        let mut end_list = false;
        let mut playlist_type = PlaylistNature::Unknown;
        let mut i_frames_only = false;
        let mut discontinuity_sequence = 0;
        let mut last_incomplete_map = None;
        let mut maps_info: Vec<InitSegmentInfo> = vec![];
        let mut start = None;
        let mut skip_next_segment = false;
        let mut pending_discontinuities = 0u32;
        let mut saw_program_date_time = false;

        let playlist_base_url = url.pathname();

        let mut curr_start_time = 0.;
        let mut curr_program_date_time = None;
        let mut media_segments: Vec<MediaSegmentInfo> = vec![];
        let mut next_segment_duration: Option<f64> = None;
        let mut current_byte: Option<usize> = None;
        let mut next_segment_byte_range: Option<ByteRange> = None;
        let mut variable_store = VariableStore::from_url(&url);
        let mut seen_singleton_tags = HashSet::new();

        let mut line_bytes = Vec::new();
        while let Some(str_line) = read_playlist_line(&mut playlist, &mut line_bytes)
            .map_err(|_| MediaPlaylistParsingError::UnableToReadLine)?
        {
            if str_line.is_empty() {
                continue;
            } else if let Some(stripped) = str_line.strip_prefix("#EXT") {
                if let Some(tag_name) = tag_name(&str_line) {
                    if is_multivariant_playlist_tag_name(tag_name) {
                        return Err(MediaPlaylistParsingError::ConflictingPlaylistTagTypes);
                    }

                    if let Some(singleton_tag_name) = media_playlist_singleton_tag_name(tag_name) {
                        if !seen_singleton_tags.insert(singleton_tag_name) {
                            return Err(MediaPlaylistParsingError::DuplicateTag);
                        }
                    }
                }

                let colon_idx = match stripped.find(':') {
                    None => str_line.len(),
                    Some(idx) => idx + 4,
                };

                match &str_line[4..colon_idx] {
                    "-X-DEFINE" => match parse_define_tag(&str_line) {
                        Ok(VariableDefinition::Name { name, value }) => {
                            variable_store.define(name, value)?
                        }
                        Ok(VariableDefinition::Import { name }) => {
                            variable_store.import(&name, context.multivariant_variables())?
                        }
                        Ok(VariableDefinition::QueryParam { name }) => {
                            variable_store.define_query_param(&name)?
                        }
                        Err(err) => return Err(err.into()),
                    },
                    "-X-VERSION" => match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                        Ok(v) if v <= (u32::MAX as u64) => version = Some(v as u32),
                        _ => log_warn!("Unparsable VERSION value"),
                    },
                    "-X-TARGETDURATION" => {
                        match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                            Ok(t) if t <= (u32::MAX as u64) => target_duration = Some(t as u32),
                            _ => log_warn!("Unparsable TARGETDURATION value"),
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
                            log_warn!("Parser: Failed to parse `EXT-X-START` attribute");
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
                            _ => log_warn!("Unparsable MEDIA-SEQUENCE value"),
                        }
                    }
                    "-X-DISCONTINUITY-SEQUENCE" => {
                        match parse_decimal_integer(&str_line, colon_idx + 1).0 {
                            Ok(s) if s <= (u32::MAX as u64) => discontinuity_sequence = s as u32,
                            _ => log_warn!("Unparsable DISCONTINUITY-SEQUENCE value"),
                        }
                    }
                    "-X-DISCONTINUITY" => {
                        pending_discontinuities = pending_discontinuities.wrapping_add(1);
                    }
                    "-X-PLAYLIST-TYPE" => match parse_enumerated_string(&str_line, colon_idx + 1).0
                    {
                        "EVENT" => playlist_type = PlaylistNature::Event,
                        "VOD" => playlist_type = PlaylistNature::VoD,
                        x => {
                            log_warn!("Unrecognized playlist type: {}", x);
                        }
                    },
                    "-X-PROGRAM-DATE-TIME" => {
                        if let Some(date) = parse_iso_8601_date(&str_line, colon_idx + 1) {
                            saw_program_date_time = true;
                            curr_start_time = date;
                            curr_program_date_time = Some(date);
                        }
                    }
                    "-X-I-FRAMES-ONLY" => i_frames_only = true,
                    "-X-MAP" => {
                        let mut map_info_url: Option<Url> = None;
                        let mut map_info_byte_range: Option<ByteRange> = None;
                        for item in AttributeListIter::new(&str_line, colon_idx + 1) {
                            match item.name {
                                "URI" => {
                                    let val = parse_substituted_quoted_string(
                                        &str_line,
                                        item.value_start_offset,
                                        &variable_store,
                                    )?;
                                    let init_url = Url::new(val.into_owned());
                                    let init_url = if init_url.is_absolute() {
                                        init_url
                                    } else {
                                        Url::from_relative(playlist_base_url, init_url)
                                    };
                                    map_info_url = Some(init_url);
                                }
                                "BYTERANGE" => {
                                    let val = parse_substituted_quoted_string(
                                        &str_line,
                                        item.value_start_offset,
                                        &variable_store,
                                    )?;
                                    match parse_byte_range(&val, 0, None) {
                                        Some(br) => {
                                            current_byte = Some(br.last_byte + 1);
                                            map_info_byte_range = Some(br);
                                        }
                                        _ => {
                                            return Err(
                                                MediaPlaylistParsingError::UnparsableByteRange,
                                            );
                                        }
                                    };
                                }
                                _ => {}
                            }
                        }
                        if let Some(url) = map_info_url {
                            last_incomplete_map = Some((url, map_info_byte_range));
                        } else {
                            return Err(MediaPlaylistParsingError::UriMissingInMap);
                        }
                    }
                    "M3U" => {}
                    x => log_debug!("Unrecognized tag: \"{}\"", x),
                }
            } else if str_line.starts_with('#') {
                continue;
            } else if skip_next_segment {
                skip_next_segment = false;
                if let Some(duration) = next_segment_duration {
                    discontinuity_sequence =
                        discontinuity_sequence.wrapping_add(pending_discontinuities);
                    pending_discontinuities = 0;
                    curr_start_time += duration;
                    if let Some(program_date_time) = curr_program_date_time {
                        curr_program_date_time = Some(program_date_time + duration);
                    }
                    next_segment_duration = None;
                    next_segment_byte_range = None;
                } else {
                    return Err(MediaPlaylistParsingError::UriWithoutExtInf);
                }
            } else {
                // URI
                let seg_line = variable_store.substitute(&str_line)?;
                let seg_url = Url::new(seg_line.into_owned());
                let seg_url = if seg_url.is_absolute() {
                    seg_url
                } else {
                    Url::from_relative(playlist_base_url, seg_url)
                };
                if let Some(duration) = next_segment_duration {
                    discontinuity_sequence =
                        discontinuity_sequence.wrapping_add(pending_discontinuities);
                    pending_discontinuities = 0;
                    let seg = MediaSegmentInfo {
                        sequence: 0,
                        discontinuity_sequence,
                        time_info: SegmentTimeInfo::new(curr_start_time, duration),
                        program_date_time: curr_program_date_time,
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
                    media_segments.push(MediaSegmentInfo {
                        sequence: media_sequence + media_segments.len() as u32,
                        ..seg
                    });
                    curr_start_time += duration;
                    if let Some(program_date_time) = curr_program_date_time {
                        curr_program_date_time = Some(program_date_time + duration);
                    }
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

        let mut uses_program_date_time = saw_program_date_time;
        if !saw_program_date_time {
            if let Some((offset, inherited_program_date_time)) = infer_live_timeline_offset(
                prev_playlist,
                timeline_reference,
                media_segments.as_slice(),
            ) {
                uses_program_date_time = inherited_program_date_time;
                for seg in &mut media_segments {
                    seg.time_info.start += offset;
                }

                for init in &mut maps_info {
                    let is_inherited_from_previous = prev_playlist.is_some_and(|prev| {
                        prev.segment_list.init.iter().any(|known| {
                            known.url == init.url && known.byte_range == init.byte_range
                        })
                    });
                    if !is_inherited_from_previous {
                        init.start += offset;
                    }
                }
            }
        } else {
            backfill_program_date_time(&mut media_segments);
        }

        if playlist_type == PlaylistNature::Unknown {
            playlist_type = if end_list {
                PlaylistNature::VoD
            } else {
                PlaylistNature::Live
            };
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
            segment_list: SegmentList {
                init: maps_info,
                media: media_segments,
            },
            url,
            external_media_info: prev_playlist.and_then(|p| p.external_media_info.clone()),
            has_program_date_time: saw_program_date_time,
            uses_program_date_time,
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
                    self.segment_list
                        .segment_from_pos(actual_time)
                        .map(|s| s.start())
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
                    .iter()
                    .rev()
                    .find(|s| s.duration() > 0.)
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

    pub(crate) fn media_sequence(&self) -> u32 {
        self.media_sequence
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
        !self.end_list && self.playlist_type != PlaylistNature::VoD
    }

    /// Returns `true` if the `MediaPlaylist` is linked to a "live content" which is an unfinished
    /// content that may need to be played close to its maximum position.
    pub(crate) fn is_live(&self) -> bool {
        self.playlist_type == PlaylistNature::Live
    }

    pub(crate) fn playlist_type(&self) -> PlaylistNature {
        self.playlist_type
    }

    /// Returns `true` if the last segment referenced in this `MediaPlaylist` can be assumed to be
    /// the last chronological one.
    pub(crate) fn is_ended(&self) -> bool {
        self.end_list
    }

    pub(crate) fn first_sequence(&self) -> Option<u32> {
        self.segment_list.media().first().map(|s| s.sequence())
    }

    pub(crate) fn last_sequence(&self) -> Option<u32> {
        self.segment_list.media().last().map(|s| s.sequence())
    }

    pub(crate) fn contains_sequence(&self, sequence: u32) -> bool {
        match (self.first_sequence(), self.last_sequence()) {
            (Some(first), Some(last)) => first <= sequence && sequence <= last,
            _ => false,
        }
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

    pub(crate) fn external_media_info(&self) -> Option<&ExternalMediaInfo> {
        self.external_media_info.as_ref()
    }

    pub(crate) fn uses_program_date_time(&self) -> bool {
        self.uses_program_date_time
    }

    pub(crate) fn set_external_media_info(&mut self, media_info: ExternalMediaInfo) {
        self.external_media_info = Some(media_info);
    }

    /// Returns the "extension" part of the media segments referenced in this Media Playlist (e.g.
    /// `"mp4"` for `.mp4` files).
    ///
    /// Returns `None` if unknown.
    fn extension(&self) -> Option<&str> {
        self.segment_list.media().first().map(|s| s.url.extension())
    }
}

fn urls_match_for_reload(prev_url: &Url, new_url: &Url) -> bool {
    // Query parameters often rotate between reloads without changing the
    // underlying segment identity, so ignore them for overlap matching.
    strip_query(prev_url.get_ref()) == strip_query(new_url.get_ref())
}

fn strip_query(url: &str) -> &str {
    match url.find('?') {
        Some(idx) => &url[..idx],
        None => url,
    }
}

fn backfill_program_date_time(media_segments: &mut [MediaSegmentInfo]) {
    let Some(first_pdt_index) = media_segments
        .iter()
        .position(|seg| seg.program_date_time.is_some())
    else {
        return;
    };

    let mut next_program_date_time = media_segments[first_pdt_index].program_date_time.unwrap();
    for seg in media_segments[..first_pdt_index].iter_mut().rev() {
        // RFC 8216 says clients SHOULD extrapolate backward from the first PDT tag using segment
        // durations when earlier segments have no explicit EXT-X-PROGRAM-DATE-TIME. hls.js does
        // the same, including across discontinuities, so keep the mapping monotonic here.
        next_program_date_time -= seg.duration();
        seg.program_date_time = Some(next_program_date_time);
        seg.time_info.start = next_program_date_time;
    }
}

fn infer_live_timeline_offset(
    prev_playlist: Option<&MediaPlaylist>,
    timeline_reference: Option<&TimelineReference>,
    media_segments: &[MediaSegmentInfo],
) -> Option<(f64, bool)> {
    if let Some(prev_playlist) = prev_playlist {
        for new_seg in media_segments {
            if let Some(prev_seg) = prev_playlist.segment_list.media.iter().find(|prev_seg| {
                prev_seg.sequence == new_seg.sequence
                    && prev_seg.discontinuity_sequence == new_seg.discontinuity_sequence
                    && urls_match_for_reload(&prev_seg.url, &new_seg.url)
            }) {
                return Some((
                    prev_seg.start() - new_seg.start(),
                    prev_playlist.uses_program_date_time,
                ));
            }
        }

        match (
            prev_playlist.segment_list.media.last(),
            media_segments.first(),
        ) {
            (Some(prev_last), Some(new_first))
                if prev_last.sequence.wrapping_add(1) == new_first.sequence
                    && prev_last.discontinuity_sequence == new_first.discontinuity_sequence =>
            {
                return Some((
                    prev_last.end() - new_first.start(),
                    prev_playlist.uses_program_date_time,
                ));
            }
            _ => {}
        }
    }

    let reference_playlist = timeline_reference?;
    reference_playlist
        .infer_offset_for(media_segments)
        .map(|offset| (offset, reference_playlist.uses_program_date_time()))
}

#[cfg(test)]
mod tests {
    use super::{MediaPlaylist, MediaPlaylistParsingError, TimelineReference};
    use crate::{
        bindings::PlaylistNature, parser::multi_variant_playlist::MediaPlaylistContext,
        utils::url::Url,
    };
    use std::io::Cursor;

    fn parse_media_playlist(playlist: &str) -> Result<MediaPlaylist, MediaPlaylistParsingError> {
        MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
    }

    #[test]
    fn rejects_duplicate_singleton_tags() {
        let err = parse_media_playlist(
            r#"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg.ts
"#,
        );

        assert!(matches!(err, Err(MediaPlaylistParsingError::DuplicateTag)));
    }

    #[test]
    fn rejects_multivariant_tags_in_media_playlists() {
        let err = parse_media_playlist(
            r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-STREAM-INF:BANDWIDTH=1000
#EXTINF:4,
seg.ts
"#,
        );

        assert!(matches!(
            err,
            Err(MediaPlaylistParsingError::ConflictingPlaylistTagTypes)
        ));
    }

    #[test]
    fn keeps_segments_after_invalid_utf8_in_a_comment() {
        let playlist =
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n# malformed \xff comment\n#EXTINF:4,\nseg.ts\n";
        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = parsed.segment_list().media();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].url().get_ref(), "https://example.com/seg.ts");
    }

    #[test]
    fn keeps_invalid_utf8_in_its_segment_uri() {
        let playlist =
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg-\xff.ts\n#EXTINF:4,\nnext.ts\n";
        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = parsed.segment_list().media();
        assert_eq!(segments.len(), 2);
        assert!(segments[0].url().get_ref().contains('�'));
        assert_eq!(segments[1].url().get_ref(), "https://example.com/next.ts");
    }

    #[test]
    fn infers_vod_from_endlist_without_explicit_playlist_type() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg-0.ts
#EXT-X-ENDLIST
"#;

        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        assert_eq!(parsed.playlist_type(), PlaylistNature::VoD);
        assert!(!parsed.may_be_refreshed());
        assert!(!parsed.is_live());
    }

    #[test]
    fn defaults_discontinuity_sequence_to_zero_without_tags() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXTINF:4,
seg-0.ts
#EXTINF:4,
seg-1.ts
"#;

        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = parsed.segment_list().media();
        assert_eq!(segments[0].discontinuity_sequence(), 0);
        assert_eq!(segments[1].discontinuity_sequence(), 0);
    }

    #[test]
    fn parses_discontinuity_sequence_and_discontinuity_tags() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-DISCONTINUITY-SEQUENCE:7
#EXTINF:4,
seg-0.ts
#EXT-X-DISCONTINUITY
#EXTINF:4,
seg-1.ts
#EXTINF:4,
seg-2.ts
#EXT-X-DISCONTINUITY
#EXTINF:4,
seg-3.ts
"#;

        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = parsed.segment_list().media();
        assert_eq!(segments[0].discontinuity_sequence(), 7);
        assert_eq!(segments[1].discontinuity_sequence(), 8);
        assert_eq!(segments[2].discontinuity_sequence(), 8);
        assert_eq!(segments[3].discontinuity_sequence(), 9);
    }

    #[test]
    fn discontinuity_before_gap_still_advances_following_segment_sequence() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-DISCONTINUITY-SEQUENCE:5
#EXT-X-DISCONTINUITY
#EXT-X-GAP
#EXTINF:4,
seg-gap.ts
#EXT-X-DISCONTINUITY
#EXTINF:4,
seg-next.ts
"#;

        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = parsed.segment_list().media();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].discontinuity_sequence(), 7);
    }

    #[test]
    fn refreshed_live_playlist_keeps_timeline_from_previous_sequence_overlap() {
        let previous_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:10
#EXTINF:4,
seg-10.ts
#EXTINF:4,
seg-11.ts
#EXTINF:4,
seg-12.ts
"#;
        let refreshed_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:11
#EXTINF:4,
seg-11.ts
#EXTINF:4,
seg-12.ts
#EXTINF:4,
seg-13.ts
"#;

        let previous = MediaPlaylist::create(
            Cursor::new(previous_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();
        let refreshed = MediaPlaylist::create(
            Cursor::new(refreshed_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            Some(&previous),
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = refreshed.segment_list().media();
        assert_eq!(segments[0].sequence(), 11);
        assert_eq!(segments[0].start(), 4.);
        assert_eq!(segments[1].start(), 8.);
        assert_eq!(segments[2].start(), 12.);
    }

    #[test]
    fn refreshed_live_playlist_does_not_reuse_timeline_for_mismatched_overlap_segment() {
        let previous_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:10
#EXTINF:4,
seg-10.ts
#EXTINF:4,
seg-11.ts
"#;
        let refreshed_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:11
#EXTINF:4,
replacement-11.ts
#EXTINF:4,
seg-12.ts
"#;

        let previous = MediaPlaylist::create(
            Cursor::new(previous_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();
        let refreshed = MediaPlaylist::create(
            Cursor::new(refreshed_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            Some(&previous),
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = refreshed.segment_list().media();
        assert_eq!(segments[0].start(), 0.);
        assert_eq!(segments[1].start(), 4.);
    }

    #[test]
    fn refreshed_live_playlist_accepts_query_only_uri_change() {
        let previous_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:10
#EXTINF:4,
seg-10.ts?token=old
#EXTINF:4,
seg-11.ts?token=old
"#;
        let refreshed_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:11
#EXTINF:4,
seg-11.ts?token=new
#EXTINF:4,
seg-12.ts?token=new
"#;

        let previous = MediaPlaylist::create(
            Cursor::new(previous_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();
        let refreshed = MediaPlaylist::create(
            Cursor::new(refreshed_playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            Some(&previous),
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = refreshed.segment_list().media();
        assert_eq!(segments[0].start(), 4.);
        assert_eq!(segments[1].start(), 8.);
    }

    #[test]
    fn first_load_of_alternate_live_playlist_can_align_from_current_reference_playlist() {
        let current_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:110
#EXTINF:4,
video-110.ts
#EXTINF:4,
video-111.ts
#EXTINF:4,
video-112.ts
"#;
        let newly_selected_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:110
#EXTINF:4,
audio-110.ts
#EXTINF:4,
audio-111.ts
#EXTINF:4,
audio-112.ts
"#;

        let current = MediaPlaylist::create(
            Cursor::new(current_playlist),
            Url::new("https://example.com/video.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();
        let timeline_reference = TimelineReference::from_segment_list(
            current.segment_list().media(),
            current.uses_program_date_time(),
        );
        let selected = MediaPlaylist::create(
            Cursor::new(newly_selected_playlist),
            Url::new("https://example.com/audio.m3u8".to_owned()),
            None,
            Some(&timeline_reference),
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = selected.segment_list().media();
        assert_eq!(
            segments[0].start(),
            current.segment_list().media()[0].start()
        );
        assert_eq!(
            segments[1].start(),
            current.segment_list().media()[1].start()
        );
        assert_eq!(segments[2].end(), current.segment_list().media()[2].end());
    }

    #[test]
    fn first_load_of_unrelated_playlist_does_not_align_from_reference_start_only() {
        let current_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:110
#EXTINF:4,
video-110.ts
#EXTINF:4,
video-111.ts
#EXTINF:4,
video-112.ts
"#;
        let unrelated_playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:4
#EXT-X-MEDIA-SEQUENCE:510
#EXTINF:4,
audio-510.ts
#EXTINF:4,
audio-511.ts
#EXTINF:4,
audio-512.ts
"#;

        let current = MediaPlaylist::create(
            Cursor::new(current_playlist),
            Url::new("https://example.com/video.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();
        let timeline_reference = TimelineReference::from_segment_list(
            current.segment_list().media(),
            current.uses_program_date_time(),
        );
        let selected = MediaPlaylist::create(
            Cursor::new(unrelated_playlist),
            Url::new("https://example.com/audio.m3u8".to_owned()),
            None,
            Some(&timeline_reference),
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let segments = selected.segment_list().media();
        assert_eq!(segments[0].start(), 0.);
        assert_eq!(segments[2].end(), 12.);
    }

    fn refresh_interval_ignores_trailing_zero_duration_segment() {
        let playlist = r#"#EXTM3U
#EXT-X-TARGETDURATION:3
#EXT-X-MEDIA-SEQUENCE:90
#EXTINF:3.008,
seg-90.m4s
#EXTINF:2.98667,
seg-91.m4s
#EXTINF:0,
seg-92.m4s
"#;

        let parsed = MediaPlaylist::create(
            Cursor::new(playlist),
            Url::new("https://example.com/media.m3u8".to_owned()),
            None,
            None,
            &MediaPlaylistContext::default(),
        )
        .unwrap();

        let refresh_interval = parsed.refresh_interval().unwrap();
        assert!((refresh_interval - (2986.67 * 1.1)).abs() < 0.001);
    }
}
