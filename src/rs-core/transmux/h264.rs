//! H.264 byte-stream framing and the small part of SPS syntax needed by MP4.
//!
//! Encoded slices are never decoded or copied here. Their ranges refer to the
//! logical elementary stream. SPS/PPS are small decoder settings; owning those
//! bytes lets the next segment omit them without retaining the old media data.

use std::ops::Range;

use super::{transport::Elementary, Error, Result, Sample, MAX_SAMPLES};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Config {
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub sar: (u16, u16),
    pub frame_duration: Option<u32>,
    pub frame_only: bool,
}

/// Exp-Golomb codes describe counts as well as values. Limit both their bit
/// length and any loop they control so corrupt input cannot cause a long loop.
struct Bits {
    bytes: Vec<u8>,
    position: usize,
}

impl Bits {
    fn new(escaped: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(escaped.len());
        let mut zeros = 0;
        for &byte in escaped {
            if zeros >= 2 && byte == 3 {
                zeros = 0;
                continue;
            }
            bytes.push(byte);
            zeros = if byte == 0 { zeros + 1 } else { 0 };
        }
        Self { bytes, position: 0 }
    }

    fn read(&mut self, count: usize) -> Result<u32> {
        if count > 32 || self.position + count > self.bytes.len() * 8 {
            return Err(Error::Truncated);
        }
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1)
                | u32::from((self.bytes[self.position / 8] >> (7 - self.position % 8)) & 1);
            self.position += 1;
        }
        Ok(value)
    }

    fn flag(&mut self) -> Result<bool> {
        Ok(self.read(1)? != 0)
    }

    fn ue(&mut self) -> Result<u32> {
        let mut zeros = 0;
        while !self.flag()? {
            zeros += 1;
            if zeros > 30 {
                return Err(Error::Codec);
            }
        }
        Ok((1 << zeros) - 1 + self.read(zeros)?)
    }

    fn se(&mut self) -> Result<i32> {
        let code = self.ue()?;
        Ok(if code & 1 != 0 {
            (code / 2 + 1) as i32
        } else {
            -(code as i32 / 2)
        })
    }
}

fn scaling_list(bits: &mut Bits, count: usize) -> Result<()> {
    let (mut last, mut next) = (8, 8);
    for _ in 0..count {
        if next != 0 {
            next = (last + bits.se()?).rem_euclid(256);
        }
        if next != 0 {
            last = next;
        }
    }
    Ok(())
}

fn vui(bits: &mut Bits) -> Result<((u16, u16), Option<u32>)> {
    let mut sar = (1, 1);
    if bits.flag()? {
        let idc = bits.read(8)?;
        const RATIOS: [(u16, u16); 17] = [
            (1, 1),
            (1, 1),
            (12, 11),
            (10, 11),
            (16, 11),
            (40, 33),
            (24, 11),
            (20, 11),
            (32, 11),
            (80, 33),
            (18, 11),
            (15, 11),
            (64, 33),
            (160, 99),
            (4, 3),
            (3, 2),
            (2, 1),
        ];
        sar = if idc == 255 {
            (bits.read(16)? as u16, bits.read(16)? as u16)
        } else {
            *RATIOS.get(idc as usize).unwrap_or(&(1, 1))
        };
        if sar.0 == 0 || sar.1 == 0 {
            sar = (1, 1);
        }
    }
    if bits.flag()? {
        bits.read(1)?;
    } // overscan
    if bits.flag()? {
        // video_signal_type
        bits.read(4)?;
        if bits.flag()? {
            bits.read(24)?;
        }
    }
    if bits.flag()? {
        bits.ue()?;
        bits.ue()?;
    } // chroma location
    let duration = if bits.flag()? {
        let units = u64::from(bits.read(32)?);
        let scale = u64::from(bits.read(32)?);
        bits.read(1)?; // fixed_frame_rate_flag; the clock also gives a nominal fallback
                       // A progressive picture occupies two clock ticks in H.264 VUI. A zero
                       // time scale is invalid and cannot supply a fallback picture duration.
        (180_000 * units + scale / 2)
            .checked_div(scale)
            .and_then(|duration| u32::try_from(duration).ok())
            .filter(|&duration| duration > 0)
    } else {
        None
    };
    Ok((sar, duration))
}

