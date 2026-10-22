//! AAC transport headers and the timestamp used by packed-audio HLS segments.
//!
//! ADTS headers describe the decoder settings and frame lengths. MP4 needs the
//! raw AAC payload, a two-byte AudioSpecificConfig and sample durations in the
//! audio clock. Keeping durations as integers avoids drift at 44.1 kHz.

use super::{
    transport::{Elementary, Timestamp},
    Error, Result, Sample, MAX_SAMPLES,
};

const RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Config {
    pub object_type: u8,
    pub frequency_index: u8,
    pub channels: u8,
    pub channel_config: u8,
    pub sample_rate: u32,
}

pub(super) struct Audio {
    pub config: Config,
    pub samples: Vec<Sample>,
}

/// Read a complete ADTS elementary stream, including frames crossing PES and
/// TS boundaries. A new PES timestamp applies at the next frame header, not
/// halfway through a frame which began in the previous PES.
pub(super) fn parse(input: &[u8], stream: &Elementary) -> Result<Option<Audio>> {
    let (mut at, mut frame_number, mut last_anchor) = (0, 0_i64, None);
    let mut config = None;
    let mut samples = Vec::new();
    while at < stream.bytes.len {
        if stream.bytes.len - at < 7 {
            return Err(Error::Truncated);
        }
        let header = stream.bytes.header::<7>(input, at)?;
        if header[0] != 0xff || header[1] & 0xf6 != 0xf0 {
            return Err(Error::Codec);
        }
        let frequency_index = (header[2] >> 2) & 15;
        let sample_rate = *RATES
            .get(usize::from(frequency_index))
            .ok_or(Error::Codec)?;
        let channel_config = ((header[2] & 1) << 2) | (header[3] >> 6);
        // Channel configuration zero requires a program-config element inside
        // AAC. We cannot truthfully put a channel count in MP4 without parsing
        // it. Multiple raw data blocks also require more than stripping ADTS.
        if channel_config == 0 || header[6] & 3 != 0 {
            return Err(Error::UnsupportedCodec);
        }
        let current = Config {
            object_type: (header[2] >> 6) + 1,
            frequency_index,
            channels: if channel_config == 7 {
                8
            } else {
                channel_config
            },
            channel_config,
            sample_rate,
        };
        if config.as_ref().is_some_and(|saved| saved != &current) {
            return Err(Error::ConfigChanged);
        }
        config = Some(current);
        let length = (usize::from(header[3] & 3) << 11)
            | (usize::from(header[4]) << 3)
            | usize::from(header[5] >> 5);
        let header_length = if header[1] & 1 != 0 { 7 } else { 9 };
        if length <= header_length || at + length > stream.bytes.len {
            return Err(Error::Truncated);
        }
        let time = stream.timestamp(at)?;
        if last_anchor != Some(time.offset) {
            frame_number = 0;
            last_anchor = Some(time.offset);
        }
        let dts = time.dts + frame_number * 1024 * 90_000 / i64::from(sample_rate);
        if samples.len() >= MAX_SAMPLES {
            return Err(Error::Limit);
        }
        samples.push(Sample {
            pts: dts,
            dts,
            duration: 1024,
            key: true,
            payload: at + header_length..at + length,
            size: (length - header_length) as u32,
        });
        at += length;
        frame_number += 1;
    }
    Ok(config.map(|config| Audio { config, samples }))
}

fn synchsafe(bytes: &[u8]) -> Result<usize> {
    if bytes.len() != 4 || bytes.iter().any(|&b| b & 0x80 != 0) {
        return Err(Error::Codec);
    }
    Ok(bytes
        .iter()
        .fold(0, |value, &byte| (value << 7) | usize::from(byte)))
}

