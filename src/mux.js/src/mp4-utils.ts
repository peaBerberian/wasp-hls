/**
 * mux.js
 *
 * Copyright (c) Brightcove
 * Licensed Apache-2.0 https://github.com/videojs/mux.js/blob/master/LICENSE
 *
 * Functions that generate fragmented MP4s suitable for use with Media
 * Source Extensions.
 */

import { MAX_UINT32 } from "./numbers";
import { TrackInfo, TrackSample } from "./types";

const BOXES_NAME = {
  avc1: [97, 118, 99, 49],
  avcC: [97, 118, 99, 67],
  btrt: [98, 116, 114, 116],
  dinf: [100, 105, 110, 102],
  dref: [100, 114, 101, 102],
  esds: [101, 115, 100, 115],
  ftyp: [102, 116, 121, 112],
  hdlr: [104, 100, 108, 114],
  mdat: [109, 100, 97, 116],
  mdhd: [109, 100, 104, 100],
  mdia: [109, 100, 105, 97],
  mfhd: [109, 102, 104, 100],
  minf: [109, 105, 110, 102],
  moof: [109, 111, 111, 102],
  moov: [109, 111, 111, 118],
  mp4a: [109, 112, 52, 97],
  mvex: [109, 118, 101, 120],
  mvhd: [109, 118, 104, 100],
  pasp: [112, 97, 115, 112],
  sdtp: [115, 100, 116, 112],
  smhd: [115, 109, 104, 100],
  stbl: [115, 116, 98, 108],
  stco: [115, 116, 99, 111],
  stsc: [115, 116, 115, 99],
  stsd: [115, 116, 115, 100],
  stsz: [115, 116, 115, 122],
  stts: [115, 116, 116, 115],
  styp: [115, 116, 121, 112],
  tfdt: [116, 102, 100, 116],
  tfhd: [116, 102, 104, 100],
  traf: [116, 114, 97, 102],
  trak: [116, 114, 97, 107],
  trun: [116, 114, 117, 110],
  trex: [116, 114, 101, 120],
  tkhd: [116, 107, 104, 100],
  vmhd: [118, 109, 104, 100],
} as const;
const MAJOR_BRAND = new Uint8Array([
  "i".charCodeAt(0),
  "s".charCodeAt(0),
  "o".charCodeAt(0),
  "m".charCodeAt(0),
]);
const AVC1_BRAND = new Uint8Array([
  "a".charCodeAt(0),
  "v".charCodeAt(0),
  "c".charCodeAt(0),
  "1".charCodeAt(0),
]);
const MINOR_VERSION = new Uint8Array([0, 0, 0, 1]);
const VIDEO_HDLR = new Uint8Array([
  0x00, // version 0
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00,
  0x00,
  0x00, // pre_defined
  0x76,
  0x69,
  0x64,
  0x65, // handler_type: 'vide'
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x56,
  0x69,
  0x64,
  0x65,
  0x6f,
  0x48,
  0x61,
  0x6e,
  0x64,
  0x6c,
  0x65,
  0x72,
  0x00, // name: 'VideoHandler'
]);
const AUDIO_HDLR = new Uint8Array([
  0x00, // version 0
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00,
  0x00,
  0x00, // pre_defined
  0x73,
  0x6f,
  0x75,
  0x6e, // handler_type: 'soun'
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x00,
  0x00,
  0x00,
  0x00, // reserved
  0x53,
  0x6f,
  0x75,
  0x6e,
  0x64,
  0x48,
  0x61,
  0x6e,
  0x64,
  0x6c,
  0x65,
  0x72,
  0x00, // name: 'SoundHandler'
]);
const HDLR_TYPES = {
  video: VIDEO_HDLR,
  audio: AUDIO_HDLR,
} as const;
const DREF = new Uint8Array([
  0x00, // version 0
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00,
  0x00,
  0x01, // entry_count
  0x00,
  0x00,
  0x00,
  0x0c, // entry_size
  0x75,
  0x72,
  0x6c,
  0x20, // 'url' type
  0x00, // version 0
  0x00,
  0x00,
  0x01, // entry_flags
]);
const SMHD = new Uint8Array([
  0x00, // version
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00, // balance, 0 means centered
  0x00,
  0x00, // reserved
]);
const STCO = new Uint8Array([
  0x00, // version
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00,
  0x00,
  0x00, // entry_count
]);
const STSC = STCO;
const STSZ = new Uint8Array([
  0x00, // version
  0x00,
  0x00,
  0x00, // flags
  0x00,
  0x00,
  0x00,
  0x00, // sample_size
  0x00,
  0x00,
  0x00,
  0x00, // sample_count
]);
const STTS = STCO;
const VMHD = new Uint8Array([
  0x00, // version
  0x00,
  0x00,
  0x01, // flags
  0x00,
  0x00, // graphicsmode
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00, // opcolor
]);

