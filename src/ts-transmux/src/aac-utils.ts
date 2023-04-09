/**
 * @param {Uint8Array} header
 * @param {number} byteIndex
 * @returns {number}
 */
function parseId3TagSize(header: Uint8Array, byteIndex: number): number {
  let returnSize =
    (header[byteIndex + 6] << 21) |
    (header[byteIndex + 7] << 14) |
    (header[byteIndex + 8] << 7) |
    header[byteIndex + 9];
  const flags = header[byteIndex + 5];
  const footerPresent = (flags & 16) >> 4;

  // if we get a negative returnSize clamp it to 0
  returnSize = returnSize >= 0 ? returnSize : 0;

  if (footerPresent) {
    return returnSize + 20;
  }
  return returnSize + 10;
}

/**
 * @param {Uint8Array} data
 * @param {number} initialOffset
 * @returns {number}
 */
function getId3Offset(data: Uint8Array, initialOffset: number): number {
  if (
    data.length - initialOffset < 10 ||
    data[initialOffset] !== "I".charCodeAt(0) ||
    data[initialOffset + 1] !== "D".charCodeAt(0) ||
    data[initialOffset + 2] !== "3".charCodeAt(0)
  ) {
    return initialOffset;
  }

  const offset = initialOffset + parseId3TagSize(data, initialOffset);

  return getId3Offset(data, offset);
}

/**
 * Returns `true` if the given data appears to be AAC data.
 * @param {Uint8Array} data
 * @returns {boolean}
 */
function isLikelyAacData(data: Uint8Array): boolean {
  const offset = getId3Offset(data, 0);

  return (
    data.length >= offset + 2 &&
    (data[offset] & 0xff) === 0xff &&
    (data[offset + 1] & 0xf0) === 0xf0 &&
    // verify that the 2 layer bits are 0, aka this
    // is not mp3 data but aac data.
    (data[offset + 1] & 0x16) === 0x10
  );
}

function parseAdtsSize(header: Uint8Array, byteIndex: number): number {
  const lowThree = (header[byteIndex + 5] & 0xe0) >> 5;
  const middle = header[byteIndex + 4] << 3;
  const highTwo = header[byteIndex + 3] & (0x3 << 11);
  return highTwo | middle | lowThree;
}

export {
  isLikelyAacData,
  parseId3TagSize,
  parseAdtsSize,
  // parseType,
  // parseSampleRate,
  // parseAacTimestamp,
};
