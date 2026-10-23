import type { MediaType } from "./enums.js";
import type { WaspWasmExports } from "./generatedTypes.js";
export type { HostBindings, WaspWasmExports } from "./generatedTypes.js";

export interface ISafeU64 {
  hi: number;
  lo: number;
}

export interface AddSourceBufferValue {
  id: number;
  /** Host decision, including native AAC support on this browser. */
  needsTransmux: boolean;
}

export interface AppendBufferValue {
  start: ISafeU64 | undefined;
  end: ISafeU64 | undefined;
  timescale: number | undefined;
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
  RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export type InitOutput = WaspWasmExports;

export type InitializeWasmArg =
  | { module_or_path: InitInput | Promise<InitInput> }
  | InitInput
  | Promise<InitInput>;