/**
 * Create a box with the given name and corresponding content.
 * @param {Array.<number>} boxName - The name of the box, as ASCII character
 * codes.
 * @param {Array.<Uint8Array>} ...args - The box's content.
 * @returns {Uint8Array} - The created box.
 */
function createBox(
  boxName: readonly [number, number, number, number],
  ...args: Uint8Array[]
): Uint8Array {
  const payload = [];
  for (const arg of args) {
    payload.push(arg);
  }

  // calculate the total size we need to allocate
  let currentIndex = payload.length - 1;
  let size = 0;
  while (currentIndex >= 0) {
    size += payload[currentIndex].byteLength;
    currentIndex--;
  }
  const result = new Uint8Array(size + 8);
  const view = new DataView(
    result.buffer,
    result.byteOffset,
    result.byteLength
  );
  view.setUint32(0, result.byteLength);
  result.set(boxName, 4);

  // copy the payload into the result
  for (
    let payloadIdx = 0, offset = 8;
    payloadIdx < payload.length;
    payloadIdx++
  ) {
    result.set(payload[payloadIdx], offset);
    offset += payload[payloadIdx].byteLength;
  }
  return result;
}

/**
 * Create a template `dinf` ISOBMFF box.
 * @returns {Uint8Array}
 */
function createDinf(): Uint8Array {
  return createBox(BOXES_NAME.dinf, createBox(BOXES_NAME.dref, DREF));
}

/**
 * Create an `esds` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createEsds(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.esds,
    new Uint8Array([
      0x00, // version
      0x00,
      0x00,
      0x00, // flags

      // ES_Descriptor
      0x03, // tag, ES_DescrTag
      0x19, // length
      0x00,
      0x00, // ES_ID
      0x00, // streamDependenceFlag, URL_flag, reserved, streamPriority

      // DecoderConfigDescriptor
      0x04, // tag, DecoderConfigDescrTag
      0x11, // length
      0x40, // object type
      0x15, // streamType
      0x00,
      0x06,
      0x00, // bufferSizeDB
      0x00,
      0x00,
      0xda,
      0xc0, // maxBitrate
      0x00,
      0x00,
      0xda,
      0xc0, // avgBitrate

      // DecoderSpecificInfo
      0x05, // tag, DecoderSpecificInfoTag
      0x02, // length
      // ISO/IEC 14496-3, AudioSpecificConfig
      // for samplingFrequencyIndex see ISO/IEC 13818-7:2006, 8.1.3.2.2, Table 35
      (trackInfo.audioobjecttype << 3) |
        (trackInfo.samplingfrequencyindex >>> 1),
      (trackInfo.samplingfrequencyindex << 7) | (trackInfo.channelcount << 3),
      0x06,
      0x01,
      0x02, // GASpecificConfig
    ])
  );
}

/**
 * Create a template `ftyp` ISOBMFF box.
 * @returns {Uint8Array}
 */
function createFtyp(): Uint8Array {
  return createBox(
    BOXES_NAME.ftyp,
    MAJOR_BRAND,
    MINOR_VERSION,
    MAJOR_BRAND,
    AVC1_BRAND
  );
}

