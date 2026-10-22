//! MPEG-2 transport framing, program tables and PES headers.
//!
//! This module knows which bytes belong to each elementary stream. It does not
//! know how AAC frames or H.264 pictures are represented. Keeping that boundary
//! makes transport damage and codec errors easier to reason about separately.

use std::ops::Range;

use super::{bytes::Stream, Error, Result};

const PACKET_SIZE: usize = 188;

#[derive(Clone, Copy, Debug)]
pub(super) struct Timestamp {
    pub offset: usize,
    pub pts: i64,
    pub dts: i64,
}

#[derive(Default)]
pub(super) struct Elementary {
    pub id: u32,
    pub bytes: Stream,
    pub times: Vec<Timestamp>,
}

impl Elementary {
    pub fn timestamp(&self, offset: usize) -> Result<Timestamp> {
        let index = self.times.partition_point(|time| time.offset <= offset);
        self.times
            .get(index.saturating_sub(1))
            .copied()
            .ok_or(Error::MissingTimestamp)
    }
}

#[derive(Default)]
pub(super) struct Tracks {
    pub audio: Elementary,
    pub video: Elementary,
}

struct Packet {
    pid: u16,
    start: bool,
    counter: u8,
    discontinuity: bool,
    payload: Range<usize>,
}

fn packet(input: &[u8], at: usize) -> Result<Option<Packet>> {
    let data = &input[at..at + PACKET_SIZE];
    if data[0] != 0x47 || data[1] & 0x80 != 0 {
        return Err(Error::Transport);
    }
    if data[3] & 0xc0 != 0 {
        return Err(Error::Encrypted);
    }
    let adaptation_control = (data[3] >> 4) & 3;
    if adaptation_control == 0 {
        return Err(Error::Transport);
    }
    let mut offset = 4;
    let mut discontinuity = false;
    if adaptation_control & 2 != 0 {
        let length = usize::from(data[4]);
        offset = 5 + length;
        if offset > PACKET_SIZE {
            return Err(Error::Transport);
        }
        discontinuity = length > 0 && data[5] & 0x80 != 0;
    }
    if adaptation_control & 1 == 0 || offset == PACKET_SIZE {
        return Ok(None);
    }
    Ok(Some(Packet {
        pid: (u16::from(data[1] & 0x1f) << 8) | u16::from(data[2]),
        start: data[1] & 0x40 != 0,
        counter: data[3] & 15,
        discontinuity,
        payload: at + offset..at + PACKET_SIZE,
    }))
}

/// Ignore an optional prefix, but require repeated sync bytes before accepting
/// a position. Finding one 0x47 inside media data would be a weak sync check.
fn sync_offset(input: &[u8]) -> Result<usize> {
    (0..input.len().min(PACKET_SIZE * 3))
        .find(|&at| {
            at + PACKET_SIZE <= input.len()
                && input[at] == 0x47
                && (at + PACKET_SIZE >= input.len() || input[at + PACKET_SIZE] == 0x47)
                && (at + PACKET_SIZE * 2 >= input.len() || input[at + PACKET_SIZE * 2] == 0x47)
        })
        .ok_or(Error::Transport)
}

/// PSI sections can cross TS packets, and a packet can finish one section and
/// start another. Only these small tables are joined into a byte buffer. Their
/// 12-bit section length is checked before any allocation can follow it.
#[derive(Default)]
struct Sections {
    pending: Vec<u8>,
    counter: Option<u8>,
}

impl Sections {
    fn feed(&mut self, data: &[u8], packet: &Packet) -> Result<Vec<Vec<u8>>> {
        if let Some(previous) = self.counter {
            if previous == packet.counter && !packet.discontinuity {
                return Ok(Vec::new());
            }
            if (previous + 1) & 15 != packet.counter || packet.discontinuity {
                self.pending.clear();
            }
        }
        self.counter = Some(packet.counter);
        let mut completed = Vec::new();
        if packet.start {
            let pointer = usize::from(*data.first().ok_or(Error::Truncated)?);
            let prefix = data.get(1..1 + pointer).ok_or(Error::Truncated)?;
            if !self.pending.is_empty() {
                self.consume(prefix, &mut completed)?;
                // The pointer identifies a fresh section even if a previous
                // section was damaged or missing some continuation packets.
                self.pending.clear();
            }
            self.consume(&data[1 + pointer..], &mut completed)?;
        } else if !self.pending.is_empty() {
            self.consume(data, &mut completed)?;
        }
        Ok(completed)
    }

