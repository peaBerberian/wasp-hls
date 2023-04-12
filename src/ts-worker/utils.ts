import { cachedCodecsSupport } from "./globals";

export function formatErrMessage(err: unknown, defaultMsg: string) {
  return err instanceof Error ? err.name + ": " + err.message : defaultMsg;
}

const MPEG_TS_REGEXP = /^[a-z]+\/mp2t;/i;
const AAC_REGEXP = /^audio\/aac;/i;

export function isMpegTsType(typ: string): boolean {
  return MPEG_TS_REGEXP.test(typ);
}

export function isAacType(typ: string): boolean {
  return AAC_REGEXP.test(typ);
}

export function shouldTransmux(typ: string) {
  if (!canTransmux(typ)) {
    return false;
  }
  if (typeof MediaSource === "undefined") {
    return cachedCodecsSupport.get(typ) !== true;
  }
  return !MediaSource.isTypeSupported(typ);
}

export function canTransmux(typ: string): boolean {
  return isMpegTsType(typ) || isAacType(typ);
}
