import {
  getDurationFromTrun,
  getTrackFragmentDecodeTime,
} from "../ts-common/isobmff-utils.js";

export function getTimeInformationFromMp4(
  segment: Uint8Array,
  initTimescale: number
) : ({ time: number; duration: number | undefined }) | null {
  const baseDecodeTime = getTrackFragmentDecodeTime(segment);
  if (baseDecodeTime === undefined) {
    return null;
  }
  const trunDuration = getDurationFromTrun(segment);
  return {
    time: baseDecodeTime / initTimescale,
    duration: trunDuration === undefined ?
      undefined :
      trunDuration / initTimescale,
  };
}
