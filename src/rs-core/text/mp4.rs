//! Narrow ISO BMFF `stpp` extraction. XML parsing is deliberately independent.
//! Offsets and sample sizes select documents, never the entire mdat payload.
use super::{Error, Limits};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
/// One supported TTML track found in the initialization segment.
pub struct Track {
    pub id: u32,
    /// Number of media-time ticks per second.
    pub timescale: u32,
    pub namespace: String,
    pub schema_location: String,
    /// One-based sample-description index selected by trex.
    pub sample_description_index: u32,
    /// Sample duration in ticks when tfhd/trun does not provide one.
    pub default_duration: u32,
    /// Sample size in bytes when tfhd/trun does not provide one.
    pub default_size: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Initialization {
    pub tracks: Vec<Track>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// One XML sample. Its bytes borrow the media segment, so extraction does not
/// copy each document. The caller must keep that segment alive while using it.
pub struct Sample<'a> {
    pub track_id: u32,
    /// Number of media-time ticks per second.
    pub timescale: u32,
    /// Decode time in track ticks, as given by tfdt and sample durations.
    pub decode_time: u64,
    /// Decode time plus the composition offset, in track ticks. The wide signed
    /// type preserves both a u64 decode time and a negative composition offset.
    pub presentation_time: i128,
    /// Sample duration in track ticks. It is separate from XML cue timing.
    pub duration: u32,
    pub document: &'a [u8],
}
#[derive(Clone, Copy)]
struct BoxRef<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
    start: usize,
    payload_start: usize,
}
fn invalid(s: &str) -> Error {
    Error::Invalid(format!("IMSC MP4: {s}"))
}
fn boxes(bytes: &[u8], origin: usize) -> Result<Vec<BoxRef<'_>>, Error> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if out.len() >= Limits::default().nodes {
            return Err(Error::Limit("boxes"));
        }
        let mut c = Cursor::new(&bytes[pos..]);
        let size = c.u32()?;
        let kind = c.take(4)?.try_into().unwrap();
        let size = match size {
            0 => bytes.len() - pos,
            1 => usize::try_from(c.u64()?).map_err(|_| invalid("box size overflow"))?,
            n => n as usize,
        };
        let header = c.pos;
        if size < header || size > bytes.len() - pos {
            return Err(invalid("truncated or undersized box"));
        }
        out.push(BoxRef {
            kind,
            payload: &bytes[pos + header..pos + size],
            start: origin + pos,
            payload_start: origin + pos + header,
        });
        pos += size;
    }
    Ok(out)
}
fn children(b: BoxRef<'_>) -> Result<Vec<BoxRef<'_>>, Error> {
    boxes(b.payload, b.payload_start)
}
fn unique<'a>(values: &[BoxRef<'a>], kind: &[u8; 4]) -> Result<BoxRef<'a>, Error> {
    let mut found = values.iter().filter(|b| &b.kind == kind);
    let b = *found
        .next()
        .ok_or_else(|| invalid(&format!("missing {}", String::from_utf8_lossy(kind))))?;
    if found.next().is_some() {
        return Err(invalid("duplicate required box"));
    }
    Ok(b)
}
fn path<'a>(mut b: BoxRef<'a>, names: &[[u8; 4]]) -> Result<BoxRef<'a>, Error> {
    for name in names {
        b = unique(&children(b)?, name)?;
    }
    Ok(b)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| invalid("offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated field"))?;
        self.pos = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn full(&mut self) -> Result<(u8, u32), Error> {
        let n = self.u32()?;
        Ok(((n >> 24) as u8, n & 0xff_ffff))
    }
    fn string(&mut self) -> Result<String, Error> {
        let n = self.bytes[self.pos..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated stpp string"))?;
        let value = std::str::from_utf8(self.take(n)?)
            .map_err(|_| invalid("invalid stpp UTF-8"))?
            .into();
        self.take(1)?;
        Ok(value)
    }
}

