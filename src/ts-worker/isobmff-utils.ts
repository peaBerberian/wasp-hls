/**
 * Parse track Fragment Decode Time to get a precize initial time for this
 * segment (in the media timescale).
 *
 * Stops at the first tfdt encountered from the beginning of the file.
 * Returns this time.
 * `undefined` if not found.
 * @param {Uint8Array} buffer
 * @returns {Number | undefined}
 */
function getTrackFragmentDecodeTime(buffer: Uint8Array): number | undefined {
  return getTRAFs(buffer)
    .map(getTrackFragmentDecodeTimeFromTRAF)
    .find((decodeTime) => decodeTime !== undefined);
}

/**
 * Calculate segment duration approximation by additioning the duration from
 * every samples in a trun ISOBMFF box.
 *
 * Returns `undefined` if we could not parse the duration.
 * @param {Uint8Array} buffer
 * @returns {number | undefined}
 */
function getDurationFromTrun(buffer: Uint8Array): number | undefined {
  const trafs = getTRAFs(buffer);
  if (trafs.length === 0) {
    return undefined;
  }

  let completeDuration: number = 0;
  for (const traf of trafs) {
    const truns = getBoxesContent(traf, 0x7472756e /* trun */);
    if (truns.length === 0) {
      return undefined;
    }
    for (const trun of truns) {
      const duration = getDurationFromSingleTrun(traf, trun, undefined);
      if (duration === undefined) {
        return undefined;
      }
      completeDuration += duration;
    }
  }
  return completeDuration;
}

function getInitTrackInfo(buffer: Uint8Array):
  | Map<
      number,
      {
        timescale: number;
        type: "audio" | "video" | "other";
        defaultSampleDuration: number | undefined;
      }
    >
  | undefined {
  const moov = getBoxContent(buffer, 0x6d6f6f76 /* moov */);
  if (moov === null) {
    return undefined;
  }

  const traks = getBoxesContent(moov, 0x7472616b /* trak */);
  if (traks.length === 0) {
    return undefined;
  }

  const defaultSampleDurationByTrackId =
    getDefaultSampleDurationFromTREXByTrackId(moov);
  const trackInfo = new Map<
    number,
    {
      timescale: number;
      type: "audio" | "video" | "other";
      defaultSampleDuration: number | undefined;
    }
  >();
  for (const trak of traks) {
    const trackId = getTrackIdFromTRAK(trak);
    const mdia = getBoxContent(trak, 0x6d646961 /* mdia */);
    if (trackId === undefined || mdia === null) {
      continue;
    }
    const timescale = getTimescaleFromMDIA(mdia);
    if (timescale !== undefined) {
      trackInfo.set(trackId, {
        timescale,
        type: getTrackTypeFromTRAK(trak),
        defaultSampleDuration: defaultSampleDurationByTrackId.get(trackId),
      });
    }
  }

  return trackInfo.size === 0 ? undefined : trackInfo;
}

/**
 * Get timescale information from the first movie track. Found in init segments.
 * `undefined` if not found or not parsed.
 * @param {Uint8Array} buffer
 * @returns {Number | undefined}
 */
function getMDHDTimescale(buffer: Uint8Array): number | undefined {
  const trackInfo = getInitTrackInfo(buffer);
  if (trackInfo === undefined) {
    return undefined;
  }
  return trackInfo.values().next().value?.timescale;
}

