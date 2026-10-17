import type { MediaType } from "./enums.js";
import type { WaspWasmExports } from "./generatedTypes.js";
export type { HostBindings, WaspWasmExports } from "./generatedTypes.js";

export interface ISafeU64 {
  hi: number;
  lo: number;
}

export interface AppendBufferValue {
  start: ISafeU64 | undefined;
  end: ISafeU64 | undefined;
  timescale: number | undefined;
}

/** Supplementary segment information communicated when pushing one to a buffer. */
export interface SegmentHints {
  /**
   * High 4 bytes for the hinted `baseMediaDecodeTime`.
   *
   * This might be used in specific scenario where the segment is not explicit
   * enough about timestamp.
   */
  baseDecodeTimeStartHi: number;
  /**
   * Low 4 bytes for the hinted `baseMediaDecodeTime`.
   *
   * This might be used in specific scenario where the segment is not explicit
   * enough about timestamp.
   */
  baseDecodeTimeStartLo: number;

  /** Timescale used for the "baseDecodeTimeStart" (combination of hi+lo). */
  baseDecodeTimeStartTimescale: number;
  /**
   * Some state may be maintained by the potential lower-level transmuxer we'll
   * feed data too.
   * That state makes sense as we're pushing contiguous segments but does not
   * make sense to be maintained once either a track switch or a new init
   * segment is pushed.
   */
  resetTransmuxerState: boolean;
}

export interface InspectSegmentValue {
  codec: string;
  mimeType: string;
  mediaType: MediaType;
}

export interface HostResult<Value, ErrorCode extends number> {
  value: Value | undefined;
  errorCode: ErrorCode | undefined;
  description: string | undefined;
}

export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export type InitOutput = WaspWasmExports;

export type InitializeWasmArg =
  | { module_or_path: InitInput | Promise<InitInput> }
  | InitInput
  | Promise<InitInput>;