/// Extract Apple's 33-bit transport timestamp from an ID3 PRIV frame. Other ID3
/// frames do not leave this module: the current worker API exposes media bytes
/// and timing only, with no metadata or caption delivery contract.
fn id3_timestamp(tag: &[u8]) -> Result<Option<i64>> {
    if tag.len() < 10 || ![3, 4].contains(&tag[3]) {
        return Err(Error::Codec);
    }
    if tag[5] & 0xc0 != 0 {
        return Err(Error::UnsupportedCodec);
    } // unsync / extended header
    let mut at = 10;
    while at + 10 <= tag.len() && tag[at] != 0 {
        let size = if tag[3] == 4 {
            synchsafe(&tag[at + 4..at + 8])?
        } else {
            u32::from_be_bytes(
                tag[at + 4..at + 8]
                    .try_into()
                    .map_err(|_| Error::Truncated)?,
            ) as usize
        };
        let end = at
            .checked_add(10 + size)
            .filter(|&end| end <= tag.len())
            .ok_or(Error::Truncated)?;
        if &tag[at..at + 4] == b"PRIV" {
            let value = &tag[at + 10..end];
            const OWNER: &[u8] = b"com.apple.streaming.transportStreamTimestamp\0";
            if value.starts_with(OWNER) && value.len() >= OWNER.len() + 8 {
                let encoded = u64::from_be_bytes(
                    value[OWNER.len()..OWNER.len() + 8]
                        .try_into()
                        .map_err(|_| Error::Truncated)?,
                );
                return Ok(Some((encoded & ((1_u64 << 33) - 1)) as i64));
            }
        }
        at = end;
    }
    Ok(None)
}

pub(super) fn is_packed(input: &[u8]) -> bool {
    input.starts_with(b"ID3") || (input.len() >= 2 && input[0] == 0xff && input[1] & 0xf6 == 0xf0)
}

pub(super) fn packed(input: &[u8], default_timestamp: i64) -> Result<Elementary> {
    let mut stream = Elementary {
        id: 1,
        ..Elementary::default()
    };
    let mut at = 0;
    let mut pending_timestamp = default_timestamp;
    let mut needs_timestamp = true;
    let mut frame_count = 0;
    while at < input.len() {
        if input[at..].starts_with(b"ID3") {
            let header = input.get(at..at + 10).ok_or(Error::Truncated)?;
            let length = 10 + synchsafe(&header[6..10])?;
            let tag = input.get(at..at + length).ok_or(Error::Truncated)?;
            if let Some(time) = id3_timestamp(tag)? {
                pending_timestamp = time;
                needs_timestamp = true;
            }
            at += length;
            // ID3v2.4 can append a ten-byte footer outside the tag body.
            if header[3] == 4 && header[5] & 0x10 != 0 {
                if input.get(at..at + 3) != Some(b"3DI".as_slice()) {
                    return Err(Error::Truncated);
                }
                at += 10;
                if at > input.len() {
                    return Err(Error::Truncated);
                }
            }
        } else {
            // Bound the range table before building it. Waiting for the AAC
            // parser to count samples would let tiny ADTS frames first grow a
            // very large table whose pages WASM could never return to the host.
            if frame_count >= MAX_SAMPLES {
                return Err(Error::Limit);
            }
            frame_count += 1;
            let header = input.get(at..at + 7).ok_or(Error::Truncated)?;
            if header[0] != 0xff || header[1] & 0xf6 != 0xf0 {
                return Err(Error::Codec);
            }
            let length = (usize::from(header[3] & 3) << 11)
                | (usize::from(header[4]) << 3)
                | usize::from(header[5] >> 5);
            if length < 7 || at + length > input.len() {
                return Err(Error::Truncated);
            }
            if needs_timestamp {
                stream.times.push(Timestamp {
                    offset: stream.bytes.len,
                    pts: pending_timestamp,
                    dts: pending_timestamp,
                });
                needs_timestamp = false;
            }
            stream.bytes.push(at..at + length);
            at += length;
        }
    }
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_crc_and_rejects_partial_adts() {
        let input = [0xff, 0xf0, 0x4c, 0x80, 1, 0x7f, 0xfc, 0, 0, 0x21, 0x10];
        let stream = packed(&input, 90_000).unwrap();
        let audio = parse(&input, &stream).unwrap().unwrap();
        assert_eq!(audio.config.sample_rate, 48_000);
        assert_eq!(audio.samples[0].payload, 9..11);
        assert_eq!(audio.samples[0].duration, 1024);
        assert_eq!(audio.samples[0].dts, 90_000);
        assert!(packed(&input[..10], 0).is_err());
    }
}
