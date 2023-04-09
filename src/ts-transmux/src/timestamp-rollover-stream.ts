import EventEmitter from "../../ts-common/EventEmitter";

const MAX_TS = 8589934592;

const RO_THRESH = 4294967296;

const TYPE_SHARED = "shared";

function handleRollover(value: number, reference: number): number {
  let direction = 1;
  let resValue = value;

  if (resValue > reference) {
    // If the current timestamp value is greater than our reference timestamp
    // and we detect a timestamp rollover, this means the roll over is happening
    // in the opposite direction.
    // Example scenario: Enter a long stream/video just after a rollover
    // occurred. The reference point will be set to a small number, e.g. 1. The
    // user then seeks backwards over the rollover point. In loading this
    // segment, the timestamp values will be very large, e.g. 2^33 - 1.
    // Since this comes before the data we loaded previously, we want to adjust
    // the time stamp to be `resValue - 2^33`.
    direction = -1;
  }

  // Note: A seek forwards or back that is greater than the RO_THRESH (2^32,
  // ~13 hours) will cause an incorrect adjustment.
  while (Math.abs(reference - resValue) > RO_THRESH) {
    resValue += direction * MAX_TS;
  }

  return resValue;
}

export interface TimestampRolloverStreamEvents {
  data: {
    pts: number;
    dts: number;
    type: "audio" | "video" | "shared" | "timed-metadata";
  };
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * TODO?
 */
export class TimestampRolloverHandler {
  /** Last `dts` originally encountered in an input. */
  private _lastDts: number | null;
  /**
   * `dts` used as a reference to detect if a rollover has happened.
   *
   * Set to the original `dts` encountered and successive `dts` while parsing a
   * media segment, then set to the last corrected `dts` value once that segment
   * has been fully parsed.
   */
  private _referenceDts: number | null;

  constructor() {
    this._lastDts = null;
    this._referenceDts = null;
  }

  public correctTimestamps<T extends {
    type: string;
    dts: number;
    pts: number
  }>(data: T): void {
    if (this._referenceDts === null) {
      this._referenceDts = data.dts;
    }

    data.dts = handleRollover(data.dts, this._referenceDts);
    data.pts = handleRollover(data.pts, this._referenceDts);

    this._lastDts = data.dts;
  }

  public signalEndOfSegment(): void {
    this._referenceDts = this._lastDts;
  }
}

// export function handleTimestampRolloverForType<T extends {
//   type: string;
//   dts: number;
//   pts: number
// }>(
//   streamType: "audio" | "video" | "shared" | "timed-metadata" = "shared",
//   data: T[],
//   referenceDts: number | null
// ): [T[], number | null] {
//   let usedReference = referenceDts;
//   let lastDts: number | null = null;
//   const res: T[] = [];
//   for (const item of data) {
//     // Any "shared" rollover streams will accept _all_ data. Otherwise,
//     // streams will only accept data that matches their type.
//     if (streamType !== "shared" && item.type !== streamType) {
//       continue;
//     }

//     if (usedReference === null) {
//       usedReference = item.dts;
//     }

//     item.dts = handleRollover(item.dts, usedReference);
//     item.pts = handleRollover(item.pts, usedReference);

//     lastDts = item.dts;
//     res.push(item);
//   }
//   return [res, lastDts];
// }

class TimestampRolloverStream extends EventEmitter<TimestampRolloverStreamEvents> {
  private _lastDts: number | null;
  private _referenceDts: number | null;

  // The "shared" type is used in cases where a stream will contain muxed
  // video and audio. We could use `undefined` here, but having a string
  // makes debugging a little clearer.
  private _streamType: "audio" | "video" | "shared" | "timed-metadata";

  constructor(streamType?: "audio" | "video" | "shared" | "timed-metadata") {
    super();
    this._streamType = streamType ?? TYPE_SHARED;
    this._lastDts = null;
    this._referenceDts = null;
  }

  public push(data: any): void {
    // Any "shared" rollover streams will accept _all_ data. Otherwise,
    // streams will only accept data that matches their type.
    if (this._streamType !== TYPE_SHARED && data.type !== this._streamType) {
      return;
    }

    if (this._referenceDts === null) {
      this._referenceDts = data.dts as number;
    }

    data.dts = handleRollover(data.dts, this._referenceDts);
    data.pts = handleRollover(data.pts, this._referenceDts);

    this._lastDts = data.dts;

    this.trigger("data", data);
  }

  public flush(): void {
    this._referenceDts = this._lastDts;
    this.trigger("done", null);
  }

  public partialFlush(): void {
    this.trigger("partialdone", null);
  }

  public endTimeline(): void {
    this.flush();
    this.trigger("endedtimeline", null);
  }
  public discontinuity(): void {
    this._referenceDts = null;
    this._lastDts = null;
  }

  public reset(): void {
    this.discontinuity();
    this.trigger("reset", null);
  }
}

export default TimestampRolloverStream;
export { handleRollover };
