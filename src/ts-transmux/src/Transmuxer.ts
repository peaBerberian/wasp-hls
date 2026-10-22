import { isLikelyAacData } from "./aac-utils.ts";
import AdtsPacketParser from "./AdtsPacketParser.ts";
import { toVideoTsFromTimescale } from "./clock-utils.ts";
import type { ElementaryPacket } from "./ElementaryPacketParser.ts";
import ElementaryPacketParser from "./ElementaryPacketParser.ts";
import FullMp4SegmentConstructor from "./FullMp4SegmentConstructor.ts";
import H264NalUnitProducer from "./H264NalUnitProducer.ts";
import { Mp4AudioSegmentGenerator } from "./Mp4AudioSegmentGenerator.ts";
import Mp4VideoSegmentGenerator from "./Mp4VideoSegmentGenerator.ts";
import { readNextAdtsOrId3 } from "./read-aac.ts";
import TimedMetadataParser from "./TimedMetadataParser.ts";
import TimestampRolloverHandler from "./TimestampRolloverHandler.ts";
import TransportPacketParser from "./TransportPacketParser.ts";
import TransportStreamSplitter from "./TransportStreamSplitter.ts";

interface AacPipelineElements {
  name: "aac";
  adtsParser: AdtsPacketParser;
  audioTimestampRolloverHandler: TimestampRolloverHandler;
  getAdtsTimestamp(): number;
  metadataParser: TimedMetadataParser;
  mp4AudioSegmentGenerator: Mp4AudioSegmentGenerator | null;
  mp4SegmentConstructor: FullMp4SegmentConstructor;
  timedMetadataRolloverHandler: TimestampRolloverHandler;
}

interface TsPipelineElements {
  name: "ts";
  adtsParser: AdtsPacketParser;
  elementaryPacketParser: ElementaryPacketParser;
  h264NalUnitProducer: H264NalUnitProducer;
  metadataParser: TimedMetadataParser;
  mp4AudioSegmentGenerator: Mp4AudioSegmentGenerator | null;
  mp4SegmentConstructor: FullMp4SegmentConstructor;
  mp4VideoSegmentGenerator: Mp4VideoSegmentGenerator | null;
  timestampRolloverHandler: TimestampRolloverHandler;
  transportPacketParser: TransportPacketParser;
  transportStreamSplitter: TransportStreamSplitter;
}

/** Contrustor options for `Transmuxer`. */
export interface TransmuxerOptions {
  /** Decode-time seed used until the transmuxer can infer continuity itself. */
  baseMediaDecodeTimeSeed?: number;
  keepOriginalTimestamps?: boolean;
  firstSequenceNumber?: number;
  alignGopsAtEnd?: boolean;
}

/** Extracted timing information from a segment. */
export interface TransmuxedSegmentTimingInfo {
  /** Its start time, in timescale. */
  start: number;
  /** Its end time, in timescale. */
  end: number;
  /** Timescale used for `start` and `end`. */
  timescale: number;
}

/** Context given to the transmuxer when transmuxing a segment. */
export interface TransmuxSegmentOptions {
  /** If `true`, this segment is non-contiguous with the previous one. */
  reset?: boolean;
  /** Decode-time seed to use when the transmuxer has no prior DTS continuity. */
  baseMediaDecodeTimeSeed?: {
    /** Timestamp value in the associated timescale. */
    value: number;
    /** Timescale used for that timestamp. */
    timescale: number;
  };
}

/** Output from the transmuxer. */
export interface TransmuxedSegmentData {
  /** The transmuxed fmp4, in the flesh. */
  data: Uint8Array;
  /**
   * Precise timing information for continuity decisions:
   * - If segment is muxed audio + video, this corresponds to the video track
   *   and should be reused as the continuity anchor for the next segment.
   * - If segment is only audio or only video, this corresponds to that track.
   */
  timingInfo: TransmuxedSegmentTimingInfo | undefined;
}