function getIsobmfTimeInfo(
  buffer: Uint8Array,
  initTrackInfoByTrackId: Map<
    number,
    {
      timescale: number;
      type: "audio" | "video" | "other";
      defaultSampleDuration: number | undefined;
    }
  >,
): { time: number; duration: number | undefined; timescale: number } | null {
  const trafs = getTRAFs(buffer);
  if (trafs.length === 0) {
    return null;
  }

  const candidates = trafs
    .map((traf) => {
      const trackId = getTrackIdFromTFHDInTRAF(traf);
      const decodeTime = getTrackFragmentDecodeTimeFromTRAF(traf);
      if (trackId === undefined || decodeTime === undefined) {
        return null;
      }
      const initTrackInfo = initTrackInfoByTrackId.get(trackId);
      if (initTrackInfo === undefined) {
        return null;
      }
      const duration = getDurationFromTRAF(
        traf,
        initTrackInfo.defaultSampleDuration,
      );
      if (duration === undefined) {
        return null;
      }

      return {
        time: decodeTime,
        duration,
        timescale: initTrackInfo.timescale,
        trackType: initTrackInfo.type,
      };
    })
    .filter(isTrackTimeCandidate);

  const bestCandidate =
    candidates.find((candidate) => candidate.trackType === "video") ??
    candidates.find((candidate) => candidate.trackType === "audio") ??
    candidates[0];

  if (bestCandidate === undefined) {
    return null;
  }

  return {
    time: bestCandidate.time,
    duration: bestCandidate.duration,
    timescale: bestCandidate.timescale,
  };
}

function isTrackTimeCandidate(
  candidate: {
    time: number;
    duration: number;
    timescale: number;
    trackType: "audio" | "video" | "other";
  } | null,
): candidate is {
  time: number;
  duration: number;
  timescale: number;
  trackType: "audio" | "video" | "other";
} {
  return candidate !== null;
}

function getTrackTypeFromTRAK(trak: Uint8Array): "audio" | "video" | "other" {
  const mdia = getBoxContent(trak, 0x6d646961 /* mdia */);
  if (mdia === null) {
    return "other";
  }

  const hdlr = getBoxContent(mdia, 0x68646c72 /* hdlr */);
  if (hdlr === null || hdlr.length < 12) {
    return "other";
  }

  const handlerType = be4toi(hdlr, 8);
  if (handlerType === 0x76696465 /* vide */) {
    return "video";
  }
  if (handlerType === 0x736f756e /* soun */) {
    return "audio";
  }
  return "other";
}
/**
 * Extract RFC 6381 codec strings from the initialization metadata contained in
 * the given ISOBMFF data.
 *
 * Returns an empty array if no supported codec metadata could be parsed.
 * @param {Uint8Array} buffer
 * @returns {string[]}
 */
function getIsoBmffCodecs(buffer: Uint8Array): string[] {
  const moov = getBoxContent(buffer, 0x6d6f6f76 /* moov */);
  if (moov === null) {
    return [];
  }

  const traks = getBoxesContent(moov, 0x7472616b /* trak */);
  return traks.reduce((acc: string[], trak: Uint8Array) => {
    const codec = getTrackCodec(trak);
    if (codec !== undefined && !acc.includes(codec)) {
      acc.push(codec);
    }
    return acc;
  }, []);
}

function getTrackCodec(trak: Uint8Array): string | undefined {
  const mdia = getBoxContent(trak, 0x6d646961 /* mdia */);
  if (mdia === null) {
    return undefined;
  }

  const hdlr = getBoxContent(mdia, 0x68646c72 /* hdlr */);
  if (hdlr === null || hdlr.length < 12) {
    return undefined;
  }

  const minf = getBoxContent(mdia, 0x6d696e66 /* minf */);
  if (minf === null) {
    return undefined;
  }
  const stbl = getBoxContent(minf, 0x7374626c /* stbl */);
  if (stbl === null) {
    return undefined;
  }
  const stsd = getBoxContent(stbl, 0x73747364 /* stsd */);
  if (stsd === null) {
    return undefined;
  }

  const handlerType = be4toi(hdlr, 8);
  return getCodecFromStsd(stsd, handlerType);
}

function getCodecFromStsd(
  stsd: Uint8Array,
  handlerType: number,
): string | undefined {
  if (stsd.length < 16) {
    return undefined;
  }

  const entryCount = be4toi(stsd, 4);
  let cursor = 8;
  for (let i = 0; i < entryCount && cursor + 8 <= stsd.length; i++) {
    const [boxStart, contentStart, boxEnd, boxName] = getNextBox(stsd, cursor);
    if (
      boxStart === undefined ||
      contentStart === undefined ||
      boxEnd === undefined ||
      boxName === undefined
    ) {
      return undefined;
    }
    const sampleEntry = stsd.subarray(contentStart, boxEnd);
    const codec = getCodecFromSampleEntry(sampleEntry, boxName, handlerType);
    if (codec !== undefined) {
      return codec;
    }
    cursor = boxEnd;
  }
  return undefined;
}

