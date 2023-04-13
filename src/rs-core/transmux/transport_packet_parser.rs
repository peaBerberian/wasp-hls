// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

const H264_STREAM_TYPE: u8 = 0x1b;
const ADTS_STREAM_TYPE: u8 = 0x0f;
const METADATA_STREAM_TYPE: u8 = 0x15;

// PIDs are 13 bits
pub(super) type Pid = u16;

#[derive(Clone, Debug)]
pub(super) struct PatPacketInfo {
    section_number: u8,
    last_section_number: u8,
    pmt_pid: Pid,
    pid: Pid,
    payload_unit_start_indicator: bool,
}

#[derive(Clone, Debug)]
pub(super) struct PmtPacketInfo {
    program_map_table: ProgramMapTable,
    pid: Pid,
    payload_unit_start_indicator: bool,
}

impl PmtPacketInfo {
    pub(super) fn pid(&self) -> Pid {
        self.pid
    }
    pub(super) fn program_map_table(&self) -> &ProgramMapTable {
        &self.program_map_table
    }
    pub(super) fn payload_unit_start_indicator(&self) -> bool {
        self.payload_unit_start_indicator
    }
}

#[derive(Clone, Debug)]
pub(super) struct PesPacketInfo {
    stream_type: Option<u8>,
    data: Vec<u8>,
    pid: Pid,
    payload_unit_start_indicator: bool,
}

impl PesPacketInfo {
    pub(super) fn pid(&self) -> Pid {
        self.pid
    }
    pub(super) fn data(&self) -> &[u8] {
        &self.data
    }
    pub(super) fn stream_type(&self) -> Option<u8> {
        self.stream_type
    }
    pub(super) fn payload_unit_start_indicator(&self) -> bool {
        self.payload_unit_start_indicator
    }
}

#[derive(Clone, Debug)]
pub(super) struct ProgramMapTable {
    /// PID for audio streams
    audio: Option<Pid>,
    /// PID for video streams
    video: Option<Pid>,
    /// map pid to stream type for metadata streams
    timed_metadata: Vec<(Pid, u8)>,
}

impl ProgramMapTable {
    pub(super) fn audio(&self) -> Option<Pid> {
        self.audio
    }
    pub(super) fn video(&self) -> Option<Pid> {
        self.video
    }
}

#[derive(Clone, Debug)]
pub(super) enum ParsedTsPacket {
    Pat(PatPacketInfo),
    Pmt(PmtPacketInfo),
    Pes(PesPacketInfo),
}

/// Accepts an MP2T Transport packet and parses it into legible
/// information.
pub(super) struct TransportPacketParser {
    packets_waiting_for_pmt: Vec<(Vec<u8>, usize, Pid, bool)>,
    program_map_table: Option<ProgramMapTable>,
    pmt_pid: Option<Pid>,
}

impl TransportPacketParser {
    pub(super) fn new() -> Self {
        Self {
            packets_waiting_for_pmt: vec![],
            program_map_table: None,
            pmt_pid: None,
        }
    }

    /// Parse a new MP2T Transport packet.
    pub(super) fn parse(&mut self, packet: &[u8]) -> Vec<ParsedTsPacket> {
        if packet.len() < 4 {
            return vec![];
        }

        let mut offset: usize = 4;
        let payload_unit_start_indicator = packet[1] & 0x40 != 0;

        // pid is a 13-bit field starting at the last bit of packet[1]
        let mut pid = (packet[1] & 0x1f) as u16;
        pid <<= 8;
        pid |= packet[2] as u16;

        // if an adaption field is present, its length is specified by the
        // fifth byte of the TS packet header. The adaptation field is
        // used to add stuffing to PES packets that don't fill a complete
        // TS packet, and to specify some forms of timing and control data
        // that we do not currently use.
        if (packet[3] & 0x30) >> 4 > 0x01 {
            offset += packet[offset] as usize + 1;
        }

        if offset >= packet.len() {
            return vec![];
        }

        // parse the rest of the packet based on the type
        if pid == 0 {
            if let Some(packet) =
                self.parse_psi(&packet[offset..], pid, payload_unit_start_indicator)
            {
                vec![packet]
            } else {
                vec![]
            }
        } else if Some(pid) == self.pmt_pid {
            let result = if let Some(packet) =
                self.parse_psi(&packet[offset..], pid, payload_unit_start_indicator)
            {
                packet
            } else {
                return vec![];
            };
            let mut ret = vec![result];

            // if there are any packets waiting for a PMT to be found, process them now
            while !self.packets_waiting_for_pmt.is_empty() {
                let waiting = self.packets_waiting_for_pmt.remove(0);
                ret.push(self.process_pes(&waiting.0, waiting.1, waiting.2, waiting.3));
            }
            return ret;
        } else if self.program_map_table.is_none() {
            // When we have not seen a PMT yet, defer further processing of PES packets until one
            // has been parsed
            self.packets_waiting_for_pmt.push((
                packet.to_owned(),
                offset,
                pid,
                payload_unit_start_indicator,
            ));
            vec![]
        } else {
            vec![self.process_pes(packet, offset, pid, payload_unit_start_indicator)]
        }
    }

    pub fn reset(&mut self) {
        self.packets_waiting_for_pmt.clear();
        self.program_map_table = None;
    }

