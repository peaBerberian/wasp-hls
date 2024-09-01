import {
  ONE_SECOND_IN_TS,
  audioTsToVideoTs,
  videoTsToAudioTs,
} from "./clock-utils";
import generateFrameOfSilence from "./generate_frame_of_silence";
import type { TrackInfo } from "./types";

/**
 * Sum the `byteLength` properties of the data in each AAC frame
 * @param {Array.<Object>} array
 * @returns {number}
 */
function sumFrameByteLengths(array: Array<{ data: Uint8Array }>): number {
  return array.reduce((acc, arr) => acc + arr.data.byteLength, 0);
}

export interface FrameData {
  data: Uint8Array;
  dts: number;
  pts: number;
}

/**
 * Possibly pad (prefix) the audio track with silence if appending this track
 * would lead to the introduction of a gap in the audio buffer
 * @param {Object} track
 * @param {Array.<Object>} frames
 * @param {number} audioAppendStartTs
 * @param {number} videoBaseMediaDecodeTime
 * @returns {number|undefined}
 */
function prefixWithSilence(
  track: TrackInfo,
  frames: FrameData[],
  audioAppendStartTs: number,
  videoBaseMediaDecodeTime: number,
): number | undefined {
  let audioGapDuration = 0;
  let audioFillFrameCount = 0;
  let audioFillDuration = 0;

  if (!frames.length) {
    return undefined;
  }

  const baseMediaDecodeTimeTs = audioTsToVideoTs(
    track.baseMediaDecodeTime,
    track.samplerate,
  );
  // determine frame clock duration based on sample rate, round up to avoid overfills
  const frameDuration = Math.ceil(ONE_SECOND_IN_TS / (track.samplerate / 1024));

  if (audioAppendStartTs && videoBaseMediaDecodeTime) {
    // insert the shortest possible amount (audio gap or audio to video gap)
    audioGapDuration =
      baseMediaDecodeTimeTs -
      Math.max(audioAppendStartTs, videoBaseMediaDecodeTime);
    // number of full frames in the audio gap
    audioFillFrameCount = Math.floor(audioGapDuration / frameDuration);
    audioFillDuration = audioFillFrameCount * frameDuration;
  }

  // don't attempt to fill gaps smaller than a single frame or larger
  // than a half second
  if (audioFillFrameCount < 1 || audioFillDuration > ONE_SECOND_IN_TS / 2) {
    return undefined;
  }

  // If we don't have a silent frame pregenerated for the sample rate, we use a
  // frame from the content instead
  const silentFrame =
    generateFrameOfSilence()[track.samplerate] ?? frames[0].data;

  for (let i = 0; i < audioFillFrameCount; i++) {
    const firstFrame = frames[0];
    frames.splice(0, 0, {
      data: silentFrame,
      dts: firstFrame.dts - frameDuration,
      pts: firstFrame.pts - frameDuration,
    });
  }

  track.baseMediaDecodeTime -= Math.floor(
    videoTsToAudioTs(audioFillDuration, track.samplerate),
  );

  return audioFillDuration;
}

/**
 * If the audio segment extends before the earliest allowed dts
 * value, remove AAC frames until starts at or after the earliest
 * allowed DTS so that we don't end up with a negative baseMedia-
 * DecodeTime for the audio track
 * @param {Array.<Object>} adtsFrames
 * @param {Object} track
 * @param {number} earliestAllowedDts
 * @returns {Array.<Object>}
 */
function trimAdtsFramesByEarliestDts(
  adtsFrames: FrameData[],
  track: TrackInfo,
  earliestAllowedDts: number,
): FrameData[] {
  if (track.minSegmentDts >= earliestAllowedDts) {
    return adtsFrames;
  }

  // We will need to recalculate the earliest segment Dts
  track.minSegmentDts = Infinity;

  return adtsFrames.filter(function (currentFrame) {
    // If this is an allowed frame, keep it and record it's Dts
    if (currentFrame.dts >= earliestAllowedDts) {
      track.minSegmentDts = Math.min(track.minSegmentDts, currentFrame.dts);
      track.minSegmentPts = track.minSegmentDts;
      return true;
    }
    // Otherwise, discard it
    return false;
  });
}

/**
 * @param {Array.<Object>} frames
 * @returns {Array.<Object>}
 */
function generateSampleTable(frames: FrameData[]): Array<{
  size: number;
  duration: number;
}> {
  const samples: Array<{
    size: number;
    duration: number;
  }> = [];

  for (const currentFrame of frames) {
    samples.push({
      size: currentFrame.data.byteLength,
      duration: 1024, // For AAC audio, all samples contain 1024 samples
    });
  }
  return samples;
}

/**
 * Generate the track's sample table from an array of frames.
 * @param {Array.<Object>} frames
 * @returns {Uint8Array}
 */
function concatenateFrameData(frames: FrameData[]): Uint8Array {
  let dataOffset = 0;
  const data = new Uint8Array(sumFrameByteLengths(frames));

  for (const currentFrame of frames) {
    data.set(currentFrame.data, dataOffset);
    dataOffset += currentFrame.data.byteLength;
  }
  return data;
}

export {
  prefixWithSilence,
  trimAdtsFramesByEarliestDts,
  generateSampleTable,
  concatenateFrameData,
};
