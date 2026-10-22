//! The MP4 writer owns container structure, never codec parsing or timeline policy.
//!
//! Initialization boxes and moof tables are small. Build those first, sum the
//! exact payload size, then allocate the final buffer once. Unlike nested box
//! builders, this never constructs a separate media-sized mdat buffer.

use super::{aac, h264, transport, CachedGop, Error, Result, Sample, MAX_INPUT_BYTES};

#[derive(Default)]
struct Boxes {
    bytes: Vec<u8>,
}

impl Boxes {
    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn zeros(&mut self, count: usize) {
        self.bytes.resize(self.bytes.len() + count, 0);
    }
    fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    fn full(&mut self, version: u8, flags: u32) {
        self.u32((u32::from(version) << 24) | flags);
    }
    fn patch(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn boxed(&mut self, kind: &[u8; 4], write: impl FnOnce(&mut Self)) {
        let start = self.bytes.len();
        self.u32(0);
        self.raw(kind);
        write(self);
        // Only small headers pass through here. Media box sizes are checked
        // separately before allocation and written straight to final output.
        self.patch(start, (self.bytes.len() - start) as u32);
    }

    fn matrix(&mut self) {
        // Identity transform: the diagonal uses 16.16, 16.16 and 2.30 fixed
        // point values. Off-diagonal entries are zero: no rotation or skew.
        for value in [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
            self.u32(value);
        }
    }
}

#[derive(Clone, Copy)]
enum Config<'a> {
    Audio(&'a aac::Config),
    Video(&'a h264::Config),
}

impl Config<'_> {
    fn video(self) -> bool {
        matches!(self, Self::Video(_))
    }
    fn timescale(self) -> u32 {
        match self {
            Self::Audio(a) => a.sample_rate,
            Self::Video(_) => 90_000,
        }
    }
    fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Audio(_) => (0, 0),
            Self::Video(v) => (v.width, v.height),
        }
    }
}

fn sample_entry(out: &mut Boxes, config: Config<'_>) {
    match config {
        Config::Audio(audio) => out.boxed(b"mp4a", |out| {
            out.zeros(6);
            out.u16(1); // SampleEntry: data_reference_index
            out.zeros(8); // AudioSampleEntry reserved fields
            out.u16(u16::from(audio.channels));
            out.u16(16); // decoded sample size; encoded AAC size varies
            out.zeros(4); // predefined and reserved
                          // The legacy sample-entry field cannot hold rates above 65535.
                          // The AudioSpecificConfig and mdhd retain the full sampling rate.
            out.u32(if audio.sample_rate <= 65535 {
                audio.sample_rate << 16
            } else {
                0
            });
            out.boxed(b"esds", |out| {
                out.full(0, 0);
                // MPEG-4 descriptors are tag/length records, not MP4 boxes.
                // ES_Descriptor (3) holds DecoderConfigDescriptor (4), which
                // holds the two-byte AudioSpecificConfig (5). SLConfig (6)
                // says this MP4 carries the timing and synchronization itself.
                out.raw(&[3, 25, 0, 1, 0, 4, 17, 0x40, 0x15, 0, 0, 0]);
                out.u32(0);
                out.u32(0); // bitrate is unspecified, not guessed
                out.raw(&[
                    5,
                    2,
                    (audio.object_type << 3) | (audio.frequency_index >> 1),
                    (audio.frequency_index << 7) | (audio.channel_config << 3),
                    6,
                    1,
                    2,
                ]);
            });
        }),
        Config::Video(video) => out.boxed(b"avc1", |out| {
            out.zeros(6);
            out.u16(1);
            out.zeros(16); // predefined and reserved VisualSampleEntry fields
            out.u16(video.width);
            out.u16(video.height);
            out.u32(0x0048_0000); // horizontal resolution, 72 dpi in 16.16
            out.u32(0x0048_0000); // vertical resolution
            out.u32(0);
            out.u16(1); // one picture per sample
            out.zeros(32); // optional compressor name, empty
            out.u16(24); // default display depth
            out.u16(0xffff);
            out.boxed(b"avcC", |out| {
                // Configuration version, profile/constraints/level, four-byte
                // NAL lengths (0xff), then one SPS with reserved bits set.
                // SPS/PPS lengths here are 16-bit; mdat NAL lengths are 32-bit.
                out.raw(&[1, video.sps[1], video.sps[2], video.sps[3], 0xff, 0xe1]);
                out.u16(video.sps.len() as u16);
                out.raw(&video.sps);
                out.raw(&[1]);
                out.u16(video.pps.len() as u16);
                out.raw(&video.pps);
            });
            out.boxed(b"pasp", |out| {
                out.u32(u32::from(video.sar.0));
                out.u32(u32::from(video.sar.1));
            });
        }),
    }
}