pub fn parse_init(bytes: &[u8]) -> Result<Initialization, Error> {
    if bytes.len() > Limits::default().bytes {
        return Err(Error::Limit("bytes"));
    }
    let top = boxes(bytes, 0)?;
    let moov = unique(&top, b"moov")?;
    let content = children(moov)?;
    let mvex = unique(&content, b"mvex")?;
    let mut defaults = std::collections::BTreeMap::new();
    for b in children(mvex)?.iter().filter(|b| b.kind == *b"trex") {
        let mut c = Cursor::new(b.payload);
        let (version, flags) = c.full()?;
        if version != 0 || flags != 0 {
            return Err(invalid("trex version/flags"));
        }
        let id = c.u32()?;
        let index = c.u32()?;
        let duration = c.u32()?;
        let size = c.u32()?;
        c.u32()?;
        if defaults.insert(id, (index, duration, size)).is_some() {
            return Err(invalid("duplicate trex"));
        }
    }
    let mut tracks = Vec::new();
    for trak in content.iter().filter(|b| b.kind == *b"trak") {
        let stsd = path(*trak, &[*b"mdia", *b"minf", *b"stbl", *b"stsd"])?;
        let mut c = Cursor::new(stsd.payload);
        let (v, f) = c.full()?;
        if v != 0 || f != 0 {
            return Err(invalid("stsd version/flags"));
        }
        let count = c.u32()?;
        let entries = boxes(&stsd.payload[c.pos..], stsd.payload_start + c.pos)?;
        if entries.len() != count as usize {
            return Err(invalid("stsd entry count"));
        }
        if !entries.iter().any(|b| b.kind == *b"stpp") {
            continue;
        }
        let tkhd = path(*trak, &[*b"tkhd"])?;
        let mut c = Cursor::new(tkhd.payload);
        let (v, _) = c.full()?;
        match v {
            0 => {
                c.take(8)?;
            }
            1 => {
                c.take(16)?;
            }
            _ => return Err(invalid("tkhd version")),
        }
        let id = c.u32()?;
        let mdhd = path(*trak, &[*b"mdia", *b"mdhd"])?;
        let mut c = Cursor::new(mdhd.payload);
        let (v, _) = c.full()?;
        match v {
            0 => {
                c.take(8)?;
            }
            1 => {
                c.take(16)?;
            }
            _ => return Err(invalid("mdhd version")),
        }
        let timescale = c.u32()?;
        if timescale == 0 {
            return Err(invalid("zero timescale"));
        }
        let (default_index, default_duration, default_size) = *defaults
            .get(&id)
            .ok_or_else(|| invalid("missing trex for stpp track"))?;
        for (i, entry) in entries
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == *b"stpp")
        {
            let mut c = Cursor::new(entry.payload);
            c.take(6)?;
            let reference = u16::from_be_bytes(c.take(2)?.try_into().unwrap());
            if reference != 1 {
                return Err(Error::Unsupported("external stpp data reference".into()));
            }
            let namespace = c.string()?;
            let schema_location = c.string()?;
            let auxiliary = c.string()?;
            if namespace != "http://www.w3.org/ns/ttml" {
                return Err(Error::Unsupported(format!("stpp namespace {namespace}")));
            }
            if !auxiliary.is_empty() {
                return Err(Error::Unsupported("stpp auxiliary resources".into()));
            }
            let index = (i + 1) as u32;
            if index != default_index {
                return Err(Error::Unsupported(
                    "multiple stpp sample descriptions".into(),
                ));
            }
            tracks.push(Track {
                id,
                timescale,
                namespace,
                schema_location,
                sample_description_index: index,
                default_duration,
                default_size,
            });
        }
    }
    if tracks.is_empty() {
        return Err(invalid("no stpp track"));
    }
    let mut ids = std::collections::BTreeSet::new();
    if tracks.iter().any(|t| !ids.insert(t.id)) {
        return Err(invalid("duplicate track id"));
    }
    Ok(Initialization { tracks })
}

