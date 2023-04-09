import EventEmitter from "../../ts-common/EventEmitter";
import { readNextAdtsOrId3 } from "./aac-stream";
import { isLikelyAacData } from "./aac-utils";
import { AacFrame, AdtsPacketParser } from "./adts-stream";
import {
  prefixWithSilence,
  trimAdtsFramesByEarliestDts,
  generateSampleTable as generateAudioSampleTable,
  concatenateFrameData,
} from "./audio-frame-utils";
import * as clock from "./clock-utils";
import { AUDIO_PROPERTIES, VIDEO_PROPERTIES } from "./constants";
import {
  groupNalsIntoFrames,
  groupFramesIntoGops,
  extendFirstKeyFrame,
  generateSampleTable,
  concatenateNalData,
} from "./frame-utils";
import { H264Stream } from "./h264-streams";
import {
  TransportParseStream,
  ElementaryStream,
  CaptionStream,
  TransportPacketParser,
} from "./m2ts-streams";
import { TimedMetadataParser } from "./metadata-stream";
import { createMdat, createMoof, createInitSegment } from "./mp4-utils";
import { TimestampRolloverHandler } from "./timestamp-rollover-stream";
import {
  calculateTrackBaseMediaDecodeTime,
  collectDtsInfo,
  clearDtsInfo,
} from "./track-decode-info";
import { TrackInfo } from "./types";

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

interface TimingInformation {
  start: {
    dts: number;
    pts: number;
  };
  end: {
    dts: number;
    pts: number;
  };
  prependedContentDuration: boolean;
  baseMediaDecodeTime: number;
}

function generateSegmentTimingInfo(
  baseMediaDecodeTime: number,
  startDts: number,
  startPts: number,
  endDts: number,
  endPts: number,
  prependedContentDuration: boolean
): TimingInformation {
  const ptsOffsetFromDts = startPts - startDts;
  const decodeDuration = endDts - startDts;
  const presentationDuration = endPts - startPts;

  // The PTS and DTS values are based on the actual stream times from the segment,
  // however, the player time values will reflect a start from the baseMediaDecodeTime.
  // In order to provide relevant values for the player times, base timing info on the
  // baseMediaDecodeTime and the DTS and PTS durations of the segment.
  return {
    start: {
      dts: baseMediaDecodeTime,
      pts: baseMediaDecodeTime + ptsOffsetFromDts,
    },
    end: {
      dts: baseMediaDecodeTime + decodeDuration,
      pts: baseMediaDecodeTime + presentationDuration,
    },
    prependedContentDuration,
    baseMediaDecodeTime,
  };
}

interface AudioSegmentStreamOptions {
  firstSequenceNumber?: number | undefined;
  keepOriginalTimestamps?: boolean | undefined;
}

interface AudioSegmentStreamEvents {
  reset: null;
  done: "AudioSegmentStream";
  partialdone: null;
  data: {
    trackInfo: TrackInfo;
    boxes: Uint8Array;
  };
}

/**
 * Constructs a single-track, ISOBMFF media segment from AAC data events.
 *
 * The output of this stream can be fed to a SourceBuffer configured with a
 * suitable initialization segment.
 * @class AudioSegmentStream
 */
class AudioSegmentStream extends EventEmitter<AudioSegmentStreamEvents> {
  private _adtsFrames: any[];
  private _earliestAllowedDts: number;
  private _audioAppendStartTs: number;
  private _videoBaseMediaDecodeTime: number;
  private _sequenceNumber: number;
  private _keepOriginalTimestamps: boolean;
  private _trackInfo: TrackInfo;

  /**
   * @param {Object} trackInfo
   * @param {Object} options
   */
  constructor(trackInfo: TrackInfo, options: AudioSegmentStreamOptions = {}) {
    super();
    this._adtsFrames = [];
    this._earliestAllowedDts = 0;
    this._audioAppendStartTs = 0;
    this._videoBaseMediaDecodeTime = Infinity;
    this._trackInfo = trackInfo;
    this._sequenceNumber = options.firstSequenceNumber ?? 0;
    this._keepOriginalTimestamps = options.keepOriginalTimestamps === true;
  }

  public push(data: AacFrame): void {
    collectDtsInfo(this._trackInfo, data);
    this._trackInfo.audioobjecttype = data.audioobjecttype;
    this._trackInfo.channelcount = data.channelcount;
    this._trackInfo.samplerate = data.samplerate;
    this._trackInfo.samplingfrequencyindex = data.samplingfrequencyindex;
    this._trackInfo.samplesize = data.samplesize;
    this._adtsFrames.push(data);
  }

