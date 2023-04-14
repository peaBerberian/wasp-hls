import {
  concatenateNalData,
  extendFirstKeyFrame,
  generateSampleTable,
  groupFramesIntoGops,
  groupNalsIntoFrames,
} from "./frame-utils";
import { NalUnitType, ParsedNalUnit } from "./H264NalUnitProducer";
import { createMdat, createMoof } from "./mp4-utils";
import {
  calculateTrackBaseMediaDecodeTime,
  clearDtsInfo,
  collectDtsInfo,
} from "./track-utils";
import { TrackInfo } from "./types";

export interface Mp4VideoSegmentData {
  trackInfo: TrackInfo;
  boxes: Uint8Array;
}

/**
 * Compare two arrays (even typed) for same-ness
 */
function arrayEquals<T>(a: T[], b: T[]): boolean {
  if (a.length !== b.length) {
    return false;
  }

  // compare the value of each element in the array
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) {
      return false;
    }
  }

  return true;
}

export interface Mp4VideoSegmentGeneratorOptions {
  firstSequenceNumber?: number;
  /**
   * If true, start from the end of the gopsToAlignWith list when attempting
   * to align gop pts
   */
  alignGopsAtEnd?: boolean;
  /**
   * If true, keep the timestamps in the source; false to adjust the first
   * segment to start at 0.
   */
  keepOriginalTimestamps?: boolean;
}

/**
 * Constructs a single-track, ISO BMFF media segment from H264 data.
 * @class Mp4VideoSegmentGenerator
 */
export default class Mp4VideoSegmentGenerator {
  private _sequenceNumber: number;
  private _nalUnits: ParsedNalUnit[];
  private _gopsToAlignWith: any[];
  private _hasParsedTrackInfo: boolean;
  private _pps: Uint8Array | undefined;
  private _shouldAlignGopsAtEnd: boolean;
  private _keepOriginalTimestamps: boolean;
  private _gopCache: any[];
  private _trackInfo: any;

  /**
   * @param {Object} trackInfo - track metadata configuration
   * @param {object} options - transmuxer options object
   */
  constructor(trackInfo: any, options: Mp4VideoSegmentGeneratorOptions) {
    this._sequenceNumber = options.firstSequenceNumber ?? 0;
    this._shouldAlignGopsAtEnd = options.alignGopsAtEnd === true;
    this._keepOriginalTimestamps = options.keepOriginalTimestamps === true;
    this._nalUnits = [];
    this._gopsToAlignWith = [];
    this._hasParsedTrackInfo = false;
    this._pps = undefined;
    this._gopCache = [];
    this._trackInfo = trackInfo;
  }

  /**
   * Push new Nal Unit to the `Mp4VideoSegmentGenerator` which will be used to
   * generate a video segment once `generateBoxes` is called (once all Nal
   * Units) of a segment have been pushed.
   * @param {Object} nalUnit
   **/
  public pushNalUnit(nalUnit: ParsedNalUnit): void {
    collectDtsInfo(this._trackInfo, nalUnit);

    // record the track config
    if (
      nalUnit.nalUnitType === NalUnitType.SeqParamSetRbsp &&
      !this._hasParsedTrackInfo
    ) {
      this._trackInfo.sps = [nalUnit.data];
      this._trackInfo.width = nalUnit.config.width;
      this._trackInfo.height = nalUnit.config.height;
      this._trackInfo.profileIdc = nalUnit.config.profileIdc;
      this._trackInfo.levelIdc = nalUnit.config.levelIdc;
      this._trackInfo.profileCompatibility =
        nalUnit.config.profileCompatibility;
      this._trackInfo.sarRatio = nalUnit.config.sarRatio;
    }
    if (
      nalUnit.nalUnitType === NalUnitType.PicParamSet &&
      this._pps === undefined
    ) {
      this._pps = nalUnit.data;
      this._trackInfo.pps = [nalUnit.data];
    }
    this._nalUnits.push(nalUnit);
  }

