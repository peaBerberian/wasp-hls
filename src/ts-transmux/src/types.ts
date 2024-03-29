export interface TrackSampleFlags {
  paddingValue: number;
  isNonSyncSample: number;
  isLeading: number;
  degradationPriority: number;
  isDependedOn: number;
  dependsOn: number;
  hasRedundancy: number;
}

export interface TrackSample {
  duration: number;
  size: number;
  flags?: TrackSampleFlags;
  compositionTimeOffset?: number;
}

export interface TrackInfo {
  id: number;
  type: "audio" | "video";
  baseMediaDecodeTime: number;
  audioobjecttype: number;
  samplingfrequencyindex: number;
  channelcount: number;
  samplesize: number;
  duration: number;
  samplerate: number;
  samples: TrackSample[] | undefined;
  sps: Uint8Array[] | undefined;
  pps: Uint8Array[] | undefined;
  width: number;
  height: number;
  profileIdc: number;
  profileCompatibility: number;
  levelIdc: number;
  sarRatio: [number, number] | undefined;

  pts: number | undefined;
  dts: number | undefined;
  minSegmentPts: number;
  minSegmentDts: number;
  maxSegmentPts: number;
  maxSegmentDts: number;

  timelineStartInfo: {
    baseMediaDecodeTime: number | undefined;
    pts: number | undefined;
    dts: number | undefined;
  };
}
