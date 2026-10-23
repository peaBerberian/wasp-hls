//! TODO: Add incremental input/output and validate LL-HLS parts as a separate
//! contract; an arbitrary network chunk is not a complete media part.
//!
//! Whole-segment MPEG-TS / packed AAC to fragmented MP4 conversion.
//!
//! The transport and codec modules borrow payload ranges from one input buffer.
//! `mp4` sizes its output before copying those ranges. The only persistent media
//! state is decoder configuration and timing. No encoded GOP is cached.
//! This matters in WASM: freeing a Vec makes its space reusable, but does not
//! shrink linear memory. Avoiding a temporary full-size copy lowers the memory
//! high-water mark for the entire remaining lifetime of a player.

mod aac;
mod bytes;
#[cfg(feature = "transmux-test-api")]
mod ffi;
mod h264;
mod mp4;
mod transport;

use std::ops::Range;

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_SAMPLES: usize = 131_072;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    Truncated = 1,
    Transport,
    Encrypted,
    Continuity,
    MissingProgram,
    MissingTimestamp,
    UnsupportedCodec,
    Codec,
    ConfigChanged,
    MissingConfig,
    MissingDuration,
    Limit,
    InvalidSeed,
}

type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Truncated => "truncated segment",
            Self::Transport => "invalid transport stream",
            Self::Encrypted => "encrypted transport stream",
            Self::Continuity => "missing transport packet",
            Self::MissingProgram => "missing program table",
            Self::MissingTimestamp => "missing timestamp",
            Self::UnsupportedCodec => "unsupported codec framing",
            Self::Codec => "invalid codec data",
            Self::ConfigChanged => "codec changed within segment",
            Self::MissingConfig => "missing decoder configuration",
            Self::MissingDuration => "missing picture duration",
            Self::Limit => "segment exceeds transmux limits",
            Self::InvalidSeed => "invalid decode-time seed",
        }
    }
}

/// Media decode-time hint. Once a timeline has an origin, later hints cannot
/// move it. The caller must set `reset` when crossing a discontinuity.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub reset: bool,
    pub seed: Option<(f64, u32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub start: u64,
    pub end: u64,
    pub timescale: u32,
}

pub struct Output {
    pub data: Vec<u8>,
    pub timing: Timing,
}

#[derive(Clone, Debug)]
struct Sample {
    pts: i64,
    dts: i64,
    duration: u32,
    key: bool,
    // Audio uses byte offsets; video uses indices into its NAL range list.
    payload: Range<usize>,
    size: u32,
}

#[derive(Clone, Copy)]
struct Timeline {
    origin: i64,
    seed: f64,
}

impl Timeline {
    fn decode_time(self, dts: i64, timescale: u32, keep_original: bool) -> Result<u64> {
        let ticks = self.seed
            + if keep_original {
                dts as f64
            } else {
                (dts - self.origin) as f64
            };
        let value = (ticks.max(0.0) * f64::from(timescale) / 90_000.0).floor();
        if !value.is_finite() || value > (1_u64 << 53) as f64 {
            return Err(Error::InvalidSeed);
        }
        Ok(value as u64)
    }
}

/// One instance belongs to one media timeline. It has no browser dependencies,
/// raw pointers, imported functions or hidden global state, so the same parser
/// can be exercised natively and through the project's handwritten WASM ABI.
#[derive(Default)]
pub struct Transmuxer {
    timeline: Option<Timeline>,
    rollover_reference: Option<i64>,
    video_config: Option<h264::Config>,
    last_video_duration: Option<u32>,
    // Last successfully emitted video end, in the input 90 kHz clock.
    video_end: Option<i64>,
    raw_timestamp: f64,
    sequence: u32,
    packed: Option<bool>,
    pub keep_original_timestamps: bool,
}

fn unwrap_timestamp(value: i64, reference: i64) -> i64 {
    const PERIOD: i64 = 1_i64 << 33;
    // Select the signed distance within half a 33-bit clock cycle. This works
    // for forward playback and seeks backwards across rollover alike.
    reference + (value - reference + PERIOD / 2).rem_euclid(PERIOD) - PERIOD / 2
}