  /**
   * Generate ISOBMFF data for the video segment from the Nal Unit pushed thus
   * far.
   * Returns `null` if no segment could have been generated.
   * @returns {Object|null}
   **/
  public generateBoxes(): Mp4VideoSegmentData | null {
    // Throw away nalUnits at the start of the byte stream until
    // we find the first AUD
    while (this._nalUnits.length > 0) {
      if (this._nalUnits[0].nalUnitType === NalUnitType.AccessUnitDelim) {
        break;
      }
      this._nalUnits.shift();
    }

    // Return early if no video data has been observed
    if (this._nalUnits.length === 0) {
      this.resetStream_();
      return null;
    }

    // Organize the raw nal-units into arrays that represent
    // higher-level constructs such as frames and gops
    // (group-of-pictures)
    const frames = groupNalsIntoFrames(this._nalUnits);
    let gops = groupFramesIntoGops(frames);

    // If the first frame of this fragment is not a keyframe we have
    // a problem since MSE (on Chrome) requires a leading keyframe.
    //
    // We have two approaches to repairing this situation:
    // 1) GOP-FUSION:
    //    This is where we keep track of the GOPS (group-of-pictures)
    //    from previous fragments and attempt to find one that we can
    //    prepend to the current fragment in order to create a valid
    //    fragment.
    // 2) KEYFRAME-PULLING:
    //    Here we search for the first keyframe in the fragment and
    //    throw away all the frames between the start of the fragment
    //    and that keyframe. We then extend the duration and pull the
    //    PTS of the keyframe forward so that it covers the time range
    //    of the frames that were disposed of.
    //
    // #1 is far prefereable over #2 which can cause "stuttering" but
    // requires more things to be just right.
    if (!(gops[0][0].keyFrame as boolean)) {
      // Search for a gop for fusion from our gopCache
      const gopForFusion = this._getGopForFusion(this._nalUnits[0]);

      if (gopForFusion !== null) {
        gops.unshift(gopForFusion);
        // Adjust Gops' metadata to account for the inclusion of the
        // new gop at the beginning
        gops.byteLength += gopForFusion.byteLength;
        gops.nalCount += gopForFusion.nalCount;
        gops.pts = gopForFusion.pts;
        gops.dts = gopForFusion.dts;
        gops.duration += gopForFusion.duration;
      } else {
        // If we didn't find a candidate gop fall back to keyframe-pulling
        gops = extendFirstKeyFrame(gops);
      }
    }

    // Trim gops to align with gopsToAlignWith
    if (this._gopsToAlignWith.length) {
      let alignedGops: any;
      if (this._shouldAlignGopsAtEnd) {
        alignedGops = this._alignGopsAtEnd(gops);
      } else {
        alignedGops = this._alignGopsAtStart(gops);
      }

      if (alignedGops === null) {
        // save all the nals in the last GOP into the gop cache
        this._gopCache.unshift({
          gop: gops.pop(),
          pps: this._trackInfo.pps,
          sps: this._trackInfo.sps,
        });

        // Keep a maximum of 6 GOPs in the cache
        this._gopCache.length = Math.min(6, this._gopCache.length);

        // Clear nalUnits
        this._nalUnits = [];

        // return early no gops can be aligned with desired gopsToAlignWith
        this.resetStream_();
        return null;
      }

      // Some gops were trimmed. clear dts info so minSegmentDts and pts are correct
      // when recalculated.
      clearDtsInfo(this._trackInfo);

      gops = alignedGops;
    }

    collectDtsInfo(this._trackInfo, gops);

    // First, we have to build the index from byte locations to
    // samples (that is, frames) in the video data
    this._trackInfo.samples = generateSampleTable(gops);

    // Concatenate the video data and construct the mdat
    const mdat = createMdat(concatenateNalData(gops));

    this._trackInfo.baseMediaDecodeTime = calculateTrackBaseMediaDecodeTime(
      this._trackInfo,
      this._keepOriginalTimestamps
    );

    // save all the nals in the last GOP into the gop cache
    this._gopCache.unshift({
      gop: gops.pop(),
      pps: this._trackInfo.pps,
      sps: this._trackInfo.sps,
    });

    // Keep a maximum of 6 GOPs in the cache
    this._gopCache.length = Math.min(6, this._gopCache.length);

    // Clear nalUnits
    this._nalUnits = [];

    const moof = createMoof(this._sequenceNumber, [this._trackInfo]);

    // it would be great to allocate this array up front instead of
    // throwing away hundreds of media segment fragments
    const boxes = new Uint8Array(moof.byteLength + mdat.byteLength);

    // Bump the sequence number for next time
    this._sequenceNumber++;

    boxes.set(moof);
    boxes.set(mdat, moof.byteLength);

    const trackInfo = { ...this._trackInfo };
    this.resetStream_();
    return { trackInfo, boxes };
  }

  public cancel(): void {
    this.resetStream_();
    this._nalUnits = [];
    this._gopCache.length = 0;
    this._gopsToAlignWith.length = 0;
  }

  public resetStream_(): void {
    clearDtsInfo(this._trackInfo);

    // reset config and pps because they may differ across segments
    // for instance, when we are rendition switching
    this._hasParsedTrackInfo = false;
    this._pps = undefined;
  }