/**
 * Create an `hdlr` ISOBMFF box.
 * @param {string} hdlrType
 * @returns {Uint8Array}
 */
function createHdlr(hdlrType: "audio" | "video"): Uint8Array {
  return createBox(BOXES_NAME.hdlr, HDLR_TYPES[hdlrType]);
}

/**
 * Create a `mdat` ISOBMFF box with the corresponding data.
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */
function createMdat(data: Uint8Array): Uint8Array {
  return createBox(BOXES_NAME.mdat, data);
}

/**
 * Create a `mdhd` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createMdhd(trackInfo: TrackInfo): Uint8Array {
  const result = new Uint8Array([
    0x00, // version 0
    0x00,
    0x00,
    0x00, // flags
    0x00,
    0x00,
    0x00,
    0x02, // creation_time
    0x00,
    0x00,
    0x00,
    0x03, // modification_time
    0x00,
    0x01,
    0x5f,
    0x90, // timescale, 90,000 "ticks" per second

    (trackInfo.duration >>> 24) & 0xff,
    (trackInfo.duration >>> 16) & 0xff,
    (trackInfo.duration >>> 8) & 0xff,
    trackInfo.duration & 0xff, // duration
    0x55,
    0xc4, // 'und' language (undetermined)
    0x00,
    0x00,
  ]);

  // Use the sample rate from the trackInfo metadata, when it is
  // defined. The sample rate can be parsed out of an ADTS header, for
  // instance.
  if (trackInfo.samplerate) {
    result[12] = (trackInfo.samplerate >>> 24) & 0xff;
    result[13] = (trackInfo.samplerate >>> 16) & 0xff;
    result[14] = (trackInfo.samplerate >>> 8) & 0xff;
    result[15] = trackInfo.samplerate & 0xff;
  }

  return createBox(BOXES_NAME.mdhd, result);
}

/**
 * Create a `mdia` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createMdia(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.mdia,
    createMdhd(trackInfo),
    createHdlr(trackInfo.mediaType),
    createMinf(trackInfo)
  );
}

/**
 * Create a `mfhd` ISOBMFF box.
 * @param {number} sequenceNumber
 * @returns {Uint8Array}
 */
function createMfhd(sequenceNumber: number): Uint8Array {
  return createBox(
    BOXES_NAME.mfhd,
    new Uint8Array([
      0x00,
      0x00,
      0x00,
      0x00, // flags
      (sequenceNumber & 0xff000000) >> 24,
      (sequenceNumber & 0xff0000) >> 16,
      (sequenceNumber & 0xff00) >> 8,
      sequenceNumber & 0xff, // sequence_number
    ])
  );
}

/**
 * Create a `minf` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createMinf(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.minf,
    trackInfo.mediaType === "video"
      ? createBox(BOXES_NAME.vmhd, VMHD)
      : createBox(BOXES_NAME.smhd, SMHD),
    createDinf(),
    createStbl(trackInfo)
  );
}

/**
 * Create a `moof` ISOBMFF box.
 * @param {number} sequenceNumber
 * @param {Array.<Object>} tracks
 * @returns {Uint8Array}
 */
function createMoof(sequenceNumber: number, tracks: TrackInfo[]): Uint8Array {
  const trackFragments = tracks.map((t) => createTraf(t));
  return createBox(
    BOXES_NAME.moof,
    createMfhd(sequenceNumber),
    ...trackFragments
  );
}

/**
 * Creates a `moov` ISOBMFF box.
 * @param {Array.<Object>} tracks
 * @returns {Uint8Array}
 */
function createMoov(tracks: TrackInfo[]): Uint8Array {
  const boxes = tracks.map((t) => createTrak(t));
  return createBox(
    BOXES_NAME.moov,
    createMvhd(0xffffffff),
    ...boxes,
    createMvex(tracks)
  );
}

/**
 * Creates a `mvex` ISOBMFF box.
 * @param {Array.<Object>} tracks
 * @returns {Uint8Array}
 */
