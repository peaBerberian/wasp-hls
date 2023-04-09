const HIGH_PREFIX = [33, 16, 5, 32, 164, 27];
const LOW_PREFIX = [33, 65, 108, 84, 1, 2, 4, 8, 168, 2, 4, 8, 17, 191, 252];

/**
 * Creates an array of length `len`, filled with `0`.
 */
function zeroedArray(len: number): number[] {
  const arr: number[] = [];
  for (let i = 0; i < len; i++) {
    arr.push(0);
  }
  return arr;
}

let memoizedSilence: Record<number, Uint8Array> | undefined;
export default function generateFrameOfSilence(): Record<number, Uint8Array> {
  if (!memoizedSilence) {
    // Frames-of-memoizedSilence to use for filling in missing AAC frames
    /* eslint-disable @typescript-eslint/naming-convention */
    memoizedSilence = {
      96000: new Uint8Array([...HIGH_PREFIX, 227, 64, ...zeroedArray(154), 56]),
      88200: new Uint8Array([...HIGH_PREFIX, 231, ...zeroedArray(170), 56]),
      64000: new Uint8Array([
        ...HIGH_PREFIX,
        248,
        192,
        ...zeroedArray(240),
        56,
      ]),
      48000: new Uint8Array([
        ...HIGH_PREFIX,
        255,
        192,
        ...zeroedArray(268),
        55,
        148,
        128,
        ...zeroedArray(54),
        112,
      ]),
      44100: new Uint8Array([
        ...HIGH_PREFIX,
        255,
        192,
        ...zeroedArray(268),
        55,
        163,
        128,
        ...zeroedArray(84),
        112,
      ]),
      32000: new Uint8Array([
        ...HIGH_PREFIX,
        255,
        192,
        ...zeroedArray(268),
        55,
        234,
        ...zeroedArray(226),
        112,
      ]),
      24000: new Uint8Array([
        ...HIGH_PREFIX,
        255,
        192,
        ...zeroedArray(268),
        55,
        255,
        128,
        ...zeroedArray(268),
        111,
        112,
        ...zeroedArray(126),
        224,
      ]),
      16000: new Uint8Array([
        ...HIGH_PREFIX,
        255,
        192,
        ...zeroedArray(268),
        55,
        255,
        128,
        ...zeroedArray(268),
        111,
        255,
        ...zeroedArray(269),
        223,
        108,
        ...zeroedArray(195),
        1,
        192,
      ]),
      12000: new Uint8Array([
        ...LOW_PREFIX,
        ...zeroedArray(268),
        3,
        127,
        248,
        ...zeroedArray(268),
        6,
        255,
        240,
        ...zeroedArray(268),
        13,
        255,
        224,
        ...zeroedArray(268),
        27,
        253,
        128,
        ...zeroedArray(259),
        56,
      ]),
      11025: new Uint8Array([
        ...LOW_PREFIX,
        ...zeroedArray(268),
        3,
        127,
        248,
        ...zeroedArray(268),
        6,
        255,
        240,
        ...zeroedArray(268),
        13,
        255,
        224,
        ...zeroedArray(268),
        27,
        255,
        192,
        ...zeroedArray(268),
        55,
        175,
        128,
        ...zeroedArray(108),
        112,
      ]),
      8000: new Uint8Array([
        ...LOW_PREFIX,
        ...zeroedArray(268),
        3,
        121,
        16,
        ...zeroedArray(47),
        7,
      ]),
    };
    /* eslint-enable @typescript-eslint/naming-convention */
  }
  return memoizedSilence;
}