export default class Transmuxer {
  private _options: TransmuxerOptions;
  private _videoTrack: any;
  private _audioTrack: any;
  private _baseMediaDecodeTimeSeed: number;
  private _currentPipeline: AacPipelineElements | TsPipelineElements | null;
  private _pendingSegmentBaseMediaDecodeTimeSeed: number | null;

  constructor(options?: TransmuxerOptions | undefined) {
    this._options = options ?? {};
    this._baseMediaDecodeTimeSeed = options?.baseMediaDecodeTimeSeed ?? 0;
    this._currentPipeline = null;
    this._videoTrack = null;
    this._audioTrack = null;
    this._pendingSegmentBaseMediaDecodeTimeSeed = null;
  }

  public transmuxSegment(
    data: Uint8Array,
    options: TransmuxSegmentOptions = {},
  ): TransmuxedSegmentData | null {
    if (options?.reset === true) {
      this.reset();
    }
    this._prepareSegmentTimeline(options.baseMediaDecodeTimeSeed);
    const isAac = isLikelyAacData(data);
    if (isAac && this._currentPipeline?.name !== "aac") {
      this._setupAacPipeline();
    } else if (!isAac && this._currentPipeline?.name !== "ts") {
      this._setupTsPipeline();
    }
    if (this._currentPipeline?.name === "aac") {
      return this.pushAacSegment(data);
    } else {
      return this.pushTsSegment(data);
    }
  }

  public reset(): void {
    if (this._currentPipeline?.name === "ts") {
      this._currentPipeline.mp4VideoSegmentGenerator?.cancel();
      this._currentPipeline.mp4AudioSegmentGenerator?.cancel();
      this._currentPipeline.mp4SegmentConstructor.cancel();
    } else if (this._currentPipeline?.name === "aac") {
      this._currentPipeline.mp4AudioSegmentGenerator?.cancel();
      this._currentPipeline.mp4SegmentConstructor.cancel();
    }
    this._currentPipeline = null;
    this._videoTrack = null;
    this._audioTrack = null;
    this._pendingSegmentBaseMediaDecodeTimeSeed = null;
  }

  public pushTsSegment(input: Uint8Array): TransmuxedSegmentData | null {
    let pipeline = this._currentPipeline;
    if (pipeline === null || pipeline.name !== "ts") {
      pipeline = this._setupTsPipeline();
    }

    pipeline.transportStreamSplitter.feed(input);
    while (true) {
      const [transportPacket, isEnded] =
        pipeline.transportStreamSplitter.readNextPacket();
      const tsPackets =
        transportPacket === null
          ? []
          : pipeline.transportPacketParser.parse(transportPacket);
      for (const tsPacket of tsPackets) {
        const ePckt = pipeline.elementaryPacketParser.readNextPacket(tsPacket);
        if (ePckt === null) {
          continue;
        }
        this._onElementaryStreamTsPacket(ePckt, pipeline);
      }
      if (isEnded) {
        // We've finished reading the segment!

        // Flush `ElementaryPacketParser` for any remaining packet
        const ePckts = pipeline.elementaryPacketParser.flush();
        pipeline.elementaryPacketParser.reset();
        for (const ePckt of ePckts) {
          this._onElementaryStreamTsPacket(ePckt, pipeline);
        }

        // Now do the same thing with the `H264NalUnitProducer`
        const nalUnit = pipeline.h264NalUnitProducer.flush();
        if (nalUnit !== null && pipeline.mp4VideoSegmentGenerator !== null) {
          pipeline.mp4VideoSegmentGenerator.pushNalUnit(nalUnit);
        }

        // Because we're only parsing whole segments for now, reset the
        // the pipeline as a security.
        pipeline.adtsParser.reset();
        pipeline.elementaryPacketParser.reset();
        pipeline.h264NalUnitProducer.reset();
        pipeline.metadataParser.reset();
        pipeline.timestampRolloverHandler.signalEndOfSegment();
        pipeline.transportPacketParser.reset();
        pipeline.transportStreamSplitter.reset();
        return this._buildSegment();
      }
    }
  }

