/** Media bytes are borrowed until synchronous segment assembly completes. */
export interface Mp4Fragment {
  readonly moof: Uint8Array;
  readonly payload: readonly Uint8Array[];
  /** Write a four-byte, big-endian NAL length before each payload part. */
  readonly lengthPrefixed: boolean;
  readonly byteLength: number;
}

/** Describe a fragment without copying its AAC frames or AVC NAL units. */
export function createFragment(
  moof: Uint8Array,
  payload: readonly Uint8Array[],
  lengthPrefixed: boolean,
): Mp4Fragment {
  const payloadLength = payload.reduce((sum, part) => sum + part.byteLength, 0);
  return {
    moof,
    payload,
    lengthPrefixed,
    byteLength:
      moof.byteLength +
      8 +
      payloadLength +
      (lengthPrefixed ? 4 * payload.length : 0),
  };
}

/** Allocate media-sized storage once; write every payload directly to its destination. */
export function assembleSegment(
  init: Uint8Array,
  fragments: Mp4Fragment[],
): Uint8Array {
  const size =
    init.byteLength +
    fragments.reduce((sum, fragment) => sum + fragment.byteLength, 0);
  const output = new Uint8Array(size);
  const view = new DataView(output.buffer);
  output.set(init);
  let offset = init.byteLength;
  for (const fragment of fragments) {
    output.set(fragment.moof, offset);
    offset += fragment.moof.byteLength;
    view.setUint32(offset, fragment.byteLength - fragment.moof.byteLength);
    view.setUint32(offset + 4, 0x6d646174); // mdat
    offset += 8;
    for (const part of fragment.payload) {
      if (fragment.lengthPrefixed) {
        view.setUint32(offset, part.byteLength);
        offset += 4;
      }
      output.set(part, offset);
      offset += part.byteLength;
    }
  }
  return output;
}
