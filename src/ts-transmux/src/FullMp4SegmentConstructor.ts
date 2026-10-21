import * as clock from "./clock-utils.ts";
import { assembleSegment, type Mp4Fragment } from "./mp4-fragment.ts";
import { createInitSegment } from "./mp4-utils.ts";

interface ConstructedMp4Segment {
  type: string | undefined;
  captions: any[];
  metadata: any;
  captionStreams: any;
  info: any;
  /** Complete init + audio/video media bytes. */
  data: Uint8Array | null;
}

/**
 * Combine multiple sub-segment parts (ie. audio & video) into a single ISOBMFF
 * segment for MSE.
 * Also supports audio-only and video-only segments.
 * @class FullMp4SegmentConstructor
 */
export default class FullMp4SegmentConstructor {
  private metadataDispatchType: string;
  private keepOriginalTimestamps: boolean;
  private pendingTracks: any[];
  private audioTrack: any;
  private videoTrack: any;
  private pendingFragments: Mp4Fragment[];
  private pendingCaptions: any[];
  private pendingMetadata: any[];

  constructor(keepOriginalTimestamps: boolean, metadataDispatchType: string) {
    this.metadataDispatchType = metadataDispatchType;
    this.keepOriginalTimestamps = keepOriginalTimestamps;
    this.pendingTracks = [];
    this.videoTrack = null;
    this.pendingFragments = [];
    this.pendingCaptions = [];
    this.pendingMetadata = [];
    this.audioTrack = null;
  }

  public pushSegment(output: any): undefined {
    // buffer incoming captions until the associated video segment
    // finishes
    if (output.text !== null && output.text !== undefined) {
      this.pendingCaptions.push(output);
      return;
    }
    // buffer incoming id3 tags until the final flush
    if (output.frames !== null && output.frames !== undefined) {
      this.pendingMetadata.push(output);
      return;
    }

    // Add this track to the list of pending tracks and store
    // important information required for the construction of
    // the final segment
    this.pendingTracks.push(output.trackInfo);

    // TODO: is there an issue for this against chrome?
    // We unshift audio and push video because
    // as of Chrome 75 when switching from
    // one init segment to another if the video
    // mdat does not appear after the audio mdat
    // only audio will play for the duration of our transmux.
    if (output.trackInfo.type === "video") {
      this.videoTrack = output.trackInfo;
      this.pendingFragments.push(output.fragment);
    }
    if (output.trackInfo.type === "audio") {
      this.audioTrack = output.trackInfo;
      this.pendingFragments.unshift(output.fragment);
    }
  }

  public finishSegment(): ConstructedMp4Segment | null {
    const seg: ConstructedMp4Segment = {
      type: undefined,
      captions: [],
      captionStreams: {},
      metadata: [],
      info: {},
      data: null,
    };
    const AUDIO_PROPERTIES = [
      "audioobjecttype",
      "channelcount",
      "samplerate",
      "samplingfrequencyindex",
      "samplesize",
    ] as const;

    const VIDEO_PROPERTIES = [
      "width",
      "height",
      "profileIdc",
      "levelIdc",
      "profileCompatibility",
      "sarRatio",
    ] as const;

    let timelineStartPts: number | undefined;
    if (this.videoTrack !== null && this.videoTrack !== undefined) {
      timelineStartPts = this.videoTrack.timelineStartInfo.pts;
      VIDEO_PROPERTIES.forEach((prop) => {
        seg.info[prop] = this.videoTrack[prop];
      }, this);
    } else if (this.audioTrack !== null && this.audioTrack !== undefined) {
      timelineStartPts = this.audioTrack.timelineStartInfo.pts;
      AUDIO_PROPERTIES.forEach((prop) => {
        seg.info[prop] = this.audioTrack[prop];
      }, this);
    }

    if (
      (this.videoTrack !== null && this.videoTrack !== undefined) ||
      (this.audioTrack !== null && this.audioTrack !== undefined)
    ) {
      if (this.pendingTracks.length === 1) {
        seg.type = this.pendingTracks[0].type;
      } else {
        seg.type = "combined";
      }

      const initSegment = createInitSegment(this.pendingTracks);

      seg.data = assembleSegment(initSegment, this.pendingFragments);

      if (timelineStartPts !== undefined) {
        const timelinePts: number = timelineStartPts;

        // Translate caption PTS times into second offsets to match the
        // video timeline for the segment, and add track info
        seg.captions = this.pendingCaptions.map((caption) => {
          caption.startTime = clock.metadataTsToSeconds(
            caption.startPts,
            timelinePts,
            this.keepOriginalTimestamps,
          );
          caption.endTime = clock.metadataTsToSeconds(
            caption.endPts,
            timelinePts,
            this.keepOriginalTimestamps,
          );

          seg.captionStreams[caption.stream] = true;
          return caption;
        });

        // Translate ID3 frame PTS times into second offsets to match the
        // video timeline for the segment
        seg.metadata = this.pendingMetadata.map((id3) => {
          id3.cueTime = clock.metadataTsToSeconds(
            id3.pts,
            timelinePts,
            this.keepOriginalTimestamps,
          );
          return id3;
        });
      }

      // We add this to every single emitted segment even though we only need
      // it for the first
      seg.metadata.dispatchType = this.metadataDispatchType;

      // Reset state
      this.pendingTracks.length = 0;
      this.videoTrack = null;
      this.audioTrack = null;
      this.pendingFragments.length = 0;
      this.pendingCaptions.length = 0;
      this.pendingMetadata.length = 0;

      return seg;
    }
    return null;
  }

  public cancel(): void {
    this.pendingTracks.length = 0;
    this.videoTrack = null;
    this.audioTrack = null;
    this.pendingFragments.length = 0;
    this.pendingCaptions.length = 0;
    this.pendingMetadata.length = 0;
  }
}
