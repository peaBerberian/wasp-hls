// Convert an array of nal units into an array of frames with each frame being
// composed of the nal units that make up that frame
// Also keep track of cummulative data about the frame from the nal units such

import { NalUnitType, ParsedNalUnit } from "./H264NalUnitProducer";

// as the frame duration, starting pts, etc.
function groupNalsIntoFrames(nalUnits: ParsedNalUnit[]): any[] {
  let currentFrame: any = [];
  const frames: any = [];

  // TODO added for LHLS, make sure this is OK
  frames.byteLength = 0;
  frames.nalCount = 0;
  frames.duration = 0;

  currentFrame.byteLength = 0;

  for (const currentNal of nalUnits) {
    // Split on 'aud'-type nal units
    if (currentNal.nalUnitType === NalUnitType.AccessUnitDelim) {
      // Since the very first nal unit is expected to be an AUD
      // only push to the frames array when currentFrame is not empty
      if (currentFrame.length > 0) {
        currentFrame.duration = currentNal.dts - currentFrame.dts;
        // TODO added for LHLS, make sure this is OK
        frames.byteLength += currentFrame.byteLength;
        frames.nalCount += currentFrame.length;
        frames.duration += currentFrame.duration;
        frames.push(currentFrame);
      }
      currentFrame = [currentNal];
      currentFrame.byteLength = currentNal.data.byteLength;
      currentFrame.pts = currentNal.pts;
      currentFrame.dts = currentNal.dts;
    } else {
      // Specifically flag key frames for ease of use later
      if (currentNal.nalUnitType === NalUnitType.SliceLayerWo) {
        currentFrame.keyFrame = true;
      }
      currentFrame.duration = currentNal.dts - currentFrame.dts;
      currentFrame.byteLength += currentNal.data.byteLength;
      currentFrame.push(currentNal);
    }
  }

  // For the last frame, use the duration of the previous frame if we
  // have nothing better to go on
  if (
    frames.length > 0 &&
    (currentFrame.duration == null || currentFrame.duration <= 0)
  ) {
    currentFrame.duration = frames[frames.length - 1].duration;
  }

  // Push the final frame
  // TODO added for LHLS, make sure this is OK
  frames.byteLength += currentFrame.byteLength;
  frames.nalCount += currentFrame.length;
  frames.duration += currentFrame.duration;

  frames.push(currentFrame);
  return frames;
}

// Convert an array of frames into an array of Gop with each Gop being composed
// of the frames that make up that Gop
// Also keep track of cummulative data about the Gop from the frames such as the
// Gop duration, starting pts, etc.
function groupFramesIntoGops(frames: any[]): any {
  let currentGop: any = [];
  const gops: any = [];

  // We must pre-set some of the values on the Gop since we
  // keep running totals of these values
  currentGop.byteLength = 0;
  currentGop.nalCount = 0;
  currentGop.duration = 0;
  currentGop.pts = frames[0].pts;
  currentGop.dts = frames[0].dts;

  // store some metadata about all the Gops
  gops.byteLength = 0;
  gops.nalCount = 0;
  gops.duration = 0;
  gops.pts = frames[0].pts;
  gops.dts = frames[0].dts;

  for (const currentFrame of frames) {
    if (currentFrame.keyFrame === true) {
      // Since the very first frame is expected to be an keyframe
      // only push to the gops array when currentGop is not empty
      if (currentGop.length > 0) {
        gops.push(currentGop);
        gops.byteLength += currentGop.byteLength;
        gops.nalCount += currentGop.nalCount;
        gops.duration += currentGop.duration;
      }

      currentGop = [currentFrame];
      currentGop.nalCount = currentFrame.length;
      currentGop.byteLength = currentFrame.byteLength;
      currentGop.pts = currentFrame.pts;
      currentGop.dts = currentFrame.dts;
      currentGop.duration = currentFrame.duration;
    } else {
      currentGop.duration += currentFrame.duration;
      currentGop.nalCount += currentFrame.length;
      currentGop.byteLength += currentFrame.byteLength;
      currentGop.push(currentFrame);
    }
  }

  if (gops.length > 0 && currentGop.duration <= 0) {
    currentGop.duration = gops[gops.length - 1].duration;
  }
  gops.byteLength += currentGop.byteLength;
  gops.nalCount += currentGop.nalCount;
  gops.duration += currentGop.duration;

  // push the final Gop
  gops.push(currentGop);
  return gops;
}