function getCodecFromSampleEntry(
  sampleEntry: Uint8Array,
  boxName: number,
  handlerType: number,
): string | undefined {
  switch (boxName) {
    case 0x61766331: /* avc1 */
    case 0x61766333 /* avc3 */:
      return getAvcCodec(sampleEntry, boxName);
    case 0x6d703461 /* mp4a */:
      return getMp4aCodec(sampleEntry);
    case 0x656e6376: /* encv */
    case 0x656e6361 /* enca */:
      return getEncryptedCodec(sampleEntry, handlerType);
    case 0x68766331: /* hvc1 */
    case 0x68657631 /* hev1 */:
      return getHevcCodec(sampleEntry, boxName);
    case 0x76703038: /* vp08 */
    case 0x76703039 /* vp09 */:
      return getVpCodec(sampleEntry, boxName);
    case 0x61763031 /* av01 */:
      return getAv1Codec(sampleEntry);
    case 0x64766831: /* dvh1 */
    case 0x64766865: /* dvhe */
    case 0x64766131: /* dva1 */
    case 0x64766176: /* dvav */
    case 0x61763032 /* av02 */:
    case 0x61632d33: /* ac-3 */
    case 0x65632d33: /* ec-3 */
    case 0x61632d34: /* ac-4 */
    case 0x4f707573: /* Opus */
    case 0x664c6143: /* fLaC */
    case 0x616c6163: /* alac */
    case 0x77767474: /* wvtt */
    case 0x73747070 /* stpp */:
      return fourCC(boxName);
    default:
      return undefined;
  }
}

function getEncryptedCodec(
  sampleEntry: Uint8Array,
  handlerType: number,
): string | undefined {
  const boxes = sampleEntry.subarray(getSampleEntryHeaderSize(handlerType));
  const frma = getBoxContent(boxes, 0x66726d61 /* frma */);
  if (frma === null || frma.length < 4) {
    return undefined;
  }
  const originalFormat = be4toi(frma, 0);
  if (originalFormat === 0x656e6376 || originalFormat === 0x656e6361) {
    return undefined;
  }
  return getCodecFromSampleEntry(sampleEntry, originalFormat, handlerType);
}

function getSampleEntryHeaderSize(handlerType: number): number {
  switch (handlerType) {
    case 0x76696465 /* vide */:
      return 78;
    case 0x736f756e /* soun */:
      return 28;
    default:
      return 8;
  }
}

function getAvcCodec(
  sampleEntry: Uint8Array,
  boxName: number,
): string | undefined {
  if (sampleEntry.length < 78) {
    return undefined;
  }
  const avcC = getBoxContent(sampleEntry.subarray(78), 0x61766343 /* avcC */);
  if (avcC === null || avcC.length < 4) {
    return undefined;
  }
  return `${fourCC(boxName)}.${toHex(avcC[1])}${toHex(avcC[2])}${toHex(avcC[3])}`;
}

function getMp4aCodec(sampleEntry: Uint8Array): string | undefined {
  if (sampleEntry.length < 28) {
    return undefined;
  }
  const esds = getBoxContent(sampleEntry.subarray(28), 0x65736473 /* esds */);
  if (esds === null || esds.length < 2) {
    return undefined;
  }

  return getCodecFromEsdsDescriptors(esds.subarray(4));
}