fn track(out: &mut Boxes, id: u32, config: Config<'_>) {
    // trak describes identity and display properties. mdia describes the media
    // clock, decoder and source of bytes. Its sample tables stay empty because
    // each moof supplies those tables for the following media fragment.
    out.boxed(b"trak", |out| {
        out.boxed(b"tkhd", |out| {
            out.full(0, 7); // enabled, in movie, in preview
            out.zeros(8); // creation and modification times
            out.u32(id);
            out.u32(0);
            out.u32(u32::MAX); // duration unknown in a streaming initialization
            out.zeros(8);
            out.u16(0); // display layer
            out.u16(0); // alternate group
            out.u16(if config.video() { 0 } else { 0x100 });
            out.u16(0);
            out.matrix();
            let (width, height) = config.dimensions();
            out.u32(u32::from(width) << 16);
            out.u32(u32::from(height) << 16);
        });
        out.boxed(b"mdia", |out| {
            out.boxed(b"mdhd", |out| {
                out.full(0, 0);
                out.zeros(8);
                out.u32(config.timescale());
                out.u32(u32::MAX);
                out.u16(0x55c4);
                out.u16(0); // language: und (undefined)
            });
            out.boxed(b"hdlr", |out| {
                out.full(0, 0);
                out.u32(0);
                out.raw(if config.video() { b"vide" } else { b"soun" });
                out.zeros(12);
                out.raw(if config.video() {
                    b"VideoHandler\0"
                } else {
                    b"SoundHandler\0"
                });
            });
            out.boxed(b"minf", |out| {
                if config.video() {
                    out.boxed(b"vmhd", |out| {
                        out.full(0, 1);
                        out.zeros(8);
                    });
                } else {
                    out.boxed(b"smhd", |out| {
                        out.full(0, 0);
                        out.zeros(4);
                    });
                }
                out.boxed(b"dinf", |out| {
                    // One self-contained data reference. Media lives in this
                    // stream's mdat boxes, never at an external URL.
                    out.boxed(b"dref", |out| {
                        out.full(0, 0);
                        out.u32(1);
                        out.boxed(b"url ", |out| out.full(0, 1));
                    });
                });
                out.boxed(b"stbl", |out| {
                    out.boxed(b"stsd", |out| {
                        out.full(0, 0);
                        out.u32(1);
                        sample_entry(out, config);
                    });
                    for kind in [b"stts", b"stsc", b"stco"] {
                        out.boxed(kind, |out| {
                            out.full(0, 0);
                            out.u32(0);
                        });
                    }
                    out.boxed(b"stsz", |out| {
                        out.full(0, 0);
                        out.u32(0);
                        out.u32(0);
                    });
                });
            });
        });
    });
}

