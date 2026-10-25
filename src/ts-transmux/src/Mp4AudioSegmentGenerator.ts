import type { AacFrame } from "./AdtsPacketParser.ts";
import {
  prefixWithSilence,
  trimAdtsFramesByEarliestDts,
  generateSampleTable as generateAudioSampleTable,
} from "./audio-frame-utils.ts";
import { createFragment, type Mp4Fragment } from "./mp4-fragment.ts";
import { createMoof } from "./mp4-utils.ts";
import {
  calculateTrackBaseMediaDecodeTime,
  clearDtsInfo,
  collectDtsInfo,
} from "./track-utils.ts";
import type { TrackInfo } from "./types.ts";

/** Product of the `MP4AudioSegmentGenerator` */
export interface Mp4AudioSegmentData {
  /** Extracted information about the current audio track. */
  trackInfo: TrackInfo;
  /** Headers and borrowed AAC frame payloads, ready for final assembly. */
  fragment: Mp4Fragment;
  /** Timing metadata included in that segment. */
  timingInfo: {
    /** Start time, in timescale. */
    start: number;
    /** End time, in timescale. */
    end: number;
    /** The timescale used for start and end. */
    timescale: number;
  };
}

/**
 * Constructs a single-track, ISOBMFF media segment from AAC frames.
 * @class Mp4AudioSegmentGenerator
 */
export class Mp4AudioSegmentGenerator {
  private _aacFrames: AacFrame[];
  private _sequenceNumber: number;
  private _trackInfo: TrackInfo;

  /**
   * @param {Object} trackInfo
   */
  constructor(trackInfo: TrackInfo, firstSequenceNumber?: number) {
    this._aacFrames = [];
    this._sequenceNumber = firstSequenceNumber ?? 0;
    this._trackInfo = trackInfo;
  }

  public pushAacFrame(data: AacFrame): void {
    collectDtsInfo(this._trackInfo, data);
    this._trackInfo.audioobjecttype = data.audioobjecttype;
    this._trackInfo.channelcount = data.channelcount;
    this._trackInfo.samplerate = data.samplerate;
    this._trackInfo.samplingfrequencyindex = data.samplingfrequencyindex;
    this._trackInfo.samplesize = data.samplesize;
    this._aacFrames.push(data);
  }

  public generateBoxes(args: {
    earliestAllowedDts: number;
    videoBaseMediaDecodeTime: number | undefined;
    audioAppendStartTs: number;
    keepOriginalTimestamps: boolean;
  }): Mp4AudioSegmentData | null {
    if (this._aacFrames.length === 0) {
      return null;
    }

    const {
      keepOriginalTimestamps,
      earliestAllowedDts,
      videoBaseMediaDecodeTime,
      audioAppendStartTs,
    } = args;

    const frames = trimAdtsFramesByEarliestDts(
      this._aacFrames,
      this._trackInfo,
      earliestAllowedDts,
    );

    if (frames.length === 0) {
      this._aacFrames = [];
      clearDtsInfo(this._trackInfo);
      return null;
    }

    this._trackInfo.baseMediaDecodeTime = calculateTrackBaseMediaDecodeTime(
      this._trackInfo,
      keepOriginalTimestamps,
    );

    if (videoBaseMediaDecodeTime !== undefined) {
      // amount of audio filled but the value is in video clock rather than audio clock
      prefixWithSilence(
        this._trackInfo,
        frames,
        audioAppendStartTs,
        videoBaseMediaDecodeTime,
      );
    }

    // we have to build the index from byte locations to
    // samples (that is, adts frames) in the audio data
    this._trackInfo.samples = generateAudioSampleTable(frames);

    this._aacFrames = [];

    const moof = createMoof(this._sequenceNumber, [this._trackInfo]);

    // bump the sequence number for next time
    this._sequenceNumber++;

    const continuityStart = this._trackInfo.baseMediaDecodeTime;
    const duration = (this._trackInfo.samples ?? []).reduce(
      (acc: number, sample: { duration: number }) => acc + sample.duration,
      0,
    );
    const continuityEnd = continuityStart + duration;

    clearDtsInfo(this._trackInfo);

    return {
      trackInfo: this._trackInfo,
      fragment: createFragment(
        moof,
        frames.map((frame) => frame.data),
        false,
      ),
      timingInfo: {
        start: continuityStart,
        end: continuityEnd,
        timescale: this._trackInfo.samplerate,
      },
    };
  }

  public cancel(): void {
    clearDtsInfo(this._trackInfo);
    this._aacFrames = [];
  }
}