  public pushAacSegment(input: Uint8Array): TransmuxedSegmentData | null {
    let pipeline = this._currentPipeline;
    if (pipeline === null || pipeline.name !== "aac") {
      pipeline = this._setupAacPipeline();
    }

    const onSegmentParsed = (): TransmuxedSegmentData | null => {
      if (pipeline === null || pipeline.name !== "aac") {
        return null;
      }

      // Because we're only parsing whole segments for now, reset the
      // the pipeline at the end of each segment as a security.
      pipeline.adtsParser.reset();
      pipeline.audioTimestampRolloverHandler.signalEndOfSegment();
      pipeline.metadataParser.reset();
      pipeline.timedMetadataRolloverHandler.signalEndOfSegment();
      return this._buildSegment();
    };

    let remainingInput: Uint8Array | null = input;
    while (true) {
      if (remainingInput === null) {
        return onSegmentParsed();
      }
      const [packet, remainingBuffer] = readNextAdtsOrId3(
        remainingInput,
        pipeline.getAdtsTimestamp(),
      );
      remainingInput = remainingBuffer;
      if (packet === null) {
        return onSegmentParsed();
      }

      if (packet.type === "timed-metadata") {
        // TODO `as any` needed because a TimedMetadata packet never has any
        // `pts` or `dts`. This would be a bug, but it is there in the original
        // mux.js code. To check.
        pipeline.timedMetadataRolloverHandler.correctTimestamps(packet as any);
        const parsed = pipeline.metadataParser.parsePacket(packet);
        pipeline.mp4SegmentConstructor.pushSegment(parsed);
      } else {
        pipeline.audioTimestampRolloverHandler.correctTimestamps(packet);
        const aacFrames = pipeline.adtsParser.parsePacket(packet);
        if (pipeline.mp4AudioSegmentGenerator === null) {
          this._audioTrack = this._audioTrack ?? {
            // Raw AAC has one track and no PMT to supply its MP4 track ID.
            id: 1,
            timelineStartInfo: {
              baseMediaDecodeTime: this._getCurrentBaseMediaDecodeTime(),
            },
            codec: "adts",
            type: "audio",
          };
          pipeline.mp4AudioSegmentGenerator = new Mp4AudioSegmentGenerator(
            this._audioTrack,
            this._options.firstSequenceNumber,
          );
        }
        for (const aacFrame of aacFrames) {
          pipeline.mp4AudioSegmentGenerator.pushAacFrame(aacFrame);
        }
      }
    }
  }

  private _setupTsPipeline(): TsPipelineElements {
    const timestampRolloverHandler = new TimestampRolloverHandler();
    const adtsParser = new AdtsPacketParser();
    const metadataParser = new TimedMetadataParser(null);
    const mp4SegmentConstructor = new FullMp4SegmentConstructor(
      this._options.keepOriginalTimestamps === true,
      metadataParser.dispatchType,
    );
    const elementaryPacketParser = new ElementaryPacketParser();
    const transportStreamSplitter = new TransportStreamSplitter();
    const h264NalUnitProducer = new H264NalUnitProducer();
    const transportPacketParser = new TransportPacketParser();
    this._currentPipeline = {
      name: "ts",
      timestampRolloverHandler,
      adtsParser,
      metadataParser,
      mp4SegmentConstructor,
      mp4AudioSegmentGenerator: null,
      mp4VideoSegmentGenerator: null,
      transportStreamSplitter,
      elementaryPacketParser,
      h264NalUnitProducer,
      transportPacketParser,
    };
    return this._currentPipeline;
  }

