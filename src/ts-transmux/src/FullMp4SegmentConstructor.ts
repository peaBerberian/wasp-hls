import * as clock from "./clock-utils";
import { createInitSegment } from "./mp4-utils";

interface ConstructedMp4Segment {
  type: string | undefined;
  captions: any[];
  metadata: any;
  captionStreams: any;
  info: any;
  initSegment: Uint8Array | null;
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
  private pendingBoxes: any[];
  private pendingCaptions: any[];
  private pendingMetadata: any[];
  private pendingBytes: number;

  constructor(keepOriginalTimestamps: boolean, metadataDispatchType: string) {
    this.metadataDispatchType = metadataDispatchType;
    this.keepOriginalTimestamps = keepOriginalTimestamps;
    this.pendingTracks = [];
    this.videoTrack = null;
    this.pendingBoxes = [];
    this.pendingCaptions = [];
    this.pendingMetadata = [];
    this.pendingBytes = 0;
    this.audioTrack = null;
  }

  public pushSegment(output: any): undefined {
    // buffer incoming captions until the associated video segment
    // finishes
    if (output.text != null) {
      this.pendingCaptions.push(output);
      return;
    }
    // buffer incoming id3 tags until the final flush
    if (output.frames != null) {
      this.pendingMetadata.push(output);
      return;
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

  public finishSegment(): ConstructedMp4Segment | null {
    let offset = 0;
    const seg: ConstructedMp4Segment = {
      type: undefined,
      captions: [],
      captionStreams: {},
      metadata: [],
      info: {},
      initSegment: null,
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
    if (this.videoTrack != null) {
      timelineStartPts = this.videoTrack.timelineStartInfo.pts;
      VIDEO_PROPERTIES.forEach((prop) => {
        seg.info[prop] = this.videoTrack[prop];
      }, this);
    } else if (this.audioTrack != null) {
      timelineStartPts = this.audioTrack.timelineStartInfo.pts;
      AUDIO_PROPERTIES.forEach((prop) => {
        seg.info[prop] = this.audioTrack[prop];
      }, this);
    }

    if (this.videoTrack != null || this.audioTrack != null) {
      if (this.pendingTracks.length === 1) {
        seg.type = this.pendingTracks[0].type;
      } else {
        seg.type = "combined";
      }

      const initSegment = createInitSegment(this.pendingTracks);

      // Create a new typed array to hold the init segment
      seg.initSegment = new Uint8Array(initSegment.byteLength);

      // Create an init segment containing a moov
      // and track definitions
      seg.initSegment.set(initSegment);

      // Append each moof+mdat (one per track) together
      seg.data = this.pendingBoxes.reduce((acc, box) => {
        acc.set(box, offset);
        offset += box.byteLength;
        return acc;
      }, new Uint8Array(this.pendingBytes));

      if (timelineStartPts !== undefined) {
        const timelinePts: number = timelineStartPts;

        // Translate caption PTS times into second offsets to match the
        // video timeline for the segment, and add track info
        seg.captions = this.pendingCaptions.map((caption) => {
          caption.startTime = clock.metadataTsToSeconds(
            caption.startPts,
            timelinePts,
            this.keepOriginalTimestamps
          );
          caption.endTime = clock.metadataTsToSeconds(
            caption.endPts,
            timelinePts,
            this.keepOriginalTimestamps
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
            this.keepOriginalTimestamps
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
      this.pendingBoxes.length = 0;
      this.pendingCaptions.length = 0;
      this.pendingBytes = 0;
      this.pendingMetadata.length = 0;

      return seg;
    }
    return null;
  }

  public cancel(): void {
    this.pendingTracks.length = 0;
    this.videoTrack = null;
    this.audioTrack = null;
    this.pendingBoxes.length = 0;
    this.pendingCaptions.length = 0;
    this.pendingBytes = 0;
    this.pendingMetadata.length = 0;
  }
}