    fn consume(&mut self, mut data: &[u8], completed: &mut Vec<Vec<u8>>) -> Result<()> {
        while !data.is_empty() {
            if self.pending.is_empty() && data[0] == 0xff {
                break; // PSI stuffing, not another section.
            }
            let needed = if self.pending.len() < 3 {
                3 - self.pending.len()
            } else {
                let length =
                    (usize::from(self.pending[1] & 15) << 8) | usize::from(self.pending[2]);
                if !(4..=1021).contains(&length) {
                    return Err(Error::Transport);
                }
                3 + length - self.pending.len()
            };
            if needed == 0 {
                completed.push(std::mem::take(&mut self.pending));
                continue;
            }
            let count = needed.min(data.len());
            self.pending.extend_from_slice(&data[..count]);
            data = &data[count..];
        }
        // A section can finish exactly at the end of the packet.
        if self.pending.len() >= 3 {
            let length = (usize::from(self.pending[1] & 15) << 8) | usize::from(self.pending[2]);
            if !(4..=1021).contains(&length) {
                return Err(Error::Transport);
            }
            if self.pending.len() == 3 + length {
                completed.push(std::mem::take(&mut self.pending));
            }
        }
        Ok(())
    }
}

fn pat(data: &[u8]) -> Option<u16> {
    if data.len() < 12 || data[0] != 0 || data[5] & 1 == 0 {
        return None;
    }
    data[8..data.len() - 4].chunks_exact(4).find_map(|entry| {
        // Program zero describes the network, not an elementary program.
        (entry[0] != 0 || entry[1] != 0)
            .then(|| (u16::from(entry[2] & 31) << 8) | u16::from(entry[3]))
    })
}

fn pmt(data: &[u8]) -> Result<(u16, u16)> {
    if data.len() < 16 || data[0] != 2 || data[5] & 1 == 0 {
        return Err(Error::Transport);
    }
    let end = data.len() - 4; // CRC is outside the stream loop.
    let mut at = 12 + ((usize::from(data[10] & 15) << 8) | usize::from(data[11]));
    let (mut audio, mut video) = (0, 0);
    while at < end {
        let entry = data.get(at..at + 5).ok_or(Error::Truncated)?;
        let pid = (u16::from(entry[1] & 31) << 8) | u16::from(entry[2]);
        match entry[0] {
            0x0f if audio == 0 => audio = pid, // AAC with ADTS framing
            0x1b if video == 0 => video = pid, // AVC / H.264
            _ => {} // ID3 and other codecs do not produce exposed output here.
        }
        at += 5 + ((usize::from(entry[3] & 15) << 8) | usize::from(entry[4]));
    }
    if at != end {
        return Err(Error::Transport);
    }
    if audio == 0 && video == 0 {
        return Err(Error::UnsupportedCodec);
    }
    Ok((audio, video))
}

/// Decode all 33 bits without 32-bit intermediate shifts. JS bitwise arithmetic
/// needs special care here; integer arithmetic in Rust can express it directly.
fn timestamp(bytes: &[u8]) -> Result<i64> {
    if bytes.len() < 5 || bytes[0] & 1 == 0 || bytes[2] & 1 == 0 || bytes[4] & 1 == 0 {
        return Err(Error::MissingTimestamp);
    }
    Ok((i64::from(bytes[0] & 14) << 29)
        | (i64::from(bytes[1]) << 22)
        | (i64::from(bytes[2] & 254) << 14)
        | (i64::from(bytes[3]) << 7)
        | i64::from(bytes[4] >> 1))
}

#[derive(Default)]
struct Pes {
    bytes: Stream,
    counter: Option<u8>,
    started: bool,
    last_payload: Option<Range<usize>>,
    last_start: bool,
}

impl Pes {
    fn feed(&mut self, input: &[u8], packet: &Packet, out: &mut Elementary) -> Result<()> {
        if let Some(previous) = self.counter {
            if packet.counter == previous && !packet.discontinuity {
                // A repeated counter is a duplicate only when its payload is
                // identical. Otherwise data was lost or the counter is broken.
                if self.last_start == packet.start
                    && self
                        .last_payload
                        .as_ref()
                        .is_some_and(|range| input[range.clone()] == input[packet.payload.clone()])
                {
                    return Ok(());
                }
                return Err(Error::Continuity);
            }
            if (previous + 1) & 15 != packet.counter && !packet.discontinuity {
                return Err(Error::Continuity);
            }
        }
        self.counter = Some(packet.counter);
        self.last_payload = Some(packet.payload.clone());
        self.last_start = packet.start;
        if packet.discontinuity {
            self.bytes = Stream::default();
            self.started = false;
        }
        if packet.start {
            self.finish(input, out)?;
            self.started = true;
        }
        if self.started {
            self.bytes.push(packet.payload.clone());
        }
        Ok(())
    }