function createMvex(tracks: TrackInfo[]): Uint8Array {
  const boxes = tracks.map((t) => createTrex(t));
  return createBox(BOXES_NAME.mvex, ...boxes);
}

/**
 * Creates a `mvhd` ISOBMFF box.
 * @param {number} duration
 * @returns {Uint8Array}
 */
function createMvhd(duration: number): Uint8Array {
  const bytes = new Uint8Array([
    0x00, // version 0
    0x00,
    0x00,
    0x00, // flags
    0x00,
    0x00,
    0x00,
    0x01, // creation_time
    0x00,
    0x00,
    0x00,
    0x02, // modification_time
    0x00,
    0x01,
    0x5f,
    0x90, // timescale, 90,000 "ticks" per second
    (duration & 0xff000000) >> 24,
    (duration & 0xff0000) >> 16,
    (duration & 0xff00) >> 8,
    duration & 0xff, // duration
    0x00,
    0x01,
    0x00,
    0x00, // 1.0 rate
    0x01,
    0x00, // 1.0 volume
    0x00,
    0x00, // reserved
    0x00,
    0x00,
    0x00,
    0x00, // reserved
    0x00,
    0x00,
    0x00,
    0x00, // reserved
    0x00,
    0x01,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x01,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x40,
    0x00,
    0x00,
    0x00, // transformation: unity matrix
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00,
    0x00, // pre_defined
    0xff,
    0xff,
    0xff,
    0xff, // next_track_ID
  ]);
  return createBox(BOXES_NAME.mvhd, bytes);
}

