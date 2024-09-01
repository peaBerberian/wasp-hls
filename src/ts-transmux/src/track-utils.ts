import { ONE_SECOND_IN_TS } from "./clock-utils";
import { TrackInfo } from "./types";

/**
 * Store information about the start and end of the track and the
 * duration for each frame/sample we process in order to calculate
 * the baseMediaDecodeTime.
 *
 * Mutate the given `trackInfo` to store those information.
 * @param {Object} trackInfo
 * @param {Object} data
 */
function collectDtsInfo<
  T extends {
    timelineStartInfo?:
      | {
          pts?: number | undefined;
          dts?: number | undefined;
        }
      | undefined;
    minSegmentDts?: number | undefined;
    minSegmentPts?: number | undefined;
    maxSegmentDts?: number | undefined;
    maxSegmentPts?: number | undefined;
  },
>(trackInfo: T, data: Partial<TrackInfo>) {
  if (trackInfo.timelineStartInfo === undefined) {
    trackInfo.timelineStartInfo = {};
  }
  if (typeof data.pts === "number") {
    if (trackInfo.timelineStartInfo.pts === undefined) {
      trackInfo.timelineStartInfo.pts = data.pts;
    }

    if (trackInfo.minSegmentPts === undefined) {
      trackInfo.minSegmentPts = data.pts;
    } else {
      trackInfo.minSegmentPts = Math.min(trackInfo.minSegmentPts, data.pts);
    }

    if (trackInfo.maxSegmentPts === undefined) {
      trackInfo.maxSegmentPts = data.pts;
    } else {
      trackInfo.maxSegmentPts = Math.max(trackInfo.maxSegmentPts, data.pts);
    }
  }

  if (typeof data.dts === "number") {
    if (trackInfo.timelineStartInfo.dts === undefined) {
      trackInfo.timelineStartInfo.dts = data.dts;
    }

    if (trackInfo.minSegmentDts === undefined) {
      trackInfo.minSegmentDts = data.dts;
    } else {
      trackInfo.minSegmentDts = Math.min(trackInfo.minSegmentDts, data.dts);
    }

    if (trackInfo.maxSegmentDts === undefined) {
      trackInfo.maxSegmentDts = data.dts;
    } else {
      trackInfo.maxSegmentDts = Math.max(trackInfo.maxSegmentDts, data.dts);
    }
  }
}

/**
 * Clear values used to calculate the baseMediaDecodeTime between
 * tracks
 */
function clearDtsInfo<
  T extends {
    minSegmentDts: number | undefined;
    minSegmentPts: number | undefined;
    maxSegmentDts: number | undefined;
    maxSegmentPts: number | undefined;
  },
>(trackInfo: T): void {
  trackInfo.minSegmentDts = undefined;
  trackInfo.maxSegmentDts = undefined;
  trackInfo.minSegmentPts = undefined;
  trackInfo.maxSegmentPts = undefined;
}

/**
 * Calculate the track"s baseMediaDecodeTime based on the earliest
 * DTS the transmuxer has ever seen and the minimum DTS for the
 * current track
 * @param {Object} trackInfo
 * @param {boolean} keepOriginalTimestamps - If true, keep the timestamps in the
 * source. If `false`, adjust the first segment to start at 0.
 */
function calculateTrackBaseMediaDecodeTime<
  T extends {
    type: string;
    minSegmentDts: number;
    timelineStartInfo: {
      dts?: number | undefined;
      baseMediaDecodeTime?: number | undefined;
    };
    samplerate?: number;
  },
>(trackInfo: T, keepOriginalTimestamps: boolean): number {
  let minSegmentDts = trackInfo.minSegmentDts;

  // Optionally adjust the time so the first segment starts at zero.
  if (
    !keepOriginalTimestamps &&
    trackInfo.timelineStartInfo.dts !== undefined
  ) {
    minSegmentDts -= trackInfo.timelineStartInfo.dts;
  }

  // trackInfo.timelineStartInfo.baseMediaDecodeTime is the location, in time, where
  // we want the start of the first segment to be placed
  let baseMediaDecodeTime = trackInfo.timelineStartInfo.baseMediaDecodeTime;

  if (baseMediaDecodeTime !== undefined) {
    // Add to that the distance this segment is from the very first
    baseMediaDecodeTime += minSegmentDts;
  } else {
    baseMediaDecodeTime = minSegmentDts;
    trackInfo.timelineStartInfo.baseMediaDecodeTime = baseMediaDecodeTime;
  }

  // baseMediaDecodeTime must not become negative
  baseMediaDecodeTime = Math.max(0, baseMediaDecodeTime);

  if (trackInfo.type === "audio") {
    // Audio has a different clock equal to the sampling_rate so we need to
    // scale the PTS values into the clock rate of the track
    const scale = (trackInfo.samplerate as number) / ONE_SECOND_IN_TS;
    baseMediaDecodeTime *= scale;
    baseMediaDecodeTime = Math.floor(baseMediaDecodeTime);
  }

  return baseMediaDecodeTime;
}

export { clearDtsInfo, calculateTrackBaseMediaDecodeTime, collectDtsInfo };
