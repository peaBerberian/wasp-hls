import type {
  TransmuxedSegmentData,
  TransmuxSegmentOptions,
} from "../../../src/ts-transmux/src/Transmuxer.ts";
import {
  allocBytes,
  freeBytes,
  getUint8Memory,
  getWasmExports,
  readString,
} from "../../../src/wasm/js/memory.js";

// This interface belongs to the test-only WASM artifact. Production builds
// export no transmuxer handles, and playback never enters this wrapper.
interface TestExports {
  wasp_transmuxer_new(keepOriginal: number): number;
  wasp_transmuxer_free(pointer: number): void;
  wasp_transmuxer_release(pointer: number): void;
  wasp_transmuxer_run(
    session: number,
    input: number,
    length: number,
    reset: number,
    hasSeed: number,
    seed: number,
    timescale: number,
  ): number;
}
function getTestExports() {
  return getWasmExports() as ReturnType<typeof getWasmExports> & TestExports;
}

const MAX_INPUT_BYTES = 64 * 1024 * 1024;

/**
 * Owns one Rust transmuxer. Media bytes cross the WASM boundary twice: input
 * into linear memory, output into a standalone JS buffer. The output copy is
 * needed for asynchronous MSE use and transfer to the main thread: WASM memory
 * cannot be transferred and may grow or be reused as soon as this call ends.
 */
export class RustTransmuxer {
  private _pointer: number;

  constructor(options: { keepOriginalTimestamps?: boolean } = {}) {
    this._pointer = getTestExports().wasp_transmuxer_new(
      options.keepOriginalTimestamps === true ? 1 : 0,
    );
  }

  public transmuxSegment(
    input: Uint8Array,
    options: TransmuxSegmentOptions = {},
  ): TransmuxedSegmentData | null {
    if (this._pointer === 0) {
      throw new Error("Transmuxer has been disposed");
    }
    if (input.byteLength > MAX_INPUT_BYTES) {
      throw new Error("Segment exceeds the 64 MiB transmux input limit");
    }
    return transmuxWithSession(this._pointer, input, options);
  }

  public dispose(): void {
    if (this._pointer !== 0) {
      getTestExports().wasp_transmuxer_free(this._pointer);
      this._pointer = 0;
    }
  }
}

/**
 * Run a test-owned session. This wrapper measures the same input/output copies
 * as playback, while keeping test handles outside the production ABI.
 */
export function transmuxWithSession(
  session: number,
  input: Uint8Array,
  options: TransmuxSegmentOptions = {},
): TransmuxedSegmentData | null {
  if (session === 0) throw new Error("Missing Rust transmux session");
  if (input.byteLength > MAX_INPUT_BYTES) {
    throw new Error("Segment exceeds the 64 MiB transmux input limit");
  }
  const seed = options.baseMediaDecodeTimeSeed;
  if (
    seed !== undefined &&
    (!Number.isFinite(seed.value) ||
      seed.value < 0 ||
      !Number.isInteger(seed.timescale) ||
      seed.timescale <= 0 ||
      seed.timescale > 0xffffffff)
  ) {
    throw new Error("Invalid decode-time seed");
  }
  const wasm = getTestExports();
  const inputLength = input.byteLength;
  const inputInWasm = input.buffer === wasm.memory.buffer;
  const inputOffset = input.byteOffset;
  const pointer = allocBytes(inputLength);
  try {
    // Refresh the view after allocation and again after running Rust. Either
    // may grow memory; an earlier view would then refer to a detached buffer.
    getUint8Memory().set(
      inputInWasm
        ? getUint8Memory().subarray(inputOffset, inputOffset + inputLength)
        : input,
      pointer,
    );
    const result = wasm.wasp_transmuxer_run(
      session,
      pointer,
      inputLength,
      options.reset === true ? 1 : 0,
      seed === undefined ? 0 : 1,
      seed?.value ?? 0,
      seed?.timescale ?? 1,
    );
    const descriptor = new DataView(wasm.memory.buffer, result, 40);
    const status = descriptor.getUint32(28, true);
    if (status === 1) {
      return null;
    }
    if (status !== 0) {
      const message = readString(
        descriptor.getUint32(32, true),
        descriptor.getUint32(36, true),
      );
      throw new Error(`Rust transmux: ${message}`);
    }
    const dataPointer = descriptor.getUint32(0, true);
    const length = descriptor.getUint32(4, true);
    return {
      data: getUint8Memory().slice(dataPointer, dataPointer + length),
      timingInfo: {
        start: descriptor.getFloat64(8, true),
        end: descriptor.getFloat64(16, true),
        timescale: descriptor.getUint32(24, true),
      },
    };
  } finally {
    // Both sides release large allocations before returning to the caller.
    // The allocator can reuse their pages for subsequent segments or tracks.
    wasm.wasp_transmuxer_release(session);
    freeBytes(pointer, inputLength);
  }
}