/**
 * Creates a `sdtp` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createSdtp(trackInfo: TrackInfo): Uint8Array {
  const samples = trackInfo.samples ?? [];
  const bytes = new Uint8Array(4 + samples.length);

  // leave the full box header (4 bytes) all zero

  // write the sample table
  for (let i = 0; i < samples.length; i++) {
    const flags = samples[i].flags;

    bytes[i + 4] =
      (flags.dependsOn << 4) | (flags.isDependedOn << 2) | flags.hasRedundancy;
  }

  return createBox(BOXES_NAME.sdtp, bytes);
}

/**
 * Creates a `stbl` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createStbl(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.stbl,
    createStsd(trackInfo),
    createBox(BOXES_NAME.stts, STTS),
    createBox(BOXES_NAME.stsc, STSC),
    createBox(BOXES_NAME.stsz, STSZ),
    createBox(BOXES_NAME.stco, STCO)
  );
}

/**
 * Creates a `stsd` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createStsd(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.stsd,
    new Uint8Array([
      0x00, // version 0
      0x00,
      0x00,
      0x00, // flags
      0x00,
      0x00,
      0x00,
      0x01,
    ]),
    trackInfo.mediaType === "video"
      ? createAvc1(trackInfo)
      : createMp4a(trackInfo)
  );
}

/**
 * Creates an `avc1` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createAvc1(trackInfo: TrackInfo): Uint8Array {
  const sps = trackInfo.sps ?? [];
  const pps = trackInfo.pps ?? [];
  let sequenceParameterSets: number[] = [];
  let pictureParameterSets: number[] = [];

  // assemble the SPSs
  for (let i = 0; i < sps.length; i++) {
    sequenceParameterSets.push((sps[i].byteLength & 0xff00) >>> 8);
    sequenceParameterSets.push(sps[i].byteLength & 0xff); // sequenceParameterSetLength
    sequenceParameterSets = sequenceParameterSets.concat(
      Array.prototype.slice.call(sps[i])
    ); // SPS
  }

  // assemble the PPSs
  for (let i = 0; i < pps.length; i++) {
    pictureParameterSets.push((pps[i].byteLength & 0xff00) >>> 8);
    pictureParameterSets.push(pps[i].byteLength & 0xff);
    pictureParameterSets = pictureParameterSets.concat(
      Array.prototype.slice.call(pps[i])
    );
  }

  const avc1Box = [
    new Uint8Array([
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      0x00,
      0x01, // data_reference_index
      0x00,
      0x00, // pre_defined
      0x00,
      0x00, // reserved
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00, // pre_defined
      (trackInfo.width & 0xff00) >> 8,
      trackInfo.width & 0xff, // width
      (trackInfo.height & 0xff00) >> 8,
      trackInfo.height & 0xff, // height
      0x00,
      0x48,
      0x00,
      0x00, // horizresolution
      0x00,
      0x48,
      0x00,
      0x00, // vertresolution
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      0x00,
      0x01, // frame_count
      0x13,
      0x76,
      0x69,
      0x64,
      0x65,
      0x6f,
      0x6a,
      0x73,
      0x2d,
      0x63,
      0x6f,
      0x6e,
      0x74,
      0x72,
      0x69,
      0x62,
      0x2d,
      0x68,
      0x6c,
      0x73,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00, // compressorname
      0x00,
      0x18, // depth = 24
      0x11,
      0x11, // pre_defined = -1
    ]),
    createBox(
      BOXES_NAME.avcC,
      new Uint8Array(
        [
          0x01, // configurationVersion
          trackInfo.profileIdc, // AVCProfileIndication
          trackInfo.profileCompatibility, // profile_compatibility
          trackInfo.levelIdc, // AVCLevelIndication
          0xff, // lengthSizeMinusOne, hard-coded to 4 bytes
        ].concat(
          [sps.length], // numOfSequenceParameterSets
          sequenceParameterSets, // "SPS"
          [pps.length], // numOfPictureParameterSets
          pictureParameterSets // "PPS"
        )
      )
    ),
    createBox(
      BOXES_NAME.btrt,
      new Uint8Array([
        0x00,
        0x1c,
        0x9c,
        0x80, // bufferSizeDB
        0x00,
        0x2d,
        0xc6,
        0xc0, // maxBitrate
        0x00,
        0x2d,
        0xc6,
        0xc0, // avgBitrate
      ])
    ),
  ];

  if (trackInfo.sarRatio !== undefined) {
    const hSpacing = trackInfo.sarRatio[0];
    const vSpacing = trackInfo.sarRatio[1];

    avc1Box.push(
      createBox(
        BOXES_NAME.pasp,
        new Uint8Array([
          (hSpacing & 0xff000000) >> 24,
          (hSpacing & 0xff0000) >> 16,
          (hSpacing & 0xff00) >> 8,
          hSpacing & 0xff,
          (vSpacing & 0xff000000) >> 24,
          (vSpacing & 0xff0000) >> 16,
          (vSpacing & 0xff00) >> 8,
          vSpacing & 0xff,
        ])
      )
    );
  }

  return createBox(BOXES_NAME.avc1, ...avc1Box);
}

/**
 * Creates a `mp4a` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createMp4a(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.mp4a,
    new Uint8Array([
      // SampleEntry, ISO/IEC 14496-12
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      0x00,
      0x01, // data_reference_index

      // AudioSampleEntry, ISO/IEC 14496-12
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      (trackInfo.channelcount & 0xff00) >> 8,
      trackInfo.channelcount & 0xff, // channelcount
      (trackInfo.samplesize & 0xff00) >> 8,
      trackInfo.samplesize & 0xff, // samplesize
      0x00,
      0x00, // pre_defined
      0x00,
      0x00, // reserved

      (trackInfo.samplerate & 0xff00) >> 8,
      trackInfo.samplerate & 0xff,
      0x00,
      0x00, // samplerate, 16.16

      // MP4AudioSampleEntry, ISO/IEC 14496-14
    ]),
    createEsds(trackInfo)
  );
}

/**
 * Creates a `tkhd` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createTkhd(trackInfo: TrackInfo): Uint8Array {
  return createBox(
    BOXES_NAME.tkhd,
    new Uint8Array([
      0x00, // version 0
      0x00,
      0x00,
      0x07, // flags
      0x00,
      0x00,
      0x00,
      0x00, // creation_time
      0x00,
      0x00,
      0x00,
      0x00, // modification_time
      (trackInfo.id & 0xff000000) >> 24,
      (trackInfo.id & 0xff0000) >> 16,
      (trackInfo.id & 0xff00) >> 8,
      trackInfo.id & 0xff, // track_ID
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      (trackInfo.duration & 0xff000000) >> 24,
      (trackInfo.duration & 0xff0000) >> 16,
      (trackInfo.duration & 0xff00) >> 8,
      trackInfo.duration & 0xff, // duration
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00, // reserved
      0x00,
      0x00, // layer
      0x00,
      0x00, // alternate_group
      0x01,
      0x00, // non-audio trackInfo volume
      0x00,
      0x00, // reserved
      0x00,
      0x01,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x01,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x40,
      0x00,
      0x00,
      0x00, // transformation: unity matrix
      (trackInfo.width & 0xff00) >> 8,
      trackInfo.width & 0xff,
      0x00,
      0x00, // width
      (trackInfo.height & 0xff00) >> 8,
      trackInfo.height & 0xff,
      0x00,
      0x00, // height
    ])
  );
}

/**
 * Creates a `traf` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createTraf(trackInfo: TrackInfo): Uint8Array {
  const trackFragmentHeader = createBox(
    BOXES_NAME.tfhd,
    new Uint8Array([
      0x00, // version 0
      0x00,
      0x00,
      0x3a, // flags
      (trackInfo.id & 0xff000000) >> 24,
      (trackInfo.id & 0xff0000) >> 16,
      (trackInfo.id & 0xff00) >> 8,
      trackInfo.id & 0xff, // track_ID
      0x00,
      0x00,
      0x00,
      0x01, // sample_description_index
      0x00,
      0x00,
      0x00,
      0x00, // default_sample_duration
      0x00,
      0x00,
      0x00,
      0x00, // default_sample_size
      0x00,
      0x00,
      0x00,
      0x00, // default_sample_flags
    ])
  );

  const upperWordBaseMediaDecodeTime = Math.floor(
    trackInfo.baseMediaDecodeTime / MAX_UINT32
  );
  const lowerWordBaseMediaDecodeTime = Math.floor(
    trackInfo.baseMediaDecodeTime % MAX_UINT32
  );

  const trackFragmentDecodeTime = createBox(
    BOXES_NAME.tfdt,
    new Uint8Array([
      0x01, // version 1
      0x00,
      0x00,
      0x00, // flags
      // baseMediaDecodeTime
      (upperWordBaseMediaDecodeTime >>> 24) & 0xff,
      (upperWordBaseMediaDecodeTime >>> 16) & 0xff,
      (upperWordBaseMediaDecodeTime >>> 8) & 0xff,
      upperWordBaseMediaDecodeTime & 0xff,
      (lowerWordBaseMediaDecodeTime >>> 24) & 0xff,
      (lowerWordBaseMediaDecodeTime >>> 16) & 0xff,
      (lowerWordBaseMediaDecodeTime >>> 8) & 0xff,
      lowerWordBaseMediaDecodeTime & 0xff,
    ])
  );

  // the data offset specifies the number of bytes from the start of
  // the containing moof to the first payload byte of the associated
  // mdat
  const dataOffset =
    32 + // tfhd
    20 + // tfdt
    8 + // traf header
    16 + // mfhd
    8 + // moof header
    8; // mdat header

  // audio tracks require less metadata
  if (trackInfo.mediaType === "audio") {
    const trackFragmentRun = createTrun(trackInfo, dataOffset);
    return createBox(
      BOXES_NAME.traf,
      trackFragmentHeader,
      trackFragmentDecodeTime,
      trackFragmentRun
    );
  }

  // video tracks should contain an independent and disposable samples
  // box (sdtp)
  // generate one and adjust offsets to match
  const sampleDependencyTable = createSdtp(trackInfo);
  const trackFragmentRun = createTrun(
    trackInfo,
    sampleDependencyTable.length + dataOffset
  );
  return createBox(
    BOXES_NAME.traf,
    trackFragmentHeader,
    trackFragmentDecodeTime,
    trackFragmentRun,
    sampleDependencyTable
  );
}

/**
 * Creates a `trak` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createTrak(trackInfo: TrackInfo): Uint8Array {
  trackInfo.duration = trackInfo.duration || 0xffffffff;
  return createBox(
    BOXES_NAME.trak,
    createTkhd(trackInfo),
    createMdia(trackInfo)
  );
}

/**
 * Creates a `trex` ISOBMFF box.
 * @param {Object} trackInfo
 * @returns {Uint8Array}
 */