  // Search for a candidate Gop for gop-fusion from the gop cache and
  // return it or return null if no good candidate was found
  public _getGopForFusion(nalUnit: ParsedNalUnit): any {
    const halfSecond = 45000; // Half-a-second in a 90khz clock
    const allowableOverlap = 10000; // About 3 frames @ 30fps
    let nearestDistance = Infinity;
    let nearestGopObj: any | undefined;

    // Search for the GOP nearest to the beginning of this nal unit
    this._gopCache.forEach((currentGopObj) => {
      const currentGop = currentGopObj.gop;

      // Reject Gops with different SPS or PPS
      if (
        !(
          this._trackInfo.pps != null &&
          arrayEquals(this._trackInfo.pps[0], currentGopObj.pps[0])
        ) ||
        !(
          this._trackInfo.sps != null &&
          arrayEquals(this._trackInfo.sps[0], currentGopObj.sps[0])
        )
      ) {
        return;
      }

      // Reject Gops that would require a negative baseMediaDecodeTime
      if (currentGop.dts < this._trackInfo.timelineStartInfo.dts) {
        return;
      }

      // The distance between the end of the gop and the start of the nalUnit
      const dtsDistance = nalUnit.dts - currentGop.dts - currentGop.duration;

      // Only consider GOPS that start before the nal unit and end within
      // a half-second of the nal unit
      if (dtsDistance >= -allowableOverlap && dtsDistance <= halfSecond) {
        // Always use the closest GOP we found if there is more than
        // one candidate
        if (nearestGopObj === undefined || nearestDistance > dtsDistance) {
          nearestGopObj = currentGopObj;
          nearestDistance = dtsDistance;
        }
      }
    });

    if (nearestGopObj !== undefined) {
      return nearestGopObj.gop;
    }
    return null;
  }

  // trim gop list to the first gop found that has a matching pts with a gop in the list
  // of gopsToAlignWith starting from the START of the list
  public _alignGopsAtStart(gops: any): any {
    let byteLength = gops.byteLength;
    let nalCount = gops.nalCount;
    let duration = gops.duration;
    let alignIndex = 0;
    let gopIndex = 0;

    while (
      alignIndex < this._gopsToAlignWith.length &&
      gopIndex < gops.length
    ) {
      const align = this._gopsToAlignWith[alignIndex];
      const gop = gops[gopIndex];

      if (align.pts === gop.pts) {
        break;
      }

      if (gop.pts > align.pts) {
        // this current gop starts after the current gop we want to align on, so increment
        // align index
        alignIndex++;
        continue;
      }

      // current gop starts before the current gop we want to align on. so increment gop
      // index
      gopIndex++;
      byteLength -= gop.byteLength;
      nalCount -= gop.nalCount;
      duration -= gop.duration;
    }

    if (gopIndex === 0) {
      // no gops to trim
      return gops;
    }

    if (gopIndex === gops.length) {
      // all gops trimmed, skip appending all gops
      return null;
    }

    const alignedGops = gops.slice(gopIndex);
    alignedGops.byteLength = byteLength;
    alignedGops.duration = duration;
    alignedGops.nalCount = nalCount;
    alignedGops.pts = alignedGops[0].pts;
    alignedGops.dts = alignedGops[0].dts;

    return alignedGops;
  }

  // trim gop list to the first gop found that has a matching pts with a gop in the list
  // of gopsToAlignWith starting from the END of the list
  private _alignGopsAtEnd(gops: any): any {
    let alignIndex = this._gopsToAlignWith.length - 1;
    let gopIndex = gops.length - 1;
    let alignEndIndex: number | null = null;
    let matchFound = false;

    while (alignIndex >= 0 && gopIndex >= 0) {
      const align = this._gopsToAlignWith[alignIndex];
      const gop = gops[gopIndex];

      if (align.pts === gop.pts) {
        matchFound = true;
        break;
      }

      if (align.pts > gop.pts) {
        alignIndex--;
        continue;
      }

      if (alignIndex === this._gopsToAlignWith.length - 1) {
        // gop.pts is greater than the last alignment candidate. If no match is found
        // by the end of this loop, we still want to append gops that come after this
        // point
        alignEndIndex = gopIndex;
      }

      gopIndex--;
    }

    if (!matchFound && alignEndIndex === null) {
      return null;
    }

    let trimIndex: number | null;
    if (matchFound) {
      trimIndex = gopIndex;
    } else {
      trimIndex = alignEndIndex;
    }

    if (trimIndex === 0) {
      return gops;
    }

    const alignedGops = gops.slice(trimIndex);
    const metadata = alignedGops.reduce(
      (total: any, gop: any): any => {
        total.byteLength += gop.byteLength;
        total.duration += gop.duration;
        total.nalCount += gop.nalCount;
        return total;
      },
      { byteLength: 0, duration: 0, nalCount: 0 }
    );

    alignedGops.byteLength = metadata.byteLength;
    alignedGops.duration = metadata.duration;
    alignedGops.nalCount = metadata.nalCount;
    alignedGops.pts = alignedGops[0].pts;
    alignedGops.dts = alignedGops[0].dts;

    return alignedGops;
  }

  public alignGopsWith(newGopsToAlignWith: any): void {
    this._gopsToAlignWith = newGopsToAlignWith;
  }
}
