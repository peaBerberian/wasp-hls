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
  const traf = getTRAF(buffer);
  if (traf === null) {
    return undefined;
  }
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
    const trun = getBoxContent(traf, 0x7472756e /* trun */);
    if (trun === null) {
      return undefined;
    }
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

    completeDuration += duration;
  }
  return completeDuration;
}

/**
 * Get timescale information from a movie header box. Found in init segments.
 * `undefined` if not found or not parsed.
 *
 * This timescale is the default timescale used for segments.
 * @param {Uint8Array} buffer
 * @returns {Number | undefined}
 */
function getMDHDTimescale(buffer: Uint8Array): number | undefined {
  const mdia = getMDIA(buffer);
  if (mdia === null) {
    return undefined;
  }

  const mdhd = getBoxContent(mdia, 0x6d646864 /* "mdhd" */);
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

/**
 * Returns the content of the first "traf" box encountered in the given ISOBMFF
 * data.
 * Returns null if not found.
 * @param {Uint8Array} buffer
 * @returns {Uint8Array|null}
 */
function getTRAF(buffer: Uint8Array): Uint8Array | null {
  const moof = getBoxContent(buffer, 0x6d6f6f66 /* moof */);
  if (moof === null) {
    return null;
  }
  return getBoxContent(moof, 0x74726166 /* traf */);
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
    const traf = getBoxContent(moof, 0x74726166 /* traf */);
    if (traf !== null) {
      acc.push(traf);
    }
    return acc;
  }, []);
}

/**
 * Returns the content of the first "mdia" box encountered in the given ISOBMFF
 * data.
 * Returns null if not found.
 * @param {Uint8Array} buf
 * @returns {Uint8Array|null}
 */
function getMDIA(buf: Uint8Array): Uint8Array | null {
  const moov = getBoxContent(buf, 0x6d6f6f76 /* moov */);
  if (moov === null) {
    return null;
  }

  const trak = getBoxContent(moov, 0x7472616b /* "trak" */);
  if (trak === null) {
    return null;
  }

  return getBoxContent(trak, 0x6d646961 /* "mdia" */);
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

export { getDurationFromTrun, getTrackFragmentDecodeTime, getMDHDTimescale };