function getHevcCodec(
  sampleEntry: Uint8Array,
  boxName: number,
): string | undefined {
  if (sampleEntry.length < 78) {
    return undefined;
  }
  const hvcC = getBoxContent(sampleEntry.subarray(78), 0x68766343 /* hvcC */);
  if (hvcC === null || hvcC.length < 13) {
    return undefined;
  }

  const profileByte = hvcC[1];
  const profileSpace = (profileByte >> 6) & 0x03;
  const tierFlag = (profileByte >> 5) & 0x01;
  const profileIdc = profileByte & 0x1f;
  const compatibilityFlags = reverseBits32(be4toi(hvcC, 2));
  const levelIdc = hvcC[12];
  const constraintBytes = trimTrailingZeroBytes(hvcC.subarray(6, 12));

  let profileSpacePrefix = "";
  if (profileSpace === 1) {
    profileSpacePrefix = "A";
  } else if (profileSpace === 2) {
    profileSpacePrefix = "B";
  } else if (profileSpace === 3) {
    profileSpacePrefix = "C";
  }
  const constraintString =
    constraintBytes.length === 0
      ? "0"
      : Array.from(constraintBytes, (value) =>
          value.toString(16).toUpperCase(),
        ).join(".");
  return `${fourCC(boxName)}.${profileSpacePrefix}${profileIdc}.${compatibilityFlags
    .toString(16)
    .toUpperCase()}.${tierFlag === 1 ? "H" : "L"}${levelIdc}.${constraintString}`;
}

function getVpCodec(
  sampleEntry: Uint8Array,
  boxName: number,
): string | undefined {
  if (sampleEntry.length < 78) {
    return undefined;
  }
  const vpcC = getBoxContent(sampleEntry.subarray(78), 0x76706343 /* vpcC */);
  if (vpcC === null || vpcC.length < 12) {
    return undefined;
  }

  const profile = vpcC[4];
  const level = vpcC[5];
  const packed = vpcC[6];
  const bitDepth = packed >> 4;
  const chromaSubsampling = (packed >> 1) & 0x07;
  const videoFullRangeFlag = packed & 0x01;
  const colourPrimaries = vpcC[7];
  const transferCharacteristics = vpcC[8];
  const matrixCoefficients = vpcC[9];

  return `${fourCC(boxName)}.${toDecimalString(profile)}.${toDecimalString(level)}.${toDecimalString(bitDepth)}.${toDecimalString(chromaSubsampling)}.${toDecimalString(colourPrimaries)}.${toDecimalString(transferCharacteristics)}.${toDecimalString(matrixCoefficients)}.${toDecimalString(videoFullRangeFlag)}`;
}

function getAv1Codec(sampleEntry: Uint8Array): string | undefined {
  if (sampleEntry.length < 78) {
    return undefined;
  }
  const boxes = sampleEntry.subarray(78);
  const av1C = getBoxContent(boxes, 0x61763143 /* av1C */);
  if (av1C === null || av1C.length < 4) {
    return undefined;
  }

  const profileAndLevel = av1C[1];
  const profile = (profileAndLevel >> 5) & 0x07;
  const level = profileAndLevel & 0x1f;
  const features = av1C[2];
  const tier = (features & 0x80) !== 0 ? "H" : "M";
  const highBitdepth = (features & 0x40) !== 0;
  const twelveBit = (features & 0x20) !== 0;
  let bitDepth = 8;
  if (highBitdepth) {
    bitDepth = twelveBit ? 12 : 10;
  }

  return `av01.${profile}.${toDecimalString(level)}${tier}.${toDecimalString(bitDepth)}`;
}

function getCodecFromEsdsDescriptors(
  data: Uint8Array,
  objectTypeIndication?: number,
): string | undefined {
  let cursor = 0;
  while (cursor + 2 <= data.length) {
    const tag = data[cursor];
    cursor += 1;
    const parsedLength = readDescriptorLength(data, cursor);
    if (parsedLength === null) {
      return undefined;
    }
    const [descriptorLength, descriptorHeaderLength] = parsedLength;
    cursor += descriptorHeaderLength;
    if (cursor + descriptorLength > data.length) {
      return undefined;
    }

    const descriptor = data.subarray(cursor, cursor + descriptorLength);
    let nextObjectTypeIndication = objectTypeIndication;
    if (tag === 0x04 /* DecoderConfigDescriptor */) {
      if (descriptor.length < 13) {
        return undefined;
      }
      nextObjectTypeIndication = descriptor[0];
    } else if (tag === 0x05 /* DecoderSpecificInfo */) {
      if (objectTypeIndication === undefined || descriptor.length === 0) {
        return undefined;
      }
      const audioObjectType = getAudioObjectType(descriptor);
      if (audioObjectType === undefined) {
        return undefined;
      }
      return `mp4a.${objectTypeIndication.toString(16)}.${audioObjectType}`;
    }

    const nestedDescriptor = getNestedEsdsDescriptors(
      tag,
      descriptor,
      nextObjectTypeIndication,
    );
    const nestedCodec =
      nestedDescriptor === undefined
        ? undefined
        : getCodecFromEsdsDescriptors(
            nestedDescriptor,
            nextObjectTypeIndication,
          );
    if (nestedCodec !== undefined) {
      return nestedCodec;
    }
    cursor += descriptorLength;
  }
  return undefined;
}