function createTrex(trackInfo: TrackInfo): Uint8Array {
  const result = new Uint8Array([
    0x00, // version 0
    0x00,
    0x00,
    0x00, // flags
    (trackInfo.id & 0xff000000) >> 24,
    (trackInfo.id & 0xff0000) >> 16,
    (trackInfo.id & 0xff00) >> 8,
    trackInfo.id & 0xff, // track_ID
    0x00,
    0x00,
    0x00,
    0x01, // default_sample_description_index
    0x00,
    0x00,
    0x00,
    0x00, // default_sample_duration
    0x00,
    0x00,
    0x00,
    0x00, // default_sample_size
    0x00,
    0x01,
    0x00,
    0x01, // default_sample_flags
  ]);
  // the last two bytes of default_sample_flags is the sample
  // degradation priority, a hint about the importance of this sample
  // relative to others. Lower the degradation priority for all sample
  // BOXES_NAME other than video.
  if (trackInfo.mediaType !== "video") {
    result[result.length - 1] = 0x00;
  }

  return createBox(BOXES_NAME.trex, result);
}

/**
 * Creates the header of a `trun` ISOBMFF box.
 *
 * This method assumes all samples are uniform. That is, if a
 * duration is present for the first sample, it will be present for
 * all subsequent samples.
 * @param {Array.<Object>} samples
 * @param {number} offset
 * @returns {Array.<number>}
 */