  private _setupAacPipeline(): AacPipelineElements {
    let adtsTimestamp = 0;
    const audioTimestampRolloverHandler = new TimestampRolloverHandler();
    const timedMetadataRolloverHandler = new TimestampRolloverHandler();
    const adtsParser = new AdtsPacketParser();
    const metadataParser = new TimedMetadataParser((val) => {
      adtsTimestamp = val.timeStamp;
    });
    const mp4SegmentConstructor = new FullMp4SegmentConstructor(
      this._options.keepOriginalTimestamps === true,
      metadataParser.dispatchType,
    );
    this._currentPipeline = {
      name: "aac",
      getAdtsTimestamp(): number {
        return adtsTimestamp;
      },
      audioTimestampRolloverHandler,
      timedMetadataRolloverHandler,
      adtsParser,
      metadataParser,
      mp4SegmentConstructor,
      mp4AudioSegmentGenerator: null,
    };
    return this._currentPipeline;
  }

  /**
   * Generate ISOBMFF boxes and coalesce them into one big file from
   * everything parsed until now.
   * @returns {TransmuxedSegmentData|null}
   */
  private _buildSegment(): TransmuxedSegmentData | null {
    const pipeline = this._currentPipeline;
    let videoBaseMediaDecodeTime: number | undefined;
    let earliestAllowedDts = 0;
    let continuityTiming: TransmuxedSegmentTimingInfo | undefined;

    if (pipeline?.name === "ts" && pipeline.mp4VideoSegmentGenerator !== null) {
      const videoSegmentData =
        pipeline.mp4VideoSegmentGenerator.generateBoxes();
      if (videoSegmentData !== null) {
        const { trackInfo, timingInfo } = videoSegmentData;
        videoBaseMediaDecodeTime = trackInfo.baseMediaDecodeTime;
        continuityTiming = timingInfo;
        if (this._options.keepOriginalTimestamps !== true) {
          const { timelineStartInfo, minSegmentDts, baseMediaDecodeTime } =
            trackInfo;
          if (this._audioTrack !== null) {
            this._audioTrack.timelineStartInfo = timelineStartInfo;
            if (
              minSegmentDts !== undefined &&
              baseMediaDecodeTime !== undefined
            ) {
              // Keep audio from starting before the current segment's decode
              // timeline floor. When baseMediaDecodeTime gets clamped to 0 for
              // an earlier segment, that floor must move with that segment
              // instead of staying tied to the first post-reset DTS.
              earliestAllowedDts = minSegmentDts - baseMediaDecodeTime;
            }
          }
        }
        pipeline.mp4SegmentConstructor.pushSegment(videoSegmentData);
      }
    }
    if (pipeline !== null && pipeline.mp4AudioSegmentGenerator !== null) {
      const audioSegmentData = pipeline.mp4AudioSegmentGenerator.generateBoxes({
        audioAppendStartTs: 0,
        keepOriginalTimestamps: this._options.keepOriginalTimestamps === true,
        earliestAllowedDts,
        videoBaseMediaDecodeTime,
      });
      if (audioSegmentData !== null) {
        continuityTiming ??= audioSegmentData.timingInfo;
        pipeline.mp4SegmentConstructor.pushSegment(audioSegmentData);
      }
    }

    if (pipeline?.mp4SegmentConstructor !== undefined) {
      const segmentInfo = pipeline.mp4SegmentConstructor.finishSegment();
      this._pendingSegmentBaseMediaDecodeTimeSeed = null;
      if (segmentInfo === null) {
        return null;
      }
      if (segmentInfo.data === null || segmentInfo.data.byteLength === 0) {
        return null;
      }
      return {
        data: segmentInfo.data,
        timingInfo: continuityTiming,
      };
    }
    return null;
  }

