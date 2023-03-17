export function formatErrMessage(err: unknown, defaultMsg: string) {
  return err instanceof Error ? err.name + ": " + err.message : defaultMsg;
}

const MPEG_TS_REGEXP = /^[a-z]+\/mp2t;/i;
export function isMpegTsType(typ: string): boolean {
  return MPEG_TS_REGEXP.test(typ);
}

export function shouldTransmux(typ: string) {
  if (!canTransmux(typ)) {
    return false;
  }
  if (typeof MediaSource === "undefined") {
    // TODO truly test support?
    return true;
  }
  return !MediaSource.isTypeSupported(typ);
}

export function canTransmux(typ: string): boolean {
  /* eslint-disable @typescript-eslint/no-explicit-any */
  /* eslint-disable @typescript-eslint/no-unsafe-member-access */
  /* eslint-disable @typescript-eslint/no-unsafe-return */
  return isMpegTsType(typ) && (globalThis as any).hasTransmuxer;
  /* eslint-enable @typescript-eslint/no-explicit-any */
  /* eslint-enable @typescript-eslint/no-unsafe-member-access */
  /* eslint-enable @typescript-eslint/no-unsafe-return */
}