  public setEarliestDts(earliestDts: number): void {
    this._earliestAllowedDts = earliestDts;
  }

  public setVideoBaseMediaDecodeTime(baseMediaDecodeTime: number): void {
    this._videoBaseMediaDecodeTime = baseMediaDecodeTime;
  }

  public setAudioAppendStart(timestamp: number): void {
    this._audioAppendStartTs = timestamp;
  }

  public flush(): void {
    if (this._adtsFrames.length === 0) {
      this.trigger("done", "AudioSegmentStream");
      return;
    }

    const frames = trimAdtsFramesByEarliestDts(
      this._adtsFrames,
      this._trackInfo,
      this._earliestAllowedDts
    );

    this._trackInfo.baseMediaDecodeTime = calculateTrackBaseMediaDecodeTime(
      this._trackInfo,
      this._keepOriginalTimestamps
    );

    // amount of audio filled but the value is in video clock rather than audio clock
    prefixWithSilence(
      this._trackInfo,
      frames,
      this._audioAppendStartTs,
      this._videoBaseMediaDecodeTime
    );

    // we have to build the index from byte locations to
    // samples (that is, adts frames) in the audio data
    this._trackInfo.samples = generateAudioSampleTable(frames);

    // concatenate the audio data to constuct the mdat
    const mdat = createMdat(concatenateFrameData(frames));

    this._adtsFrames = [];

    const moof = createMoof(this._sequenceNumber, [this._trackInfo]);
    const boxes = new Uint8Array(moof.byteLength + mdat.byteLength);

    // bump the sequence number for next time
    this._sequenceNumber++;

    boxes.set(moof);
    boxes.set(mdat, moof.byteLength);

    clearDtsInfo(this._trackInfo);

    this.trigger("data", { trackInfo: this._trackInfo, boxes });
    this.trigger("done", "AudioSegmentStream");
  }

  public partialFlush(): void {
    this.trigger("partialdone", null);
  }

  public reset(): void {
    clearDtsInfo(this._trackInfo);
    this._adtsFrames = [];
    this.trigger("reset", null);
  }
}

interface VideoSegmentStreamEvents {
  reset: null;
  baseMediaDecodeTime: number;
  timelineStartInfo: any;
  done: "VideoSegmentStream";
  partialdone: null;
  data: {
    trackInfo: TrackInfo;
    boxes: Uint8Array;
  };
}

/**
 * Constructs a single-track, ISO BMFF media segment from H264 data
 * events. The output of this stream can be fed to a SourceBuffer
 * configured with a suitable initialization segment.
 */
class VideoSegmentStream extends EventEmitter<VideoSegmentStreamEvents> {
  private _sequenceNumber: number;
  private _nalUnits: any[];
  private _gopsToAlignWith: any[];
  private _config: any;
  private _pps: any;
  private _options: any;
  private _gopCache: any[];
  private _trackInfo: any;

  /**
   * @param trackInfo {object} track metadata configuration
   * @param options {object} transmuxer options object
   * @param options.alignGopsAtEnd {boolean} If true, start from the end of the
   *        gopsToAlignWith list when attempting to align gop pts
   * @param options.keepOriginalTimestamps {boolean} If true, keep the timestamps
   *        in the source; false to adjust the first segment to start at 0.
   */
  constructor(trackInfo: any, options: any) {
    super();
    this._sequenceNumber = options.firstSequenceNumber ?? 0;
    this._nalUnits = [];
    this._gopsToAlignWith = [];
    this._config = undefined;
    this._pps = undefined;
    this._options = options ?? {};
    this._gopCache = [];
    this._trackInfo = trackInfo;
    delete trackInfo.minPTS;
  }

  /**
   * Constructs a ISO BMFF segment given H264 nalUnits
   * @param {Object} nalUnit A data event representing a nalUnit
   * @param {String} nalUnit.nalUnitType
   * @param {Object} nalUnit.config Properties for a mp4 track
   * @param {Uint8Array} nalUnit.data The nalUnit bytes
   * @see lib/codecs/h264.js
   **/
  public push(nalUnit: any): void {
    collectDtsInfo(this._trackInfo, nalUnit);

    // record the track config
    if (
      nalUnit.nalUnitType === "seq_parameter_set_rbsp" &&
      this._config === undefined
    ) {
      this._config = nalUnit.config;
      this._trackInfo.sps = [nalUnit.data];

      VIDEO_PROPERTIES.forEach(function (prop) {
        this._trackInfo[prop] = this._config[prop];
      }, this);
    }

    if (
      nalUnit.nalUnitType === "pic_parameter_set_rbsp" &&
      this._pps === undefined
    ) {
      this._pps = nalUnit.data;
      this._trackInfo.pps = [nalUnit.data];
    }

    // buffer video until flush() is called
    this._nalUnits.push(nalUnit);
  }