function getNestedEsdsDescriptors(
  tag: number,
  descriptor: Uint8Array,
  objectTypeIndication?: number,
): Uint8Array | undefined {
  switch (tag) {
    case 0x03: {
      if (descriptor.length < 3) {
        return undefined;
      }
      let cursor = 2; // ES_ID
      const flags = descriptor[cursor];
      cursor += 1;
      if ((flags & 0x80) !== 0) {
        cursor += 2;
      }
      if ((flags & 0x40) !== 0) {
        if (cursor >= descriptor.length) {
          return undefined;
        }
        const urlLength = descriptor[cursor];
        cursor += 1 + urlLength;
      }
      if ((flags & 0x20) !== 0) {
        cursor += 2;
      }
      return cursor <= descriptor.length
        ? descriptor.subarray(cursor)
        : undefined;
    }
    case 0x04:
      return descriptor.subarray(13);
    case 0x05:
      return undefined;
    default:
      if (objectTypeIndication === 0x40) {
        return descriptor;
      }
      return undefined;
  }
}

function getAudioObjectType(data: Uint8Array): number | undefined {
  if (data.length === 0) {
    return undefined;
  }
  let audioObjectType = data[0] >> 3;
  if (audioObjectType === 31) {
    if (data.length < 2) {
      return undefined;
    }
    audioObjectType = 32 + ((data[0] & 0x07) << 3) + (data[1] >> 5);
  }
  return audioObjectType;
}

function readDescriptorLength(
  data: Uint8Array,
  offset: number,
): [number, number] | null {
  let length = 0;
  let cursor = offset;
  let bytesRead = 0;
  while (cursor < data.length && bytesRead < 4) {
    const value = data[cursor];
    cursor += 1;
    bytesRead += 1;
    length = (length << 7) | (value & 0x7f);
    if ((value & 0x80) === 0) {
      return [length, bytesRead];
    }
  }
  return null;
}

function getNextBox(
  buf: Uint8Array,
  offset: number,
):
  | [
      number /* start byte */,
      number /* content start */,
      number /* end byte */,
      number /* box name */,
    ]
  | [] {
  if (offset + 8 > buf.length) {
    return [];
  }

  let cursor = offset;
  let boxSize = be4toi(buf, cursor);
  cursor += 4;
  const boxName = be4toi(buf, cursor);
  cursor += 4;

  if (boxSize === 0) {
    boxSize = buf.length - offset;
  } else if (boxSize === 1) {
    if (cursor + 8 > buf.length) {
      return [];
    }
    boxSize = be8toi(buf, cursor);
    cursor += 8;
  }

  if (boxSize < 8 || offset + boxSize > buf.length) {
    return [];
  }
  return [offset, cursor, offset + boxSize, boxName];
}

function toHex(value: number): string {
  return value.toString(16).padStart(2, "0");
}

function toDecimalString(value: number): string {
  return value.toString(10).padStart(2, "0");
}

function trimTrailingZeroBytes(buf: Uint8Array): Uint8Array {
  let end = buf.length;
  while (end > 0 && buf[end - 1] === 0) {
    end--;
  }
  return buf.subarray(0, end);
}

function reverseBits32(value: number): number {
  let reversed = 0;
  let current = value >>> 0;
  for (let i = 0; i < 32; i++) {
    reversed = (reversed << 1) | (current & 0x01);
    current >>>= 1;
  }
  return reversed >>> 0;
}