impl Transmuxer {
    pub fn reset(&mut self) {
        let keep_original_timestamps = self.keep_original_timestamps;
        *self = Self {
            keep_original_timestamps,
            ..Self::default()
        };
    }

    pub fn transmux(&mut self, input: &[u8], options: Options) -> Result<Option<Output>> {
        // Reject input before making any data-dependent allocation. The host
        // performs the same check before copying into linear memory.
        if input.len() > MAX_INPUT_BYTES {
            return Err(Error::Limit);
        }
        if input.is_empty() {
            return Ok(None);
        }
        let seed = match options.seed {
            Some((value, scale)) if scale > 0 && value.is_finite() && value >= 0.0 => {
                value * 90_000.0 / f64::from(scale)
            }
            Some(_) => return Err(Error::InvalidSeed),
            None => 0.0,
        };
        if seed > (1_u64 << 53) as f64 {
            return Err(Error::InvalidSeed);
        }
        let packed = aac::is_packed(input);
        if options.reset {
            self.reset();
        }
        // A failed format change must not discard a working timeline. Build
        // that first segment in fresh state, then replace us only on success.
        if self.packed.is_some_and(|previous| previous != packed) {
            let mut fresh = Self {
                keep_original_timestamps: self.keep_original_timestamps,
                ..Self::default()
            };
            let result = fresh.transmux(input, options);
            if result.is_ok() {
                *self = fresh;
            }
            return result;
        }
        // Do not commit codec/timeline state until every parser and MP4 sizing
        // step succeeds. A bad request must not shift the next good segment.
        let mut tracks = if packed {
            transport::Tracks {
                audio: aac::packed(input, self.raw_timestamp as i64)?,
                ..Default::default()
            }
        } else {
            transport::demux(input)?
        };
        let reference = self.rollover_reference.or_else(|| {
            tracks
                .video
                .times
                .first()
                .or(tracks.audio.times.first())
                .map(|time| time.dts)
        });
        if let Some(reference) = reference {
            for stream in [&mut tracks.video, &mut tracks.audio] {
                for time in &mut stream.times {
                    time.dts = unwrap_timestamp(time.dts, reference);
                    time.pts = unwrap_timestamp(time.pts, time.dts);
                }
            }
        }
        let mut video = h264::parse(
            input,
            &tracks.video,
            self.video_config.as_ref(),
            self.last_video_duration,
        )?;
        let mut audio = aac::parse(input, &tracks.audio)?;
        let first_dts = video
            .as_ref()
            .and_then(|v| v.samples.first())
            .map(|s| s.dts)
            .or_else(|| {
                audio
                    .as_ref()
                    .and_then(|a| a.samples.first())
                    .map(|s| s.dts)
            });
        let Some(first_dts) = first_dts else {
            return Ok(None);
        };
        let timeline = self.timeline.unwrap_or(Timeline {
            origin: first_dts,
            seed,
        });
        if let Some(video) = video.as_mut() {
            let continuous = self.video_config.as_ref() == Some(&video.config)
                && self.video_end == Some(video.samples[0].dts);
            if !continuous && !video.samples[0].key {
                // A fresh decoder cannot use pictures which depend on missing
                // references. Drop them until random access is possible. Keep
                // the keyframe's real timestamps: moving it backwards would
                // change A/V synchronization. During continuous playback the
                // decoder already has its references, so no GOP copy is needed.
                let first_key = video
                    .samples
                    .iter()
                    .position(|sample| sample.key)
                    .unwrap_or(video.samples.len());
                video.samples.drain(..first_key);
            }
        }
        if video.as_ref().is_some_and(|video| video.samples.is_empty()) {
            // Do not emit an audio-only initialization for a muxed input whose
            // video is not decodable yet. Changing the initialized track set
            // would prevent the later video fragment from being appended.
            return Ok(None);
        }
        let video_dts = video.as_ref().map(|video| video.samples[0].dts);
        let video_base = video_dts
            .map(|dts| timeline.decode_time(dts, 90_000, self.keep_original_timestamps))
            .transpose()?;
        if let Some(audio) = audio.as_mut() {
            // Share the video origin so A/V encoder delay survives remuxing.
            // Also move the floor when a back-seek clamps video decode time to
            // zero. Keeping the first segment's floor would erase older audio.
            let floor = match (video_dts, video_base) {
                (Some(dts), Some(base)) if !self.keep_original_timestamps => dts - base as i64,
                _ => 0,
            };
            audio.samples.retain(|sample| sample.dts >= floor);
        }
        if audio.as_ref().is_some_and(|audio| audio.samples.is_empty()) {
            audio = None;
        }
        if audio.is_none() && video.is_none() {
            return Ok(None);
        }
        let audio_base = audio
            .as_ref()
            .map(|audio| {
                timeline.decode_time(
                    audio.samples[0].dts,
                    audio.config.sample_rate,
                    self.keep_original_timestamps,
                )
            })
            .transpose()?;
        let timing = if let Some(video) = &video {
            let start = video_base.ok_or(Error::Codec)?;
            let duration: u64 = video
                .samples
                .iter()
                .map(|s| u64::from(s.duration))
                .sum::<u64>();
            Timing {
                start,
                end: start + duration,
                timescale: 90_000,
            }
        } else {
            let audio = audio.as_ref().ok_or(Error::Codec)?;
            let start = audio_base.ok_or(Error::Codec)?;
            Timing {
                start,
                end: start + audio.samples.len() as u64 * 1024,
                timescale: audio.config.sample_rate,
            }
        };
        let data = mp4::assemble(
            input,
            &tracks,
            audio.as_ref(),
            audio_base,
            video.as_ref(),
            video_base,
            self.sequence,
        )?;
        if let Some(video) = video {
            let last = video.samples.last().expect("nonempty video samples");
            self.video_end = Some(last.dts + i64::from(last.duration));
            self.last_video_duration = Some(last.duration);
            self.video_config = Some(video.config);
        } else {
            self.video_end = None;
        }
        self.timeline = Some(timeline);
        self.rollover_reference = tracks
            .video
            .times
            .last()
            .or(tracks.audio.times.last())
            .map(|t| t.dts);
        if packed {
            if let Some(audio) = audio {
                let anchor = tracks.audio.times.last().ok_or(Error::MissingTimestamp)?;
                let count = audio
                    .samples
                    .iter()
                    .filter(|sample| sample.payload.start >= anchor.offset)
                    .count();
                // Carry fractional 90 kHz ticks across ID3-less segments. A
                // rounded per-segment step slowly drifts at rates like 44.1 kHz.
                let anchor_time =
                    if tracks.audio.times.len() == 1 && anchor.dts == self.raw_timestamp as i64 {
                        self.raw_timestamp
                    } else {
                        anchor.dts as f64
                    };
                self.raw_timestamp = anchor_time
                    + count as f64 * 1024.0 * 90_000.0 / f64::from(audio.config.sample_rate);
            }
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.packed = Some(packed);
        Ok(Some(Output { data, timing }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_in_both_directions() {
        let period = 1_i64 << 33;
        assert_eq!(unwrap_timestamp(5, period - 5), period + 5);
        assert_eq!(unwrap_timestamp(period - 5, 5), -5);
        assert_eq!(unwrap_timestamp(5, period * 3), period * 3 + 5);
    }

    #[test]
    fn timeline_seed_and_audio_clock_are_independent() {
        let timeline = Timeline {
            origin: 900_000,
            seed: 1_800_000.0,
        };
        assert_eq!(timeline.decode_time(990_000, 90_000, false), Ok(1_890_000));
        assert_eq!(timeline.decode_time(990_000, 48_000, false), Ok(1_008_000));
        assert_eq!(timeline.decode_time(-2_000_000, 48_000, false), Ok(0));
    }
}