function createTrunHeader(samples: TrackSample[], offset: number): number[] {
  let durationPresent = 0;
  let sizePresent = 0;
  let flagsPresent = 0;
  let compositionTimeOffset = 0;

  // trun flag constants
  if (samples.length) {
    if (samples[0].duration !== undefined) {
      durationPresent = 0x1;
    }
    if (samples[0].size !== undefined) {
      sizePresent = 0x2;
    }
    if (samples[0].flags !== undefined) {
      flagsPresent = 0x4;
    }
    if (samples[0].compositionTimeOffset !== undefined) {
      compositionTimeOffset = 0x8;
    }
  }

  return [
    0x00, // version 0
    0x00,
    durationPresent | sizePresent | flagsPresent | compositionTimeOffset,
    0x01, // flags
    (samples.length & 0xff000000) >>> 24,
    (samples.length & 0xff0000) >>> 16,
    (samples.length & 0xff00) >>> 8,
    samples.length & 0xff, // sample_count
    (offset & 0xff000000) >>> 24,
    (offset & 0xff0000) >>> 16,
    (offset & 0xff00) >>> 8,
    offset & 0xff, // data_offset
  ];
}

/**
 * Creates a `trun` ISOBMFF box for a video media.
 * @param {Object} trackInfo
 * @param {number} initialOffset
 * @returns {Uint8Array}
 */