function fourCC(value: number): string {
  return String.fromCharCode(
    (value >>> 24) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 8) & 0xff,
    value & 0xff,
  );
}

/**
 * Returns the "default sample duration" which is the default value for duration
 * of samples found in a "traf" ISOBMFF box.
 *
 * Returns `undefined` if no "default sample duration" has been found.
 * @param {Uint8Array} traf
 * @returns {number|undefined}
 */
function getDefaultDurationFromTFHDInTRAF(
  traf: Uint8Array,
): number | undefined {
  const tfhd = getBoxContent(traf, 0x74666864 /* tfhd */);
  if (tfhd === null) {
    return undefined;
  }

  let cursor = /* version */ 1;

  const flags = be3toi(tfhd, cursor);
  cursor += 3;
  const hasBaseDataOffset = (flags & 0x000001) > 0;
  const hasSampleDescriptionIndex = (flags & 0x000002) > 0;
  const hasDefaultSampleDuration = (flags & 0x000008) > 0;

  if (!hasDefaultSampleDuration) {
    return undefined;
  }
  cursor += 4;

  if (hasBaseDataOffset) {
    cursor += 8;
  }

  if (hasSampleDescriptionIndex) {
    cursor += 4;
  }

  const defaultDuration = be4toi(tfhd, cursor);
  return defaultDuration;
}

function getDefaultSampleDurationFromTREXByTrackId(
  moov: Uint8Array,
): Map<number, number> {
  const mvex = getBoxContent(moov, 0x6d766578 /* mvex */);
  if (mvex === null) {
    return new Map();
  }

  const trexBoxes = getBoxesContent(mvex, 0x74726578 /* trex */);
  const defaultDurations = new Map<number, number>();
  for (const trex of trexBoxes) {
    if (trex.length < 24) {
      continue;
    }
    const trackId = be4toi(trex, 4);
    const defaultSampleDuration = be4toi(trex, 12);
    defaultDurations.set(trackId, defaultSampleDuration);
  }
  return defaultDurations;
}

function getDurationFromTRAF(
  traf: Uint8Array,
  defaultSampleDurationFromInit: number | undefined,
): number | undefined {
  const truns = getBoxesContent(traf, 0x7472756e /* trun */);
  if (truns.length === 0) {
    return undefined;
  }

  let totalDuration = 0;
  for (const trun of truns) {
    const duration = getDurationFromSingleTrun(
      traf,
      trun,
      defaultSampleDurationFromInit,
    );
    if (duration === undefined) {
      return undefined;
    }
    totalDuration += duration;
  }
  return totalDuration;
}

function getDurationFromSingleTrun(
  traf: Uint8Array,
  trun: Uint8Array,
  defaultSampleDurationFromInit: number | undefined,
): number | undefined {
  let cursor = 0;
  const version = trun[cursor];
  cursor += 1;
  if (version > 1) {
    return undefined;
  }

  const flags = be3toi(trun, cursor);
  cursor += 3;
  const hasSampleDuration = (flags & 0x000100) > 0;

  let defaultDuration: number | undefined = 0;
  if (!hasSampleDuration) {
    defaultDuration = getDefaultDurationFromTFHDInTRAF(traf);
    if (defaultDuration === undefined) {
      defaultDuration = defaultSampleDurationFromInit;
    }
    if (defaultDuration === undefined) {
      return undefined;
    }
  }

  const hasDataOffset = (flags & 0x000001) > 0;
  const hasFirstSampleFlags = (flags & 0x000004) > 0;
  const hasSampleSize = (flags & 0x000200) > 0;
  const hasSampleFlags = (flags & 0x000400) > 0;
  const hasSampleCompositionOffset = (flags & 0x000800) > 0;

  const sampleCounts = be4toi(trun, cursor);
  cursor += 4;

  if (hasDataOffset) {
    cursor += 4;
  }
  if (hasFirstSampleFlags) {
    cursor += 4;
  }

  let i = sampleCounts;
  let duration = 0;
  while (i-- > 0) {
    if (hasSampleDuration) {
      duration += be4toi(trun, cursor);
      cursor += 4;
    } else {
      duration += defaultDuration;
    }
    if (hasSampleSize) {
      cursor += 4;
    }
    if (hasSampleFlags) {
      cursor += 4;
    }
    if (hasSampleCompositionOffset) {
      cursor += 4;
    }
  }

  return duration;
}