fn parse_sps(sps: Vec<u8>, pps: Vec<u8>) -> Result<Config> {
    if sps.len() < 4 {
        return Err(Error::Codec);
    }
    let mut bits = Bits::new(&sps[1..]);
    let profile = bits.read(8)?;
    bits.read(16)?; // constraints and level
    bits.ue()?; // SPS ID
    let mut chroma = 1;
    let mut separate_colour_plane = false;
    if [100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135].contains(&profile) {
        chroma = bits.ue()?;
        if chroma > 3 {
            return Err(Error::Codec);
        }
        if chroma == 3 {
            separate_colour_plane = bits.flag()?;
        }
        bits.ue()?;
        bits.ue()?;
        bits.read(1)?;
        if bits.flag()? {
            for index in 0..if chroma == 3 { 12 } else { 8 } {
                if bits.flag()? {
                    scaling_list(&mut bits, if index < 6 { 16 } else { 64 })?;
                }
            }
        }
    }
    bits.ue()?; // log2_max_frame_num_minus4
    match bits.ue()? {
        0 => {
            bits.ue()?;
        }
        1 => {
            bits.read(1)?;
            bits.se()?;
            bits.se()?;
            let count = bits.ue()?;
            if count > 255 {
                return Err(Error::Codec);
            }
            for _ in 0..count {
                bits.se()?;
            }
        }
        2 => {}
        _ => return Err(Error::Codec),
    }
    bits.ue()?;
    bits.read(1)?; // reference count and gap flag
    let width_mbs = u64::from(bits.ue()?) + 1;
    let height_units = u64::from(bits.ue()?) + 1;
    let frame_only = bits.read(1)?;
    if frame_only == 0 {
        bits.read(1)?;
    }
    bits.read(1)?; // direct_8x8_inference_flag
    let mut crop = [0_u64; 4];
    if bits.flag()? {
        for value in &mut crop {
            *value = u64::from(bits.ue()?);
        }
    }
    // Crop units depend on chroma subsampling and field coding. Assuming 2x2
    // for every profile quietly gives wrong dimensions for 4:2:2 and 4:4:4.
    let chroma_array = if separate_colour_plane { 0 } else { chroma };
    let crop_x = if chroma_array == 1 || chroma_array == 2 {
        2
    } else {
        1
    };
    let crop_y = (if chroma_array == 1 { 2 } else { 1 }) * u64::from(2 - frame_only);
    let width = (width_mbs * 16)
        .checked_sub((crop[0] + crop[1]) * crop_x)
        .ok_or(Error::Codec)?;
    let height = (height_units * 16 * u64::from(2 - frame_only))
        .checked_sub((crop[2] + crop[3]) * crop_y)
        .ok_or(Error::Codec)?;
    let (sar, frame_duration) = if bits.flag()? {
        vui(&mut bits)?
    } else {
        ((1, 1), None)
    };
    let width = u16::try_from(width).map_err(|_| Error::Codec)?;
    let height = u16::try_from(height).map_err(|_| Error::Codec)?;
    if width == 0 || height == 0 {
        return Err(Error::Codec);
    }
    Ok(Config {
        sps,
        pps,
        width,
        height,
        sar,
        frame_duration,
        frame_only: frame_only != 0,
    })
}

fn nals(input: &[u8], stream: &Elementary) -> Result<Vec<Range<usize>>> {
    let (mut offset, mut zeros) = (0, 0);
    let mut begin = None;
    let mut nals = Vec::new();
    let mut exceeded_limit = false;
    stream.bytes.visit(input, 0..stream.bytes.len, |piece| {
        for &byte in piece {
            if byte == 1 && zeros >= 2 {
                if let Some(start) = begin {
                    if start < offset - zeros {
                        if nals.len() < MAX_SAMPLES * 8 {
                            nals.push(start..offset - zeros);
                        } else {
                            exceeded_limit = true;
                        }
                    }
                }
                begin = Some(offset + 1);
            }
            zeros = if byte == 0 { zeros + 1 } else { 0 };
            offset += 1;
        }
    });
    if exceeded_limit {
        return Err(Error::Limit);
    }
    if let Some(start) = begin {
        if start < offset - zeros {
            if nals.len() >= MAX_SAMPLES * 8 {
                return Err(Error::Limit);
            }
            nals.push(start..offset - zeros);
        }
    }
    Ok(nals)
}

pub(super) struct Video {
    pub config: Config,
    pub samples: Vec<Sample>,
    pub nals: Vec<Range<usize>>,
}