    fn parse_psi(
        &mut self,
        payload: &[u8],
        pid: Pid,
        payload_unit_start_indicator: bool,
    ) -> Option<ParsedTsPacket> {
        if payload.is_empty() {
            return None;
        }
        let mut offset = 0;

        // PSI packets may be split into multiple sections and those
        // sections may be split into multiple packets. If a PSI
        // section starts in this packet, the payload_unit_start_indicator
        // will be true and the first byte of the payload will indicate
        // the offset from the current position to the start of the
        // section.
        if payload_unit_start_indicator {
            offset += (payload[0] as usize) + 1;
        }

        if offset >= payload.len() {
            None
        } else if pid == 0 {
            self.parse_pat(&payload[offset..], payload_unit_start_indicator)
        } else {
            self.parse_pmt(&payload[offset..], pid, payload_unit_start_indicator)
        }
    }

    fn parse_pat(
        &mut self,
        payload: &[u8],
        payload_unit_start_indicator: bool,
    ) -> Option<ParsedTsPacket> {
        if payload.len() < 12 {
            return None;
        }
        let pat_info = PatPacketInfo {
            pid: 0,
            payload_unit_start_indicator,
            section_number: payload[7],
            last_section_number: payload[8],

            // skip the PSI header and parse the first PMT entry
            pmt_pid: (((payload[10] & 0x1f) as u16) << 8) | payload[11] as u16,
        };
        self.pmt_pid = Some(pat_info.pmt_pid);
        Some(ParsedTsPacket::Pat(pat_info))
    }

    /// Parses out the relevant fields of a Program Map Table (PMT).
    ///
    /// - payload - The PMT-specific portion of an MP2T
    /// packet. The first byte in this array should be the table_id
    /// field.
    fn parse_pmt(
        &mut self,
        payload: &[u8],
        pid: Pid,
        payload_unit_start_indicator: bool,
    ) -> Option<ParsedTsPacket> {
        // PMTs can be sent ahead of the time when they should actually
        // take effect. We don't believe this should ever be the case
        // for HLS but we'll ignore "forward" PMT declarations if we see
        // them. Future PMT declarations have the current_next_indicator
        // set to zero.
        if payload.len() < 12 || payload[5] & 0x01 == 0 {
            return None;
        }

        // overwrite any existing program map table
        self.program_map_table = Some(ProgramMapTable {
            audio: None,
            video: None,
            timed_metadata: vec![],
        });

        // the mapping table ends at the end of the current section
        let section_len = (((payload[1] & 0x0f) as u16) << 8) | payload[2] as u16;
        let table_end = 3 + section_len as usize - 4;

        // to determine where the table is, we have to figure out how
        // long the program info descriptors are
        let program_info_len = (((payload[10] & 0x0f) as u16) << 8) | payload[11] as u16;

        // advance the offset to the first entry in the mapping table
        let mut offset = 12 + program_info_len as usize;
        while offset < table_end && offset + 2 < payload.len() {
            let stream_type = payload[offset];
            let inner_pid =
                (((payload[offset + 1] & 0x1f) as u16) << 8) | payload[offset + 2] as u16;

            // only map a single elementary_pid for audio and video stream types
            // TODO: should this be done for metadata too? for now maintain behavior of
            //       multiple metadata streams
            if stream_type == H264_STREAM_TYPE {
                let mut pmt = self.program_map_table.as_mut().unwrap();
                if pmt.video.is_none() {
                    pmt.video = Some(inner_pid);
                }
            } else if stream_type == ADTS_STREAM_TYPE {
                let mut pmt = self.program_map_table.as_mut().unwrap();
                if pmt.audio.is_none() {
                    pmt.audio = Some(inner_pid);
                }
            } else if stream_type == METADATA_STREAM_TYPE {
                let pmt = self.program_map_table.as_mut().unwrap();
                // map inner_pid to stream type for metadata streams
                pmt.timed_metadata.push((inner_pid, stream_type));
            }

            if offset + 4 >= payload.len() {
                break;
            }

            // move to the next table entry
            // skip past the elementary stream descriptors, if present
            offset += ((((payload[offset + 3] & 0x0f) as usize) << 8)
                | (payload[offset + 4]) as usize)
                + 5;
        }

        let pmt_info = PmtPacketInfo {
            pid,
            payload_unit_start_indicator,
            program_map_table: self.program_map_table.as_ref().unwrap().clone(),
        };
        Some(ParsedTsPacket::Pmt(pmt_info))
    }

    fn process_pes(
        &mut self,
        packet: &[u8],
        offset: usize,
        pid: Pid,
        payload_unit_start_indicator: bool,
    ) -> ParsedTsPacket {
        // set the appropriate stream type
        let stream_type;
        if Some(pid) == self.program_map_table.as_ref().unwrap().video {
            stream_type = Some(H264_STREAM_TYPE);
        } else if Some(pid) == self.program_map_table.as_ref().unwrap().audio {
            stream_type = Some(ADTS_STREAM_TYPE);
        } else {
            // if not video or audio, it is timed-metadata or unknown
            // if unknown, stream_type will be undefined
            stream_type = self
                .program_map_table
                .as_ref()
                .unwrap()
                .timed_metadata
                .iter()
                .find(|t| t.0 == pid)
                .map(|t| t.1);
        }

        let info = PesPacketInfo {
            pid,
            payload_unit_start_indicator,
            stream_type,
            data: packet[offset..].to_owned(),
        };
        ParsedTsPacket::Pes(info)
    }
}