    fn finish(&mut self, input: &[u8], out: &mut Elementary) -> Result<()> {
        let stream = std::mem::take(&mut self.bytes);
        if stream.len == 0 {
            return Ok(());
        }
        if stream.len < 9 {
            return Err(Error::Truncated);
        }
        let header = stream.small_copy(input, 0..stream.len.min(19));
        if header[..3] != [0, 0, 1] || header[6] & 0xc0 != 0x80 {
            return Err(Error::Transport);
        }
        let declared = (usize::from(header[4]) << 8) | usize::from(header[5]);
        let end = if declared == 0 {
            stream.len
        } else {
            declared + 6
        };
        let start = 9 + usize::from(header[8]);
        if end > stream.len || start > end {
            return Err(Error::Truncated);
        }
        let flags = header[7] >> 6;
        if flags < 2 || start < 14 || (flags == 3 && start < 19) {
            return Err(Error::MissingTimestamp);
        }
        let pts = timestamp(&header[9..14])?;
        let dts = if flags == 3 {
            timestamp(&header[14..19])?
        } else {
            pts
        };
        out.times.push(Timestamp {
            offset: out.bytes.len,
            pts,
            dts,
        });
        out.bytes.append_range(&stream, start..end);
        Ok(())
    }
}

pub(super) fn demux(input: &[u8]) -> Result<Tracks> {
    let start = sync_offset(input)?;
    if !(input.len() - start).is_multiple_of(PACKET_SIZE) {
        return Err(Error::Truncated);
    }
    let (mut pat_sections, mut pmt_sections) = (Sections::default(), Sections::default());
    let (mut pmt_pid, mut pids) = (None, None);
    // Discover PIDs before visiting media. Some muxers put PES packets before
    // the PMT. A second cheap pass avoids retaining an unknown-packet queue.
    for at in (start..input.len()).step_by(PACKET_SIZE) {
        let Some(packet) = packet(input, at)? else {
            continue;
        };
        if packet.pid == 0 {
            for section in pat_sections.feed(&input[packet.payload.clone()], &packet)? {
                if let Some(pid) = pat(&section) {
                    pmt_pid = Some(pid);
                }
            }
        } else if Some(packet.pid) == pmt_pid {
            for section in pmt_sections.feed(&input[packet.payload.clone()], &packet)? {
                pids = Some(pmt(&section)?);
            }
            if pids.is_some() {
                break;
            }
        }
    }
    let (audio, video) = pids.ok_or(Error::MissingProgram)?;
    let mut tracks = Tracks::default();
    tracks.audio.id = u32::from(audio);
    tracks.video.id = u32::from(video);
    let (mut audio_pes, mut video_pes) = (Pes::default(), Pes::default());
    for at in (start..input.len()).step_by(PACKET_SIZE) {
        let Some(packet) = packet(input, at)? else {
            continue;
        };
        if audio != 0 && packet.pid == audio {
            audio_pes.feed(input, &packet, &mut tracks.audio)?;
        } else if video != 0 && packet.pid == video {
            video_pes.feed(input, &packet, &mut tracks.video)?;
        }
    }
    audio_pes.finish(input, &mut tracks.audio)?;
    video_pes.finish(input, &mut tracks.video)?;
    Ok(tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_full_timestamp_without_losing_high_bit() {
        assert_eq!(
            timestamp(&[0x2f, 0xff, 0xff, 0xff, 0xff]),
            Ok((1_i64 << 33) - 1)
        );
        assert_eq!(timestamp(&[0x21, 0, 1, 0, 1]), Ok(0));
        assert!(timestamp(&[0x20, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn section_header_can_cross_a_packet_boundary() {
        let section = [0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe1, 0, 0, 0, 0, 0];
        let mut sections = Sections::default();
        let mut found = Vec::new();
        sections.consume(&section[..2], &mut found).unwrap();
        assert!(found.is_empty());
        sections.consume(&section[2..], &mut found).unwrap();
        assert_eq!(found, vec![section]);
        assert_eq!(pat(&found[0]), Some(256));
    }
}