fn init(tracks: &[(u32, Config<'_>)]) -> Vec<u8> {
    // Emit an initialization on each segment, as the TS implementation does.
    // A caller can therefore reset, seek or change representation without
    // relying on an old initialization still being available in JS memory.
    let mut out = Boxes::default();
    out.boxed(b"ftyp", |out| {
        out.raw(b"isom");
        out.u32(1);
        out.raw(b"isomiso6avc1mp41");
    });
    out.boxed(b"moov", |out| {
        out.boxed(b"mvhd", |out| {
            out.full(0, 0);
            out.zeros(8);
            out.u32(90_000);
            out.u32(u32::MAX);
            out.u32(0x10000);
            out.u16(0x100);
            out.zeros(10);
            out.matrix();
            out.zeros(24);
            out.u32(u32::MAX);
        });
        for &(id, config) in tracks {
            track(out, id, config);
        }
        out.boxed(b"mvex", |out| {
            for &(id, _) in tracks {
                out.boxed(b"trex", |out| {
                    out.full(0, 0);
                    out.u32(id);
                    out.u32(1);
                    out.zeros(12);
                });
            }
        });
    });
    out.bytes
}

fn moof(
    id: u32,
    base: u64,
    samples: &[Sample],
    prefix: &[Sample],
    video: bool,
    sequence: u32,
) -> Result<Vec<u8>> {
    let mut out = Boxes::default();
    let mut data_offset = 0;
    let mut invalid_cts = false;
    out.boxed(b"moof", |out| {
        out.boxed(b"mfhd", |out| {
            out.full(0, 0);
            out.u32(sequence);
        });
        out.boxed(b"traf", |out| {
            out.boxed(b"tfhd", |out| {
                out.full(0, 0x020000);
                out.u32(id);
            }); // default-base-is-moof
            out.boxed(b"tfdt", |out| {
                out.full(1, 0);
                out.u64(base);
            });
            out.boxed(b"trun", |out| {
                // Version 1 permits negative presentation offsets for B-frames.
                // A version-0 unsigned field would turn those into huge delays.
                out.full(
                    if video { 1 } else { 0 },
                    if video { 0x0f01 } else { 0x0301 },
                );
                out.u32((samples.len() + prefix.len()) as u32);
                data_offset = out.bytes.len();
                out.u32(0);
                for sample in prefix.iter().chain(samples) {
                    out.u32(sample.duration);
                    out.u32(sample.size);
                    if video {
                        out.u32(if sample.key { 0x0200_0000 } else { 0x0101_0000 });
                        let cts = i32::try_from(sample.pts - sample.dts);
                        invalid_cts |= cts.is_err();
                        out.u32(cts.unwrap_or(0) as u32);
                    }
                }
            });
        });
    });
    if invalid_cts {
        return Err(Error::Codec);
    }
    out.patch(data_offset, (out.bytes.len() + 8) as u32);
    Ok(out.bytes)
}

fn payload_size(samples: &[Sample]) -> usize {
    samples.iter().map(|sample| sample.size as usize).sum()
}

fn mdat(out: &mut Vec<u8>, size: usize) {
    out.extend_from_slice(&((size + 8) as u32).to_be_bytes());
    out.extend_from_slice(b"mdat");
}

// The caller supplies the timeline choices explicitly. The writer cannot assume
// audio comes from the same timeline, has the same clock or should start at zero.
#[allow(clippy::too_many_arguments)]
pub(super) fn assemble(
    input: &[u8],
    tracks: &transport::Tracks,
    audio: Option<&aac::Audio>,
    audio_base: Option<u64>,
    video: Option<&h264::Video>,
    video_base: Option<u64>,
    prefix: Option<&CachedGop>,
    sequence: u32,
) -> Result<Vec<u8>> {
    let mut configs = Vec::with_capacity(2);
    if let Some(video) = video {
        configs.push((tracks.video.id, Config::Video(&video.config)));
    }
    if let Some(audio) = audio {
        configs.push((tracks.audio.id, Config::Audio(&audio.config)));
    }
    let init = init(&configs);
    let audio_header = audio
        .map(|audio| {
            moof(
                tracks.audio.id,
                audio_base.unwrap_or(0),
                &audio.samples,
                &[],
                false,
                sequence,
            )
        })
        .transpose()?;
    let video_header = video
        .map(|video| {
            moof(
                tracks.video.id,
                video_base.unwrap_or(0),
                &video.samples,
                prefix.map_or(&[], |g| &g.samples),
                true,
                sequence,
            )
        })
        .transpose()?;
    let audio_size = audio.map_or(0, |audio| payload_size(&audio.samples));
    let video_size =
        video.map_or(0, |video| payload_size(&video.samples)) + prefix.map_or(0, |g| g.bytes.len());
    let length = init.len()
        + audio_header
            .as_ref()
            .map_or(0, |h| h.len() + 8 + audio_size)
        + video_header
            .as_ref()
            .map_or(0, |h| h.len() + 8 + video_size);
    if length > MAX_INPUT_BYTES * 2 {
        return Err(Error::Limit);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    out.extend_from_slice(&init);
    // Keep audio mdat before video mdat. Chromium has historically depended on
    // that order while changing initialization segments during quality switches.
    if let (Some(audio), Some(header)) = (audio, audio_header) {
        out.extend_from_slice(&header);
        mdat(&mut out, audio_size);
        for sample in &audio.samples {
            tracks
                .audio
                .bytes
                .copy(input, sample.payload.clone(), &mut out);
        }
    }
    if let (Some(video), Some(header)) = (video, video_header) {
        out.extend_from_slice(&header);
        mdat(&mut out, video_size);
        if let Some(prefix) = prefix {
            out.extend_from_slice(&prefix.bytes);
        }
        for sample in &video.samples {
            for nal in &video.nals[sample.payload.clone()] {
                out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                tracks.video.bytes.copy(input, nal.clone(), &mut out);
            }
        }
    }
    debug_assert_eq!(out.len(), length);
    Ok(out)
}
