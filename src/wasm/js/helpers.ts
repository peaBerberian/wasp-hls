import { getUint32Memory, writeOptionalString } from "./memory.js";
import type { HostResult, ISafeU64 } from "./types.js";

// TODO: Maybe more safety into that one
const OPTIONAL_ID_NONE = 0xffffffff;

export function unwrapResult<Value, ErrorCode extends number>(
  result: HostResult<Value, ErrorCode>,
  okValue: number,
  errCodeOut: number,
  errDescPtrOut: number,
  errDescLenOut: number,
): number {
  if (result.errorCode === undefined) {
    return okValue;
  }
  getUint32Memory()[errCodeOut >>> 2] = result.errorCode;
  writeOptionalString(result.description, errDescPtrOut, errDescLenOut);
  return 0;
}

export function writeAppendBufferResult(
  result: HostResult<
    {
      start: ISafeU64 | undefined;
      end: ISafeU64 | undefined;
      timescale: number | undefined;
    },
    number
  >,
  hasStartOut: number,
  startValueHiOut: number,
  startValueLoOut: number,
  hasEndOut: number,
  endValueHiOut: number,
  endValueLoOut: number,
  timescale: number,
  errCodeOut: number,
  errDescPtrOut: number,
  errDescLenOut: number,
): number {
  if (result.errorCode !== undefined) {
    getUint32Memory()[errCodeOut >>> 2] = result.errorCode;
    writeOptionalString(result.description, errDescPtrOut, errDescLenOut);
    return 0;
  }
  const parsed = result.value;
  getUint32Memory()[hasStartOut >>> 2] = parsed?.start == null ? 0 : 1;
  getUint32Memory()[startValueHiOut >>> 2] = parsed?.start?.hi ?? 0;
  getUint32Memory()[startValueLoOut >>> 2] = parsed?.start?.lo ?? 0;
  getUint32Memory()[hasEndOut >>> 2] = parsed?.end == null ? 0 : 1;
  getUint32Memory()[endValueHiOut >>> 2] = parsed?.end?.hi ?? 0;
  getUint32Memory()[endValueLoOut >>> 2] = parsed?.end?.lo ?? 0;
  getUint32Memory()[timescale >>> 2] = parsed?.timescale ?? 1;
  return 1;
}

export function optionalIdToRaw(value: number | undefined): number {
  return value ?? OPTIONAL_ID_NONE;
}

export function rawOptionalId(value: number): number | undefined {
  // WebAssembly passes i32 imports as signed numbers, including the u32 sentinel.
  value = value >>> 0;
  return value === OPTIONAL_ID_NONE ? undefined : value;
}