/*
 * Search for the first keyframe in the GOPs and throw away all frames
 * until that keyframe. Then extend the duration of the pulled keyframe
 * and pull the PTS and DTS of the keyframe so that it covers the time
 * range of the frames that were disposed.
 *
 * @param {Array} gops video GOPs
 * @returns {Array} modified video GOPs
 */
function extendFirstKeyFrame(gops: any): any {
  if (gops[0][0].keyFrame !== true && gops.length > 1) {
    // Remove the first GOP
    const currentGop = gops.shift();

    gops.byteLength -= currentGop.byteLength;
    gops.nalCount -= currentGop.nalCount;

    // Extend the first frame of what is now the
    // first gop to cover the time period of the
    // frames we just removed
    gops[0][0].dts = currentGop.dts;
    gops[0][0].pts = currentGop.pts;
    gops[0][0].duration += currentGop.duration;
  }

  return gops;
}

/**
 * Default sample object
 * see ISO/IEC 14496-12:2012, section 8.6.4.3
 */
function createDefaultSample(): any {
  return {
    size: 0,
    flags: {
      isLeading: 0,
      dependsOn: 1,
      isDependedOn: 0,
      hasRedundancy: 0,
      degradationPriority: 0,
      isNonSyncSample: 1,
    },
  };
}

/*
 * Collates information from a video frame into an object for eventual
 * entry into an MP4 sample table.
 *
 * @param {Object} frame the video frame
 * @param {Number} dataOffset the byte offset to position the sample
 * @return {Object} object containing sample table info for a frame
 */
function sampleForFrame(frame: any, dataOffset: number): any {
  const sample = createDefaultSample();

  sample.dataOffset = dataOffset;
  sample.compositionTimeOffset = frame.pts - frame.dts;
  sample.duration = frame.duration;
  sample.size = 4 * frame.length; // Space for nal unit size
  sample.size += frame.byteLength;

  if (frame.keyFrame === true) {
    sample.flags.dependsOn = 2;
    sample.flags.isNonSyncSample = 0;
  }

  return sample;
}

// generate the track's sample table from an array of gops
function generateSampleTable(gops: any, baseDataOffset?: number): any[] {
  let dataOffset = baseDataOffset ?? 0;
  const samples: any[] = [];
  for (const currentGop of gops) {
    for (const currentFrame of currentGop) {
      const sample = sampleForFrame(currentFrame, dataOffset);
      dataOffset += sample.size;
      samples.push(sample);
    }
  }
  return samples;
}

// generate the track's raw mdat data from an array of gops
function concatenateNalData(gops: any): Uint8Array {
  let dataOffset = 0;
  const nalsByteLength = gops.byteLength;
  const numberOfNals = gops.nalCount;
  const totalByteLength = nalsByteLength + 4 * numberOfNals;
  const data = new Uint8Array(totalByteLength);
  const view = new DataView(data.buffer);

  // For each Gop..
  for (const currentGop of gops) {
    // For each Frame..
    for (const currentFrame of currentGop) {
      // For each NAL..
      for (const currentNal of currentFrame) {
        view.setUint32(dataOffset, currentNal.data.byteLength);
        dataOffset += 4;
        data.set(currentNal.data, dataOffset);
        dataOffset += currentNal.data.byteLength;
      }
    }
  }
  return data;
}

// generate the track's sample table from a frame
function generateSampleTableForFrame(
  frame: any,
  baseDataOffset: number,
): any[] {
  const dataOffset = baseDataOffset ?? 0;
  const samples: any[] = [];
  const sample = sampleForFrame(frame, dataOffset);
  samples.push(sample);
  return samples;
}

// generate the track's raw mdat data from a frame
function concatenateNalDataForFrame(frame: any): Uint8Array {
  let dataOffset = 0;
  const nalsByteLength = frame.byteLengthl;
  const numberOfNals = frame.length;
  const totalByteLength = nalsByteLength + 4 * numberOfNals;
  const data = new Uint8Array(totalByteLength);
  const view = new DataView(data.buffer);

  // For each NAL..
  for (const currentNal of frame) {
    view.setUint32(dataOffset, currentNal.data.byteLength);
    dataOffset += 4;
    data.set(currentNal.data, dataOffset);
    dataOffset += currentNal.data.byteLength;
  }
  return data;
}

export {
  groupNalsIntoFrames,
  groupFramesIntoGops,
  extendFirstKeyFrame,
  generateSampleTable,
  concatenateNalData,
  generateSampleTableForFrame,
  concatenateNalDataForFrame,
};