  /**
   * Pass constructed ISO BMFF track and boxes on to the
   * next stream in the pipeline
   **/
  public flush() {
    // Throw away nalUnits at the start of the byte stream until
    // we find the first AUD
    while (this._nalUnits.length > 0) {
      if (this._nalUnits[0].nalUnitType === "access_unit_delimiter_rbsp") {
        break;
      }
      this._nalUnits.shift();
    }

    // Return early if no video data has been observed
    if (this._nalUnits.length === 0) {
      this.resetStream_();
      this.trigger("done", "VideoSegmentStream");
      return;
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
      if (this._options.alignGopsAtEnd as boolean) {
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
        this.trigger("done", "VideoSegmentStream");
        return;
      }

      // Some gops were trimmed. clear dts info so minSegmentDts and pts are correct
      // when recalculated before sending off to CoalesceStream
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
      this._options.keepOriginalTimestamps
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

    this.trigger("baseMediaDecodeTime", this._trackInfo.baseMediaDecodeTime);
    this.trigger("timelineStartInfo", this._trackInfo.timelineStartInfo);

    const moof = createMoof(this._sequenceNumber, [this._trackInfo]);

    // it would be great to allocate this array up front instead of
    // throwing away hundreds of media segment fragments
    const boxes = new Uint8Array(moof.byteLength + mdat.byteLength);

    // Bump the sequence number for next time
    this._sequenceNumber++;

    boxes.set(moof);
    boxes.set(mdat, moof.byteLength);

    this.trigger("data", { trackInfo: this._trackInfo, boxes });

    this.resetStream_();

    // Continue with the flush process now
    this.trigger("done", "VideoSegmentStream");
  }

  public reset(): void {
    this.resetStream_();
    this._nalUnits = [];
    this._gopCache.length = 0;
    this._gopsToAlignWith.length = 0;
    this.trigger("reset", null);
  }

  public resetStream_(): void {
    clearDtsInfo(this._trackInfo);

    // reset config and pps because they may differ across segments
    // for instance, when we are rendition switching
    this._config = undefined;
    this._pps = undefined;
  }

  // Search for a candidate Gop for gop-fusion from the gop cache and
  // return it or return null if no good candidate was found
  public _getGopForFusion(nalUnit: any): any {
    const halfSecond = 45000; // Half-a-second in a 90khz clock
    const allowableOverlap = 10000; // About 3 frames @ 30fps
    let nearestDistance = Infinity;
    let nearestGopObj: any | undefined;

    // Search for the GOP nearest to the beginning of this nal unit
    for (let i = 0; i < this._gopCache.length; i++) {
      const currentGopObj = this._gopCache[i];
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
        continue;
      }

      // Reject Gops that would require a negative baseMediaDecodeTime
      if (currentGop.dts < this._trackInfo.timelineStartInfo.dts) {
        continue;
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
    }

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

interface CoalesceStreamEvents {
  reset: null;
  done: null;
  partialdone: null;
  data: any;
  id3Frame: any;
  caption: any;
}

/**
 * A Stream that can combine multiple streams (ie. audio & video)
 * into a single output segment for MSE. Also supports audio-only
 * and video-only streams.
 */
class CoalesceStream extends EventEmitter<CoalesceStreamEvents> {
  public numberOfTracks: number;
  public metadataDispatchType: string;
  public remuxTracks: boolean;
  public keepOriginalTimestamps: boolean;
  public pendingTracks: any[];
  public audioTrack: any;
  public videoTrack: any;
  public pendingBoxes: any[];
  public pendingCaptions: any[];
  public pendingMetadata: any[];
  public pendingBytes: number;
  public emittedTracks: number;

  constructor(options: any, metadataDispatchType: string) {
    super();

    // Number of Tracks per output segment
    // If greater than 1, we combine multiple
    // tracks into a single segment
    this.numberOfTracks = 0;
    this.metadataDispatchType = metadataDispatchType;

    const opts = options ?? {};

    if (typeof opts.remux === "boolean") {
      this.remuxTracks = opts.remux;
    } else {
      this.remuxTracks = true;
    }

    if (typeof opts.keepOriginalTimestamps === "boolean") {
      this.keepOriginalTimestamps = opts.keepOriginalTimestamps;
    } else {
      this.keepOriginalTimestamps = false;
    }

    this.pendingTracks = [];
    this.videoTrack = null;
    this.pendingBoxes = [];
    this.pendingCaptions = [];
    this.pendingMetadata = [];
    this.pendingBytes = 0;
    this.emittedTracks = 0;
    this.audioTrack = null;
  }

  // Take output from multiple
  public push(output: any): number | undefined {
    // buffer incoming captions until the associated video segment
    // finishes
    if (output.text != null) {
      return this.pendingCaptions.push(output);
    }
    // buffer incoming id3 tags until the final flush
    if (output.frames != null) {
      return this.pendingMetadata.push(output);
    }

    // Add this track to the list of pending tracks and store
    // important information required for the construction of
    // the final segment
    this.pendingTracks.push(output.trackInfo);
    this.pendingBytes += output.boxes.byteLength;

    // TODO: is there an issue for this against chrome?
    // We unshift audio and push video because
    // as of Chrome 75 when switching from
    // one init segment to another if the video
    // mdat does not appear after the audio mdat
    // only audio will play for the duration of our transmux.
    if (output.trackInfo.type === "video") {
      this.videoTrack = output.trackInfo;
      this.pendingBoxes.push(output.boxes);
    }
    if (output.trackInfo.type === "audio") {
      this.audioTrack = output.trackInfo;
      this.pendingBoxes.unshift(output.boxes);
    }
  }

  public flush(flushSource: any): void {
    let offset = 0;
    const event: any = {
      captions: [],
      captionStreams: {},
      metadata: [],
      info: {},
    };

    if (this.pendingTracks.length < this.numberOfTracks) {
      if (
        flushSource !== "VideoSegmentStream" &&
        flushSource !== "AudioSegmentStream"
      ) {
        // Return because we haven't received a flush from a data-generating
        // portion of the segment (meaning that we have only recieved meta-data
        // or captions.)
        return;
      } else if (this.remuxTracks) {
        // Return until we have enough tracks from the pipeline to remux (if we
        // are remuxing audio and video into a single MP4)
        return;
      } else if (this.pendingTracks.length === 0) {
        // In the case where we receive a flush without any data having been
        // received we consider it an emitted track for the purposes of coalescing
        // `done` events.
        // We do this for the case where there is an audio and video track in the
        // segment but no audio data. (seen in several playlists with alternate
        // audio tracks and no audio present in the main TS segments.)
        this.emittedTracks++;

        if (this.emittedTracks >= this.numberOfTracks) {
          this.trigger("done", null);
          this.emittedTracks = 0;
        }
        return;
      }
    }

    let timelineStartPts: number | undefined;
    if (this.videoTrack != null) {
      timelineStartPts = this.videoTrack.timelineStartInfo.pts;
      VIDEO_PROPERTIES.forEach(function (prop) {
        event.info[prop] = this.videoTrack[prop];
      }, this);
    } else if (this.audioTrack != null) {
      timelineStartPts = this.audioTrack.timelineStartInfo.pts;
      AUDIO_PROPERTIES.forEach(function (prop) {
        event.info[prop] = this.audioTrack[prop];
      }, this);
    }

    if (this.videoTrack != null || this.audioTrack != null) {
      if (this.pendingTracks.length === 1) {
        event.type = this.pendingTracks[0].type;
      } else {
        event.type = "combined";
      }

      this.emittedTracks += this.pendingTracks.length;

      const initSegment = createInitSegment(this.pendingTracks);

      // Create a new typed array to hold the init segment
      event.initSegment = new Uint8Array(initSegment.byteLength);

      // Create an init segment containing a moov
      // and track definitions
      event.initSegment.set(initSegment);

      // Create a new typed array to hold the moof+mdats
      event.data = new Uint8Array(this.pendingBytes);

      // Append each moof+mdat (one per track) together
      for (let i = 0; i < this.pendingBoxes.length; i++) {
        event.data.set(this.pendingBoxes[i], offset);
        offset += this.pendingBoxes[i].byteLength;
      }

      // Translate caption PTS times into second offsets to match the
      // video timeline for the segment, and add track info
      for (let i = 0; i < this.pendingCaptions.length; i++) {
        if (timelineStartPts !== undefined) {
          const caption = this.pendingCaptions[i];
          caption.startTime = clock.metadataTsToSeconds(
            caption.startPts,
            timelineStartPts,
            this.keepOriginalTimestamps
          );
          caption.endTime = clock.metadataTsToSeconds(
            caption.endPts,
            timelineStartPts,
            this.keepOriginalTimestamps
          );

          event.captionStreams[caption.stream] = true;
          event.captions.push(caption);
        }
      }

      // Translate ID3 frame PTS times into second offsets to match the
      // video timeline for the segment
      for (let i = 0; i < this.pendingMetadata.length; i++) {
        if (timelineStartPts !== undefined) {
          const id3 = this.pendingMetadata[i];
          id3.cueTime = clock.metadataTsToSeconds(
            id3.pts,
            timelineStartPts,
            this.keepOriginalTimestamps
          );
          event.metadata.push(id3);
        }
      }

      // We add this to every single emitted segment even though we only need
      // it for the first
      event.metadata.dispatchType = this.metadataDispatchType;

      // Reset stream state
      this.pendingTracks.length = 0;
      this.videoTrack = null;
      this.pendingBoxes.length = 0;
      this.pendingCaptions.length = 0;
      this.pendingBytes = 0;
      this.pendingMetadata.length = 0;

      // Emit the built segment
      // We include captions and ID3 tags for backwards compatibility,
      // ideally we should send only video and audio in the data event
      this.trigger("data", event);
      // Emit each caption to the outside world
      // Ideally, this would happen immediately on parsing captions,
      // but we need to ensure that video data is sent back first
      // so that caption timing can be adjusted to match video timing
      for (let i = 0; i < event.captions.length; i++) {
        const caption = event.captions[i];

        this.trigger("caption", caption);
      }
      // Emit each id3 tag to the outside world
      // Ideally, this would happen immediately on parsing the tag,
      // but we need to ensure that video data is sent back first
      // so that ID3 frame timing can be adjusted to match video timing
      for (let i = 0; i < event.metadata.length; i++) {
        const id3 = event.metadata[i];

        this.trigger("id3Frame", id3);
      }
    }

    // Only emit `done` if all tracks have been flushed and emitted
    if (this.emittedTracks >= this.numberOfTracks) {
      this.trigger("done", null);
      this.emittedTracks = 0;
    }
  }

  public setRemux(val: boolean): void {
    this.remuxTracks = val;
  }
}

interface AacPipelineInfo {
  name: "aac";
  adtsParser: AdtsPacketParser;
  audioTimestampRolloverHandler: TimestampRolloverHandler;
  timedMetadataRolloverHandler: TimestampRolloverHandler;
  metadataParser: TimedMetadataParser;
  getAdtsTimestamp(): number;
  audioSegmentStream: AudioSegmentStream | null;
  coalesceStream: CoalesceStream;
}

interface TsPipelineInfo {
  name: "ts";
  transportPacketParser: TransportPacketParser;
  metadataParser: TimedMetadataParser;
  timestampRolloverHandler: TimestampRolloverHandler;
  adtsParser: AdtsPacketParser;

  transportParseStream: TransportParseStream;
  elementaryStream: ElementaryStream;
  h264Stream: H264Stream;
  coalesceStream: CoalesceStream;
  videoSegmentStream: VideoSegmentStream | null;
  audioSegmentStream: AudioSegmentStream | null;
  captionStream: CaptionStream;
}

export default class Transmuxer {
  private _options: any;
  private _hasFlushed: boolean;
  private _videoTrack: any | null;
  private _audioTrack: any | null;
  private _baseMediaDecodeTime: number;

  private _currentPipelineInfo: AacPipelineInfo | TsPipelineInfo | null;

  constructor(options?: any) {
    this._options = options ?? {};
    this._baseMediaDecodeTime = options?.baseMediaDecodeTime ?? 0;

    this._hasFlushed = true;

    this._currentPipelineInfo = null;
    this._videoTrack = null;
    this._audioTrack = null;
  }

  public transmuxSegment(data: Uint8Array): Uint8Array | null {
    if (this._hasFlushed) {
      const isAac = isLikelyAacData(data);

      if (isAac && this._currentPipelineInfo?.name !== "aac") {
        this._setupAacPipeline();
      } else if (!isAac && this._currentPipelineInfo?.name !== "ts") {
        this._setupTsPipeline();
      }
      this._hasFlushed = false;
    }

    if (this._currentPipelineInfo?.name === "aac") {
      return this.pushAacSegment(data);
    } else {
      return this.pushTsSegment(data);
    }
  }

  // TODO make it useful for partial segments
  // Here it doesn't serve any role in the public API
  private signalEndOfSegment() {
    this._hasFlushed = true;

    if (this._currentPipelineInfo?.name === "aac") {
      this._currentPipelineInfo.audioTimestampRolloverHandler.signalEndOfSegment();
      this._currentPipelineInfo.timedMetadataRolloverHandler.signalEndOfSegment();
      this._currentPipelineInfo.audioSegmentStream?.flush();
      this._currentPipelineInfo.coalesceStream.flush("AudioSegmentStream");
    } else if (this._currentPipelineInfo?.name === "ts") {
      // XXX TODO adts and metadataParser won't hear about this one.
      this._currentPipelineInfo.elementaryStream.flush();
      this._currentPipelineInfo.h264Stream.flush();
      this._currentPipelineInfo.captionStream.flush();
      this._currentPipelineInfo.audioSegmentStream?.flush();
      this._currentPipelineInfo.videoSegmentStream?.flush();
      this._currentPipelineInfo.coalesceStream.flush("VideoSegmentStream");
    }
  }

  private _setupAacPipeline(): AacPipelineInfo {
    let adtsTimestamp = 0;
    const audioTimestampRolloverHandler = new TimestampRolloverHandler();
    const timedMetadataRolloverHandler = new TimestampRolloverHandler();
    const adtsParser = new AdtsPacketParser();
    const metadataParser = new TimedMetadataParser((val) => {
      adtsTimestamp = val.timestamp;
    });
    const coalesceStream = new CoalesceStream(
      this._options,
      metadataParser.dispatchType
    );
    this._currentPipelineInfo = {
      name: "aac",
      getAdtsTimestamp(): number {
        return adtsTimestamp;
      },
      audioTimestampRolloverHandler,
      timedMetadataRolloverHandler,
      adtsParser,
      metadataParser,
      coalesceStream,
      audioSegmentStream: null,
    };
    return this._currentPipelineInfo;
  }

  public pushAacSegment(input: Uint8Array): Uint8Array | null {
    let pipelineInfo = this._currentPipelineInfo;
    if (pipelineInfo === null || pipelineInfo.name !== "aac") {
      pipelineInfo = this._setupAacPipeline();
    }

    let remainingInput: Uint8Array | null = input;
    const subSegments: Uint8Array[] = [];
    pipelineInfo.coalesceStream.addEventListener("data", (segment) => {
      const transmuxedSegment = new Uint8Array(
        segment.initSegment.byteLength + segment.data.byteLength
      );
      transmuxedSegment.set(segment.initSegment, 0);
      transmuxedSegment.set(segment.data, segment.initSegment.byteLength);
      subSegments.push(transmuxedSegment);
    });

    while (true) {
      if (remainingInput === null) {
        return this._returnCompleteSegment(subSegments);
      }
      const [packet, remainingBuffer] = readNextAdtsOrId3(
        remainingInput,
        pipelineInfo.getAdtsTimestamp()
      );
      remainingInput = remainingBuffer;
      if (packet === null) {
        return this._returnCompleteSegment(subSegments);
      }

      if (packet.type === "audio") {
        pipelineInfo.audioTimestampRolloverHandler.correctTimestamps(packet);
        const aacFrames = pipelineInfo.adtsParser.parsePacket(packet);

        if (pipelineInfo.audioSegmentStream === null) {
          this._audioTrack = this._audioTrack ?? {
            timelineStartInfo: {
              baseMediaDecodeTime: this._baseMediaDecodeTime,
            },
            codec: "adts",
            type: "audio",
          };

          // hook up the audio segment stream to the first track with aac data
          pipelineInfo.coalesceStream.numberOfTracks++;
          pipelineInfo.audioSegmentStream = new AudioSegmentStream(
            this._audioTrack,
            this._options
          );
          pipelineInfo.audioSegmentStream.addEventListener("data", (info) => {
            if (pipelineInfo !== null && pipelineInfo.name === "aac") {
              pipelineInfo.coalesceStream.push(info);
            }
          });
        }

        for (const aacFrame of aacFrames) {
          pipelineInfo.audioSegmentStream.push(aacFrame);
        }
      } else {
        // Timed  Metadata
        pipelineInfo.timedMetadataRolloverHandler.correctTimestamps(
          packet as any
        );
        const parsed = pipelineInfo.metadataParser.parsePacket(packet);
        pipelineInfo.coalesceStream.push(parsed);
      }
    }
  }

  private _setupTsPipeline(): TsPipelineInfo {
    const timestampRolloverHandler = new TimestampRolloverHandler();
    const adtsParser = new AdtsPacketParser();
    const metadataParser = new TimedMetadataParser(null);
    const coalesceStream = new CoalesceStream(
      this._options,
      metadataParser.dispatchType
    );
    const captionStream = new CaptionStream(this._options);
    const elementaryStream = new ElementaryStream();
    const transportPacketParser = new TransportPacketParser();
    const h264Stream = new H264Stream();
    const transportParseStream = new TransportParseStream();
    this._currentPipelineInfo = {
      name: "ts",
      timestampRolloverHandler,
      adtsParser,
      metadataParser,
      coalesceStream,
      audioSegmentStream: null,
      videoSegmentStream: null,
      transportPacketParser,
      elementaryStream,
      captionStream,
      h264Stream,
      transportParseStream,
    };
    return this._currentPipelineInfo;
  }

  private _returnCompleteSegment(
    subSegmentsArr: Uint8Array[]
  ): Uint8Array | null {
    this.signalEndOfSegment();
    if (this._currentPipelineInfo?.name === "aac") {
      this._currentPipelineInfo?.audioSegmentStream?.removeEventListener();
      this._currentPipelineInfo?.coalesceStream.removeEventListener();
    } else if (this._currentPipelineInfo?.name === "ts") {
      // XXX TODO
      this._currentPipelineInfo.transportParseStream.removeEventListener();
      this._currentPipelineInfo.elementaryStream.removeEventListener();
      this._currentPipelineInfo.h264Stream.removeEventListener();
      this._currentPipelineInfo.captionStream.removeEventListener();
      this._currentPipelineInfo.audioSegmentStream?.removeEventListener();
      this._currentPipelineInfo.videoSegmentStream?.removeEventListener();
      this._currentPipelineInfo.coalesceStream.removeEventListener();
    }
    if (subSegmentsArr.length === 0) {
      return null;
    } else if (subSegmentsArr.length === 1) {
      return subSegmentsArr[0];
    } else {
      const segmentSize = subSegmentsArr.reduce((acc, s) => {
        return acc + s.byteLength;
      }, 0);

      const fullSegment = new Uint8Array(segmentSize);
      let currOffset = 0;
      for (const subSegment of subSegmentsArr) {
        fullSegment.set(subSegment, currOffset);
        currOffset += subSegment.byteLength;
      }
      return fullSegment;
    }
  }

  public pushTsSegment(input: Uint8Array): Uint8Array | null {
    let pipelineInfo = this._currentPipelineInfo;
    if (pipelineInfo === null || pipelineInfo.name !== "ts") {
      pipelineInfo = this._setupTsPipeline();
    }

    const subSegments: Uint8Array[] = [];

    pipelineInfo.transportPacketParser.feed(input);

    pipelineInfo.coalesceStream.addEventListener("data", (segment) => {
      const transmuxedSegment = new Uint8Array(
        segment.initSegment.byteLength + segment.data.byteLength
      );
      transmuxedSegment.set(segment.initSegment, 0);
      transmuxedSegment.set(segment.data, segment.initSegment.byteLength);
      subSegments.push(transmuxedSegment);
    });

    pipelineInfo.captionStream.addEventListener("data", (data) => {
      if (pipelineInfo?.name === "ts") {
        pipelineInfo.coalesceStream.push(data);
      }
    });
    pipelineInfo.elementaryStream.addEventListener("data", (data: any) => {
      if (pipelineInfo?.name !== "ts") {
        return;
      }
      pipelineInfo.timestampRolloverHandler.correctTimestamps(data);
      if (data.type === "metadata") {
        let i = data.tracks.length;

        // scan the tracks listed in the metadata
        while (i--) {
          if (this._videoTrack === null && data.tracks[i].type === "video") {
            this._videoTrack = data.tracks[i];
            this._videoTrack.timelineStartInfo.baseMediaDecodeTime =
              this._baseMediaDecodeTime;
          } else if (
            this._audioTrack === null &&
            data.tracks[i].type === "audio"
          ) {
            this._audioTrack = data.tracks[i];
            this._audioTrack.timelineStartInfo.baseMediaDecodeTime =
              this._baseMediaDecodeTime;
          }
        }

        // hook up the video segment stream to the first track with h264 data
        if (
          this._videoTrack !== null &&
          pipelineInfo.videoSegmentStream === null
        ) {
          pipelineInfo.coalesceStream.numberOfTracks++;
          pipelineInfo.videoSegmentStream = new VideoSegmentStream(
            this._videoTrack,
            this._options
          );
          pipelineInfo.videoSegmentStream.addEventListener(
            "timelineStartInfo",
            (timelineStartInfo: any) => {
              if (pipelineInfo?.name !== "ts") {
                return;
              }
              // When video emits timelineStartInfo data after a flush, we forward that
              // info to the AudioSegmentStream, if it exists, because video timeline
              // data takes precedence.  Do not do this if keepOriginalTimestamps is set,
              // because this is a particularly subtle form of timestamp alteration.
              if (
                pipelineInfo.audioSegmentStream !== null &&
                this._options.keepOriginalTimestamps !== true
              ) {
                this._audioTrack.timelineStartInfo = timelineStartInfo;
                // On the first segment we trim AAC frames that exist before the
                // very earliest DTS we have seen in video because Chrome will
                // interpret any video track with a baseMediaDecodeTime that is
                // non-zero as a gap.
                pipelineInfo.audioSegmentStream.setEarliestDts(
                  timelineStartInfo.dts - this._baseMediaDecodeTime
                );
              }
            }
          );

          pipelineInfo.videoSegmentStream.addEventListener(
            "baseMediaDecodeTime",
            function (baseMediaDecodeTime) {
              if (pipelineInfo?.name !== "ts") {
                return;
              }
              if (pipelineInfo?.audioSegmentStream !== null) {
                pipelineInfo.audioSegmentStream.setVideoBaseMediaDecodeTime(
                  baseMediaDecodeTime
                );
              }
            }
          );

          pipelineInfo.videoSegmentStream.addEventListener(
            "data",
            (mp4Video: any) => {
              if (pipelineInfo?.name === "ts") {
                pipelineInfo.coalesceStream.push(mp4Video);
              }
            }
          );

          // Set up the final part of the video pipeline
          pipelineInfo.h264Stream.addEventListener("data", (h264Data: any) => {
            if (pipelineInfo?.name === "ts") {
              pipelineInfo.videoSegmentStream?.push(h264Data);
            }
          });
        }

        if (
          this._audioTrack !== null &&
          pipelineInfo.audioSegmentStream === null
        ) {
          // hook up the audio segment stream to the first track with aac data
          pipelineInfo.coalesceStream.numberOfTracks++;
          pipelineInfo.audioSegmentStream = new AudioSegmentStream(
            this._audioTrack,
            this._options
          );
          pipelineInfo.audioSegmentStream.addEventListener("data", (info) => {
            if (pipelineInfo?.name === "ts") {
              pipelineInfo.coalesceStream.push(info);
            }
          });
        }
      } else if (data.type === "video") {
        pipelineInfo.h264Stream.push(data);
      } else if (data.type === "audio") {
        if (pipelineInfo.audioSegmentStream !== null) {
          const frames = pipelineInfo.adtsParser.parsePacket(data);
          for (const frame of frames) {
            pipelineInfo.audioSegmentStream.push(frame);
          }
        }
      } else if (data.type === "timed-metadata") {
        const parsed = pipelineInfo.metadataParser.parsePacket(data);
        pipelineInfo.coalesceStream.push(parsed);
      }
    });
    pipelineInfo.transportParseStream.addEventListener("data", (data) => {
      if (pipelineInfo?.name === "ts") {
        pipelineInfo.elementaryStream.push(data);
      }
    });
    while (true) {
      const transportPacket =
        pipelineInfo.transportPacketParser.readNextPacket();
      if (transportPacket === null) {
        return this._returnCompleteSegment(subSegments);
      }
      pipelineInfo.transportParseStream.push(transportPacket);
    }
  }
}

export {
  Transmuxer,
  VideoSegmentStream,
  AudioSegmentStream,
  AUDIO_PROPERTIES,
  VIDEO_PROPERTIES,
  // exported for testing
  generateSegmentTimingInfo,
};