/**
 * Returns the content of all "traf" boxes encountered in the given ISOBMFF
 * data.
 * Might be preferred to just `getTRAF` if you suspect that your ISOBMFF may
 * have multiple "moof" boxes.
 * @param {Uint8Array} buffer
 * @returns {Array.<Uint8Array>}
 */
function getTRAFs(buffer: Uint8Array): Uint8Array[] {
  const moofs = getBoxesContent(buffer, 0x6d6f6f66 /* moof */);
  return moofs.reduce((acc: Uint8Array[], moof: Uint8Array) => {
    acc.push(...getBoxesContent(moof, 0x74726166 /* traf */));
    return acc;
  }, []);
}

function getTrackIdFromTRAK(trak: Uint8Array): number | undefined {
  const tkhd = getBoxContent(trak, 0x746b6864 /* tkhd */);
  if (tkhd === null) {
    return undefined;
  }
  const version = tkhd[0];
  if (version === 1) {
    return be4toi(tkhd, 20);
  }
  if (version === 0) {
    return be4toi(tkhd, 12);
  }
  return undefined;
}

function getTimescaleFromMDIA(mdia: Uint8Array): number | undefined {
  const mdhd = getBoxContent(mdia, 0x6d646864 /* mdhd */);
  if (mdhd === null) {
    return undefined;
  }

  let cursor = 0;
  const version = mdhd[cursor];
  cursor += 4;
  if (version === 1) {
    return be4toi(mdhd, cursor + 16);
  }
  if (version === 0) {
    return be4toi(mdhd, cursor + 8);
  }
  return undefined;
}

function getTrackIdFromTFHDInTRAF(traf: Uint8Array): number | undefined {
  const tfhd = getBoxContent(traf, 0x74666864 /* tfhd */);
  if (tfhd === null || tfhd.length < 8) {
    return undefined;
  }
  return be4toi(tfhd, 4);
}

function getTrackFragmentDecodeTimeFromTRAF(
  traf: Uint8Array,
): number | undefined {
  const tfdt = getBoxContent(traf, 0x74666474 /* tfdt */);
  if (tfdt === null) {
    return undefined;
  }
  const version = tfdt[0];
  if (version === 1) {
    return be8toi(tfdt, 4);
  }
  if (version === 0) {
    return be4toi(tfdt, 4);
  }
  return undefined;
}

/**
 * Returns the content of a box based on its name.
 * `null` if not found.
 * @param {Uint8Array} buf - the isobmff data
 * @param {Number} boxName - the 4-letter 'name' of the box as a 4 byte integer
 * generated from encoding the corresponding ASCII in big endian.
 * @returns {UInt8Array|null}
 */
function getBoxContent(buf: Uint8Array, boxName: number): Uint8Array | null {
  const offsets = getBoxOffsets(buf, boxName);
  return offsets !== null ? buf.subarray(offsets[1], offsets[2]) : null;
}

/**
 * Reads the whole ISOBMFF and returns the content of all boxes with the given
 * name, in order.
 * @param {Uint8Array} buf - the isobmff data
 * @param {Number} boxName - the 4-letter 'name' of the box as a 4 byte integer
 * generated from encoding the corresponding ASCII in big endian.
 * @returns {Array.<Uint8Array>}
 */
function getBoxesContent(buf: Uint8Array, boxName: number): Uint8Array[] {
  const ret = [];
  let currentBuf = buf;
  while (true) {
    const offsets = getBoxOffsets(currentBuf, boxName);
    if (offsets === null) {
      return ret;
    }

    // Guard against a (very highly improbable) infinite loop
    if (offsets[2] === 0 || currentBuf.length === 0) {
      throw new Error("Error while parsing ISOBMFF box");
    }

    ret.push(currentBuf.subarray(offsets[1], offsets[2]));
    currentBuf = currentBuf.subarray(offsets[2]);
  }
}