/// `file_offset` is the resource byte offset of `bytes[0]`, needed for explicit
/// base-data-offsets (including HLS byte ranges). Returned documents borrow bytes.
pub fn extract<'a>(
    bytes: &'a [u8],
    track: &Track,
    file_offset: u64,
) -> Result<Vec<Sample<'a>>, Error> {
    if bytes.len() > Limits::default().bytes {
        return Err(Error::Limit("bytes"));
    }
    if track.timescale == 0 {
        return Err(invalid("zero timescale"));
    }
    let top = boxes(bytes, 0)?;
    if !top.iter().any(|b| b.kind == *b"moof") {
        return Err(invalid("missing moof"));
    }
    let mdats: Vec<Range<usize>> = top
        .iter()
        .filter(|b| b.kind == *b"mdat")
        .map(|b| b.payload_start..b.payload_start + b.payload.len())
        .collect();
    let mut samples = Vec::new();
    for moof in top.iter().filter(|b| b.kind == *b"moof") {
        let mut previous_end = None;
        let mut unknown_predecessor = false;
        for traf in children(*moof)?.iter().filter(|b| b.kind == *b"traf") {
            let content = children(*traf)?;
            let tfhd = unique(&content, b"tfhd")?;
            let mut c = Cursor::new(tfhd.payload);
            let (v, flags) = c.full()?;
            if v != 0 || flags & !0x03003b != 0 {
                return Err(invalid("tfhd version/flags"));
            }
            let id = c.u32()?;
            // This API extracts one selected track. Unknown track layouts must
            // not silently establish the implicit base of another track.
            if id != track.id {
                previous_end = None;
                unknown_predecessor = true;
                continue;
            }
            let base = if flags & 1 != 0 {
                let absolute = c.u64()?;
                usize::try_from(
                    absolute
                        .checked_sub(file_offset)
                        .ok_or_else(|| invalid("base offset precedes segment"))?,
                )
                .map_err(|_| invalid("base offset overflow"))?
            } else if flags & 0x020000 != 0 {
                moof.start
            } else {
                if unknown_predecessor {
                    return Err(Error::Unsupported(
                        "implicit base after another track".into(),
                    ));
                }
                previous_end.unwrap_or(moof.start)
            };
            let description = if flags & 2 != 0 {
                c.u32()?
            } else {
                track.sample_description_index
            };
            if description != track.sample_description_index {
                return Err(Error::Unsupported("stpp sample description switch".into()));
            }
            let duration = if flags & 8 != 0 {
                c.u32()?
            } else {
                track.default_duration
            };
            let size = if flags & 16 != 0 {
                c.u32()?
            } else {
                track.default_size
            };
            if flags & 32 != 0 {
                c.u32()?;
            }
            if content
                .iter()
                .any(|b| [*b"senc", *b"saiz", *b"saio", *b"subs"].contains(&b.kind))
            {
                return Err(Error::Unsupported("encrypted or subsampled stpp".into()));
            }
            let tfdt = unique(&content, b"tfdt")?;
            let mut c = Cursor::new(tfdt.payload);
            let (v, f) = c.full()?;
            if f != 0 {
                return Err(invalid("tfdt flags"));
            }
            let mut decode = match v {
                0 => u64::from(c.u32()?),
                1 => c.u64()?,
                _ => return Err(invalid("tfdt version")),
            };
            let mut cursor = base;
            for trun in content.iter().filter(|b| b.kind == *b"trun") {
                let mut c = Cursor::new(trun.payload);
                let (v, f) = c.full()?;
                if v > 1 || f & !0x000f05 != 0 || f & 4 != 0 && f & 0x400 != 0 {
                    return Err(invalid("trun version/flags"));
                }
                let count = c.u32()? as usize;
                if count > Limits::default().nodes.saturating_sub(samples.len()) {
                    return Err(Error::Limit("samples"));
                }
                if f & 1 != 0 {
                    let offset = c.u32()? as i32;
                    cursor = usize::try_from(base as i128 + i128::from(offset))
                        .map_err(|_| invalid("trun data offset"))?;
                }
                if f & 4 != 0 {
                    c.u32()?;
                }
                if flags & 0x010000 != 0 && count != 0 {
                    return Err(invalid("samples in duration-is-empty fragment"));
                }
                for _ in 0..count {
                    let duration = if f & 0x100 != 0 { c.u32()? } else { duration };
                    let size = if f & 0x200 != 0 { c.u32()? } else { size };
                    if f & 0x400 != 0 {
                        c.u32()?;
                    }
                    let offset = if f & 0x800 != 0 {
                        let n = c.u32()?;
                        if v == 1 {
                            i64::from(n as i32)
                        } else {
                            i64::from(n)
                        }
                    } else {
                        0
                    };
                    let end = cursor
                        .checked_add(size as usize)
                        .ok_or_else(|| invalid("sample size overflow"))?;
                    if !mdats.iter().any(|r| cursor >= r.start && end <= r.end) {
                        return Err(invalid("sample lies outside mdat"));
                    }
                    samples.push(Sample {
                        track_id: id,
                        timescale: track.timescale,
                        decode_time: decode,
                        presentation_time: i128::from(decode) + i128::from(offset),
                        duration,
                        document: &bytes[cursor..end],
                    });
                    cursor = end;
                    decode = decode
                        .checked_add(u64::from(duration))
                        .ok_or_else(|| invalid("decode time overflow"))?;
                }
            }
            previous_end = Some(cursor);
        }
    }
    Ok(samples)
}

#[cfg(test)]
mod tests;