pub(super) fn parse(
    input: &[u8],
    stream: &Elementary,
    saved: Option<&Config>,
    previous_duration: Option<u32>,
) -> Result<Option<Video>> {
    if stream.bytes.len == 0 {
        return Ok(None);
    }
    let ranges = nals(input, stream)?;
    let has_aud = ranges.iter().any(|range| {
        stream
            .bytes
            .byte(input, range.start)
            .is_ok_and(|byte| byte & 31 == 9)
    });
    let mut sps = saved.map(|c| c.sps.clone()).unwrap_or_default();
    let mut pps = saved.map(|c| c.pps.clone()).unwrap_or_default();
    let mut samples: Vec<Sample> = Vec::new();
    let mut frame: Option<Sample> = None;
    let mut has_slice = false;
    let mut seen_slice = false;
    // Samples index the original range table directly. Building a second table
    // of identical ranges would double this metadata's peak WASM allocation.
    for (nal_index, range) in ranges.iter().enumerate() {
        let kind = stream.bytes.byte(input, range.start)? & 31;
        if kind == 7 || kind == 8 {
            if range.len() > 16_384 {
                return Err(Error::Limit);
            }
            let bytes = stream.bytes.small_copy(input, range.clone());
            if seen_slice && ((kind == 7 && sps != bytes) || (kind == 8 && pps != bytes)) {
                return Err(Error::ConfigChanged);
            }
            if kind == 7 {
                sps = bytes;
            } else {
                pps = bytes;
            }
        }
        let slice = kind == 1 || kind == 5;
        // AUD is the usual HLS frame delimiter. first_mb_in_slice also covers
        // streams which omit AUD. Multi-slice pictures keep their later slices
        // in the same sample. With AUD present, let it define the picture:
        // first_mb alone cannot distinguish complementary fields or slices
        // sent in an unusual order. Without AUD we accept progressive AVC.
        let first_slice = if slice && !has_aud {
            let prefix = stream
                .bytes
                .small_copy(input, range.start + 1..range.end.min(range.start + 17));
            Bits::new(&prefix).ue()? == 0
        } else {
            false
        };
        let boundary = has_slice && (kind == 9 || (6..=8).contains(&kind) || first_slice);
        if boundary {
            if samples.len() >= MAX_SAMPLES {
                return Err(Error::Limit);
            }
            samples.push(frame.take().ok_or(Error::Codec)?);
            has_slice = false;
        }
        if frame.is_none() {
            let time = stream.timestamp(range.start)?;
            frame = Some(Sample {
                pts: time.pts,
                dts: time.dts,
                duration: 0,
                key: false,
                payload: nal_index..nal_index,
                size: 0,
            });
        }
        let current = frame.as_mut().ok_or(Error::Codec)?;
        if slice && !has_slice {
            let time = stream.timestamp(range.start)?;
            current.pts = time.pts;
            current.dts = time.dts;
        }
        current.key |= kind == 5;
        current.size = current
            .size
            .checked_add(u32::try_from(range.len() + 4).map_err(|_| Error::Limit)?)
            .ok_or(Error::Limit)?;
        current.payload.end = nal_index + 1;
        has_slice |= slice;
        seen_slice |= slice;
    }
    if has_slice {
        if samples.len() >= MAX_SAMPLES {
            return Err(Error::Limit);
        }
        samples.push(frame.ok_or(Error::Codec)?);
    }
    if samples.is_empty() {
        return Ok(None);
    }
    if sps.is_empty() || pps.is_empty() {
        return Err(Error::MissingConfig);
    }
    let config = parse_sps(sps, pps)?;
    if !has_aud && !config.frame_only {
        return Err(Error::UnsupportedCodec);
    }
    for index in 0..samples.len().saturating_sub(1) {
        let delta = samples[index + 1].dts - samples[index].dts;
        samples[index].duration = u32::try_from(delta)
            .ok()
            .filter(|&d| d > 0)
            .ok_or(Error::MissingDuration)?;
    }
    let duration = samples
        .iter()
        .rev()
        .find_map(|s| (s.duration > 0).then_some(s.duration))
        .or(config.frame_duration)
        .or(previous_duration)
        .ok_or(Error::MissingDuration)?;
    samples.last_mut().ok_or(Error::Codec)?.duration = duration;
    Ok(Some(Video {
        config,
        samples,
        nals: ranges,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::{bytes::Stream, transport::Timestamp};
    use super::*;

    #[test]
    fn start_codes_cross_piece_boundaries() {
        let input = [0, 0, 0, 1, 0x65, 9, 0, 0, 1, 0x41, 8, 0, 0];
        let mut bytes = Stream::default();
        for at in 0..input.len() {
            bytes.push(at..at + 1);
        }
        let elementary = Elementary {
            id: 1,
            bytes,
            times: vec![Timestamp {
                offset: 0,
                pts: 0,
                dts: 0,
            }],
        };
        assert_eq!(nals(&input, &elementary).unwrap(), vec![4..6, 9..11]);
    }

    #[test]
    fn golomb_rejects_unbounded_and_truncated_codes() {
        assert!(Bits::new(&[0; 8]).ue().is_err());
        assert_eq!(Bits::new(&[0x80]).ue(), Ok(0));
        assert_eq!(Bits::new(&[0x40]).ue(), Ok(1));
        assert_eq!(Bits::new(&[0x60]).ue(), Ok(2));
    }
}