/**
 * Returns byte offsets for the start of the box, the start of its content and
 * the end of the box (not inclusive).
 *
 * `null` if not found.
 *
 * If found, the tuple returned has three elements, all numbers:
 *   1. The starting byte corresponding to the start of the box (from its size)
 *   2. The beginning of the box content - meaning the first byte after the
 *      size and the name of the box.
 *   3. The first byte after the end of the box, might be equal to `buf`'s
 *      length if we're considering the last box.
 * @param {Uint8Array} buf - the isobmff data
 * @param {Number} boxName - the 4-letter 'name' of the box as a 4 byte integer
 * generated from encoding the corresponding ASCII in big endian.
 * @returns {Array.<number>|null}
 */
function getBoxOffsets(
  buf: Uint8Array,
  boxName: number,
):
  | [
      number /* start byte */,
      number /* First byte after the size and name (where the content begins)*/,
      number /* end byte, not included. */,
    ]
  | null {
  const len = buf.length;

  let boxBaseOffset = 0;
  let name: number;
  let lastBoxSize: number = 0;
  let lastOffset;
  while (boxBaseOffset + 8 <= len) {
    lastOffset = boxBaseOffset;
    lastBoxSize = be4toi(buf, lastOffset);
    lastOffset += 4;

    name = be4toi(buf, lastOffset);
    lastOffset += 4;

    if (lastBoxSize === 0) {
      lastBoxSize = len - boxBaseOffset;
    } else if (lastBoxSize === 1) {
      if (lastOffset + 8 > len) {
        return null;
      }
      lastBoxSize = be8toi(buf, lastOffset);
      lastOffset += 8;
    }

    if (lastBoxSize < 0) {
      throw new Error("ISOBMFF: Size out of range");
    }
    if (name === boxName) {
      if (boxName === 0x75756964 /* === "uuid" */) {
        lastOffset += 16; // Skip uuid name
      }
      return [boxBaseOffset, lastOffset, boxBaseOffset + lastBoxSize];
    } else {
      boxBaseOffset += lastBoxSize;
    }
  }
  return null;
}

/**
 * Translate groups of 3 big-endian bytes to Integer.
 * @param {Uint8Array} bytes
 * @param {Number} offset - The offset (from the start of the given array)
 * @returns {Number}
 */
function be3toi(bytes: Uint8Array, offset: number): number {
  return (
    bytes[offset + 0] * 0x0010000 +
    bytes[offset + 1] * 0x0000100 +
    bytes[offset + 2]
  );
}

/**
 * Translate groups of 4 big-endian bytes to Integer.
 * @param {Uint8Array} bytes
 * @param {Number} offset - The offset (from the start of the given array)
 * @returns {Number}
 */
function be4toi(bytes: Uint8Array, offset: number): number {
  return (
    bytes[offset + 0] * 0x1000000 +
    bytes[offset + 1] * 0x0010000 +
    bytes[offset + 2] * 0x0000100 +
    bytes[offset + 3]
  );
}

/**
 * Translate groups of 8 big-endian bytes to Integer.
 * @param {Uint8Array} bytes
 * @param {Number} offset - The offset (from the start of the given array)
 * @returns {Number}
 */
function be8toi(bytes: Uint8Array, offset: number): number {
  return (
    (bytes[offset + 0] * 0x1000000 +
      bytes[offset + 1] * 0x0010000 +
      bytes[offset + 2] * 0x0000100 +
      bytes[offset + 3]) *
      0x100000000 +
    bytes[offset + 4] * 0x1000000 +
    bytes[offset + 5] * 0x0010000 +
    bytes[offset + 6] * 0x0000100 +
    bytes[offset + 7]
  );
}

export {
  getDurationFromTrun,
  getInitTrackInfo,
  getIsobmfTimeInfo,
  getTrackFragmentDecodeTime,
  getMDHDTimescale,
  getIsoBmffCodecs,
};