function createVideoTrun(
  trackInfo: TrackInfo,
  initialOffset: number
): Uint8Array {
  const samples = trackInfo.samples ?? [];
  const offset = initialOffset + 8 + 12 + 16 * samples.length;
  const header = createTrunHeader(samples, offset);
  const bytes = new Uint8Array(header.length + samples.length * 16);
  bytes.set(header);
  let bytesOffest = header.length;

  for (let i = 0; i < samples.length; i++) {
    const sample = samples[i];

    bytes[bytesOffest++] = (sample.duration & 0xff000000) >>> 24;
    bytes[bytesOffest++] = (sample.duration & 0xff0000) >>> 16;
    bytes[bytesOffest++] = (sample.duration & 0xff00) >>> 8;
    bytes[bytesOffest++] = sample.duration & 0xff; // sample_duration
    bytes[bytesOffest++] = (sample.size & 0xff000000) >>> 24;
    bytes[bytesOffest++] = (sample.size & 0xff0000) >>> 16;
    bytes[bytesOffest++] = (sample.size & 0xff00) >>> 8;
    bytes[bytesOffest++] = sample.size & 0xff; // sample_size
    bytes[bytesOffest++] =
      (sample.flags.isLeading << 2) | sample.flags.dependsOn;
    bytes[bytesOffest++] =
      (sample.flags.isDependedOn << 6) |
      (sample.flags.hasRedundancy << 4) |
      (sample.flags.paddingValue << 1) |
      sample.flags.isNonSyncSample;
    bytes[bytesOffest++] = sample.flags.degradationPriority & (0xf0 << 8);
    bytes[bytesOffest++] = sample.flags.degradationPriority & 0x0f; // sample_flags
    bytes[bytesOffest++] = (sample.compositionTimeOffset & 0xff000000) >>> 24;
    bytes[bytesOffest++] = (sample.compositionTimeOffset & 0xff0000) >>> 16;
    bytes[bytesOffest++] = (sample.compositionTimeOffset & 0xff00) >>> 8;

    // sample_composition_time_offset
    bytes[bytesOffest++] = sample.compositionTimeOffset & 0xff;
  }
  return createBox(BOXES_NAME.trun, bytes);
}

/**
 * Creates a `trun` ISOBMFF box for an audio media.
 * @param {Object} trackInfo
 * @param {number} initialOffset
 * @returns {Uint8Array}
 */
function createAudioTrun(
  trackInfo: TrackInfo,
  initialOffset: number
): Uint8Array {
  const samples = trackInfo.samples ?? [];
  const offset = initialOffset + 8 + 12 + 8 * samples.length;
  const header = createTrunHeader(samples, offset);
  const bytes = new Uint8Array(header.length + samples.length * 8);
  bytes.set(header);
  let bytesOffest = header.length;

  for (let i = 0; i < samples.length; i++) {
    const sample = samples[i];
    bytes[bytesOffest++] = (sample.duration & 0xff000000) >>> 24;
    bytes[bytesOffest++] = (sample.duration & 0xff0000) >>> 16;
    bytes[bytesOffest++] = (sample.duration & 0xff00) >>> 8;
    bytes[bytesOffest++] = sample.duration & 0xff; // sample_duration
    bytes[bytesOffest++] = (sample.size & 0xff000000) >>> 24;
    bytes[bytesOffest++] = (sample.size & 0xff0000) >>> 16;
    bytes[bytesOffest++] = (sample.size & 0xff00) >>> 8;
    bytes[bytesOffest++] = sample.size & 0xff; // sample_size
  }

  return createBox(BOXES_NAME.trun, bytes);
}

/**
 * Creates a `trun` ISOBMFF box.
 * @param {Object} trackInfo
 * @param {number} initialOffset
 * @returns {Uint8Array}
 */
function createTrun(trackInfo: TrackInfo, initialOffset: number): Uint8Array {
  if (trackInfo.mediaType === "audio") {
    return createAudioTrun(trackInfo, initialOffset);
  }

  return createVideoTrun(trackInfo, initialOffset);
}

/**
 * Creates an fmp4 initialization segment for segments with the corresponding
 * track information.
 * @param {Array.<Object>} tracks
 * @returns {Uint8Array}
 */
function createInitSegment(tracks: TrackInfo[]): Uint8Array {
  const ftypBox = createFtyp();
  const moovBox = createMoov(tracks);

  const result = new Uint8Array(ftypBox.byteLength + moovBox.byteLength);
  result.set(ftypBox);
  result.set(moovBox, ftypBox.byteLength);
  return result;
}

export { createFtyp, createMdat, createMoof, createMoov, createInitSegment };