  private _onElementaryStreamTsPacket(
    ePckt: ElementaryPacket,
    pipeline: TsPipelineElements,
  ) {
    if (ePckt.type !== "metadata") {
      pipeline.timestampRolloverHandler.correctTimestamps(ePckt);
      if (ePckt.type === "video") {
        const nalUnits = pipeline.h264NalUnitProducer.pushPacket(ePckt);
        if (pipeline.mp4VideoSegmentGenerator !== null) {
          for (const nalUnit of nalUnits) {
            pipeline.mp4VideoSegmentGenerator.pushNalUnit(nalUnit);
          }
        }
      } else if (ePckt.type === "audio") {
        if (pipeline.mp4AudioSegmentGenerator !== null) {
          // TODO `as any` needed because there's a very specific case where
          // `pts` and `dts` would not be defined. To check if that's a real
          // bug.
          const frames = pipeline.adtsParser.parsePacket(ePckt as any);
          for (const frame of frames) {
            pipeline.mp4AudioSegmentGenerator.pushAacFrame(frame);
          }
        }
      } else if (ePckt.type === "timed-metadata") {
        const parsed = pipeline.metadataParser.parsePacket(ePckt);
        pipeline.mp4SegmentConstructor.pushSegment(parsed);
      }
    } else {
      let i = ePckt.tracks.length;

      // scan the tracks listed in the metadata
      while (i--) {
        if (this._videoTrack === null && ePckt.tracks[i].type === "video") {
          this._videoTrack = ePckt.tracks[i];
          this._videoTrack.timelineStartInfo.baseMediaDecodeTime =
            this._getCurrentBaseMediaDecodeTime();
        } else if (
          this._audioTrack === null &&
          ePckt.tracks[i].type === "audio"
        ) {
          this._audioTrack = ePckt.tracks[i];
          this._audioTrack.timelineStartInfo.baseMediaDecodeTime =
            this._getCurrentBaseMediaDecodeTime();
        }
      }

      if (
        this._videoTrack !== null &&
        pipeline.mp4VideoSegmentGenerator === null
      ) {
        pipeline.mp4VideoSegmentGenerator = new Mp4VideoSegmentGenerator(
          this._videoTrack,
          this._options,
        );
      }

      if (
        this._audioTrack !== null &&
        pipeline.mp4AudioSegmentGenerator === null
      ) {
        pipeline.mp4AudioSegmentGenerator = new Mp4AudioSegmentGenerator(
          this._audioTrack,
          this._options.firstSequenceNumber,
        );
      }
    }
  }

  private _prepareSegmentTimeline(
    baseMediaDecodeTimeSeed:
      | {
          value: number;
          timescale: number;
        }
      | undefined,
  ): void {
    if (baseMediaDecodeTimeSeed === undefined) {
      return;
    }
    const baseMediaDecodeTimeSeedVideoTs = toVideoTsFromTimescale(
      baseMediaDecodeTimeSeed.value,
      baseMediaDecodeTimeSeed.timescale,
    );
    this._pendingSegmentBaseMediaDecodeTimeSeed =
      baseMediaDecodeTimeSeedVideoTs;
  }

  private _getStoredBaseMediaDecodeTime(): number | undefined {
    const tracks = [this._videoTrack, this._audioTrack];
    for (const track of tracks) {
      const storedBaseMediaDecodeTime =
        track?.timelineStartInfo?.baseMediaDecodeTime;
      if (storedBaseMediaDecodeTime !== undefined) {
        return storedBaseMediaDecodeTime;
      }
    }
    return undefined;
  }

  private _getCurrentBaseMediaDecodeTime(): number {
    const storedBaseMediaDecodeTime = this._getStoredBaseMediaDecodeTime();
    if (storedBaseMediaDecodeTime !== undefined) {
      return storedBaseMediaDecodeTime;
    }
    if (this._pendingSegmentBaseMediaDecodeTimeSeed === null) {
      return this._baseMediaDecodeTimeSeed;
    }
    return this._pendingSegmentBaseMediaDecodeTimeSeed;
  }
}
