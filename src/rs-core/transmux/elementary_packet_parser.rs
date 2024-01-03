// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

use super::transport_packet_parser::{ParsedTsPacket, PesPacketInfo, Pid, ProgramMapTable};

const H264_STREAM_TYPE: u8 = 0x1b;
const ADTS_STREAM_TYPE: u8 = 0x0f;
const METADATA_STREAM_TYPE: u8 = 0x15;

struct MetadataElementaryPacket {
    tracks: Vec<MetadataElementaryPacketTrack>,
}

pub(super) struct MediaElementaryPacket {
    media_type: ElementaryMediaType,
    track_id: Pid,
    data: Vec<u8>,
    packet_length: Option<u32>,
    data_alignment_indicator: Option<bool>,
    pts: Option<u32>,
    dts: Option<u32>,
}

impl MediaElementaryPacket {
    pub(super) fn media_type(&self) -> ElementaryMediaType {
        self.media_type
    }
    pub(super) fn pts(&self) -> Option<u32> {
        self.pts
    }
    pub(super) fn dts(&self) -> Option<u32> {
        self.dts
    }
    pub(super) fn data(&self) -> &[u8] {
        &self.data
    }
    pub(super) fn track_id(&self) -> Pid {
        self.track_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ElementaryMediaType {
    Video,
    Audio,
    TimedMetadata,
}

struct MetadataElementaryPacketTrack {
    base_media_decode_time: u64,
    id: Pid,
    codec: &'static str,
    media_type: ElementaryMediaType,
}

enum ElementaryPacket {
    Metadata(MetadataElementaryPacket),
    Media(MediaElementaryPacket),
}

#[derive(Clone, Debug, Default)]
struct ElementaryTrackInfo {
    data: Vec<PesPacketInfo>,
    size: u32,
}

struct ElementaryPacketParser {
    segment_had_pmt: bool,
    audio: ElementaryTrackInfo,
    video: ElementaryTrackInfo,
    timed_metadata: ElementaryTrackInfo,
    program_map_table: Option<ProgramMapTable>,
}

impl ElementaryPacketParser {
    pub(super) fn new() -> Self {
        Self {
            segment_had_pmt: false,
            audio: ElementaryTrackInfo::default(),
            video: ElementaryTrackInfo::default(),
            timed_metadata: ElementaryTrackInfo::default(),
            program_map_table: None,
        }
    }

    /// Identifies M2TS packet types and parses PES packets using metadata
    /// parsed from the PMT
    pub(super) fn read_next_packet(&mut self, data: ParsedTsPacket) -> Option<ElementaryPacket> {
        match data {
            ParsedTsPacket::Pat(_) => {
                // we have to wait for the PMT to arrive as well before we
                // have any meaningful metadata
            }
            ParsedTsPacket::Pes(ref info) => {
                let stream_type = match info.stream_type() {
                    Some(H264_STREAM_TYPE) => ElementaryMediaType::Video,
                    Some(ADTS_STREAM_TYPE) => ElementaryMediaType::Audio,
                    Some(METADATA_STREAM_TYPE) => ElementaryMediaType::TimedMetadata,
                    _ => {
                        // ignore unknown stream types
                        return None;
                    }
                };

                // if a new packet is starting, we can flush the completed
                // packet
                let mut res: Option<ElementaryPacket> = None;
                if info.payload_unit_start_indicator() {
                    res = self.flush_stream(stream_type, true);
                }

                let mut stream = match stream_type {
                    ElementaryMediaType::Video => &mut self.video,
                    ElementaryMediaType::Audio => &mut self.audio,
                    ElementaryMediaType::TimedMetadata => &mut self.timed_metadata,
                };

                // buffer this fragment until we are sure we've received the
                // complete payload
                // TODO no need to clone here, we should take back ownership
                stream.data.push(info.clone());
                stream.size += info.data().len() as u32;
                return res;
            }
            ParsedTsPacket::Pmt(info) => {
                let mut packet = MetadataElementaryPacket { tracks: vec![] };

                let program_map_table = info.program_map_table();
                self.program_map_table = Some(program_map_table.clone());

                // translate audio and video streams to tracks
                if let Some(pid) = program_map_table.video() {
                    packet.tracks.push({
                        MetadataElementaryPacketTrack {
                            base_media_decode_time: 0,
                            id: pid,
                            codec: "avc",
                            media_type: ElementaryMediaType::Video,
                        }
                    });
                }
                if let Some(pid) = program_map_table.audio() {
                    packet.tracks.push({
                        MetadataElementaryPacketTrack {
                            base_media_decode_time: 0,
                            id: pid,
                            codec: "atds",
                            media_type: ElementaryMediaType::Audio,
                        }
                    });
                }

                self.segment_had_pmt = true;

                return Some(ElementaryPacket::Metadata(packet));
            }
        }
        None
    }

    pub(super) fn reset(&mut self) {
        self.video.size = 0;
        self.video.data = vec![];
        self.audio.size = 0;
        self.audio.data = vec![];
    }

    fn parse_pes(&mut self, payload: Vec<u8>, pes: &mut MediaElementaryPacket) {
        if payload.len() < 9 {
            return;
        }
        let start_prefix =
            ((payload[0] as u32) << 16) | ((payload[1] as u32) << 8) | payload[2] as u32;
        // In certain live streams, the start of a TS fragment has ts packets
        // that are frame data that is continuing from the previous fragment. This
        // is to check that the pes data is the start of a new pes payload
        if start_prefix != 1 {
            return;
        }
        // get the packet length, this will be 0 for video
        pes.packet_length = Some(6 + (((payload[4] as u32) << 8) | payload[5] as u32));

        // find out if this packets starts a new keyframe
        pes.data_alignment_indicator = Some((payload[6] & 0x04) != 0);

        // PES packets may be annotated with a PTS value, or a PTS value
        // and a DTS value. Determine what combination of values is
        // available to work with.
        let pts_dts_flags = payload[7];

        // PTS and DTS are normally stored as a 33-bit number.  Javascript
        // performs all bitwise operations on 32-bit integers but javascript
        // supports a much greater range (52-bits) of integer using standard
        // mathematical operations.
        // We construct a 31-bit value using bitwise operators over the 31
        // most significant bits and then multiply by 4 (equal to a left-shift
        // of 2) before we add the final 2 least significant bits of the
        // timestamp (equal to an OR.)
        if pts_dts_flags & 0xc0 != 0 && payload.len() >= 13 {
            // the PTS and DTS are not written out directly. For information
            // on how they are encoded, see
            // http://dvd.sourceforge.net/dvdinfo/pes-hdr.html
            let mut pts = (((payload[9] & 0x0e) as u32) << 27)
                | ((payload[10] as u32) << 20)
                | (((payload[11] & 0xfe) as u32) << 12)
                | ((payload[12] as u32) << 5)
                | (((payload[13] & 0xfe) as u32) >> 3);
            pts *= 4; // Left shift by 2
            pts += ((payload[13] & 0x06) as u32) >> 1; // OR by the two LSBs
            pes.pts = Some(pts);
            pes.dts = pes.pts;
            if pts_dts_flags & 0x40 != 0 && payload.len() >= 18 {
                let mut dts = (((payload[14] & 0x0e) as u32) << 27)
                    | ((payload[15] as u32) << 20)
                    | (((payload[16] & 0xfe) as u32) << 12)
                    | ((payload[17] as u32) << 5)
                    | (((payload[18] & 0xfe) as u32) >> 3);
                dts *= 4; // Left shift by 2
                dts += ((payload[18] & 0x06) as u32) >> 1; // OR by the two LSBs
                pes.dts = Some(dts)
            }
        }

        // the data section starts immediately after the PES header.
        // pes_header_data_length specifies the number of header bytes
        // that follow the last byte of the field.
        pes.data = payload[9 + (payload[8] as usize)..].to_owned();
    }

    /// Pass completely parsed PES packets.
    fn flush_stream(
        &mut self,
        media_type: ElementaryMediaType,
        force_flush: bool,
    ) -> Option<ElementaryPacket> {
        let mut packet_flushable = false;

        let stream = self.track_info_mut(media_type);

        // do nothing if there is not enough buffered data for a complete
        // PES header
        if stream.data.is_empty() || stream.size < 9 {
            return None;
        }
        let mut packet = MediaElementaryPacket {
            media_type,
            data: vec![],
            track_id: stream.data[0].pid(),
            packet_length: None,
            data_alignment_indicator: None,
            pts: None,
            dts: None,
        };

        // reassemble the packet
        let mut packet_data = Vec::with_capacity(stream.size as usize);
        stream.data.iter().for_each(|fragment| {
            packet_data.extend(fragment.data());
        });

        // parse assembled packet's PES header
        self.parse_pes(packet_data, &mut packet);

        let stream = self.track_info_mut(media_type);

        // non-video PES packets MUST have a non-zero PES_packet_length
        // check that there is enough stream data to fill the packet
        if media_type == ElementaryMediaType::Video {
            packet_flushable = packet
                .packet_length
                .map(|l| l <= stream.size)
                .unwrap_or(false);
        }

        // flush pending packets if the conditions are right
        if force_flush || packet_flushable {
            stream.size = 0;
            stream.data = vec![];
        }

        // only emit packets that are complete. this is to avoid assembling
        // incomplete PES packets due to poor segmentation
        if packet_flushable {
            Some(ElementaryPacket::Media(packet))
        } else {
            None
        }
    }

    /**
     * Flush any remaining input. Video PES packets may be of variable
     * length. Normally, the start of a new video packet can trigger the
     * finalization of the previous packet. That is not possible if no
     * more video is forthcoming, however. In that case, some other
     * mechanism (like the end of the file) has to be employed. When it is
     * clear that no additional data is forthcoming, calling this method
     * will flush the buffered packets.
     */
    pub fn flush_streams(&mut self) -> Vec<ElementaryPacket> {
        let mut res = vec![];
        if let Some(vid) = self.flush_stream(ElementaryMediaType::Video, false) {
            res.push(vid);
        }
        if let Some(aud) = self.flush_stream(ElementaryMediaType::Audio, false) {
            res.push(aud);
        }
        if let Some(tm) = self.flush_stream(ElementaryMediaType::TimedMetadata, false) {
            res.push(tm);
        }
        res
    }

    pub fn flush(&mut self) -> Vec<ElementaryPacket> {
        let mut pmt_packet: Option<MetadataElementaryPacket> = None;
        // if on flush we haven't had a pmt emitted and we have a
        // pmt to emit. emit the pmt so that we trigger a trackinfo downstream.
        if !self.segment_had_pmt {
            if let Some(ref program_map_table) = self.program_map_table {
                let mut pmt = MetadataElementaryPacket { tracks: vec![] };
                // translate audio and video streams to tracks
                if let Some(vid) = program_map_table.video() {
                    pmt.tracks.push(MetadataElementaryPacketTrack {
                        base_media_decode_time: 0,
                        id: vid,
                        codec: "avc",
                        media_type: ElementaryMediaType::Video,
                    });
                }

                if let Some(aud) = program_map_table.audio() {
                    pmt.tracks.push(MetadataElementaryPacketTrack {
                        base_media_decode_time: 0,
                        id: aud,
                        codec: "adts",
                        media_type: ElementaryMediaType::Audio,
                    });
                }
                pmt_packet = Some(pmt);
            }
        }

        self.segment_had_pmt = false;
        if let Some(pmt) = pmt_packet {
            let mut res = vec![ElementaryPacket::Metadata(pmt)];
            res.extend(self.flush_streams());
            res
        } else {
            self.flush_streams()
        }
    }

    fn track_info(&mut self, media_type: ElementaryMediaType) -> &ElementaryTrackInfo {
        match media_type {
            ElementaryMediaType::Video => &self.video,
            ElementaryMediaType::Audio => &self.audio,
            ElementaryMediaType::TimedMetadata => &self.timed_metadata,
        }
    }

    fn track_info_mut(&mut self, media_type: ElementaryMediaType) -> &mut ElementaryTrackInfo {
        match media_type {
            ElementaryMediaType::Video => &mut self.video,
            ElementaryMediaType::Audio => &mut self.audio,
            ElementaryMediaType::TimedMetadata => &mut self.timed_metadata,
        }
    }
}
