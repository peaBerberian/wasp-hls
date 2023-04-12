const MAX_TS = 8589934592;
const RO_THRESH = 4294967296;

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

export default class TimestampRolloverHandler {
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

  public correctTimestamps<
    T extends {
      dts?: number;
      pts?: number;
    }
  >(data: T): void {
    if (data.dts === undefined) {
      return;
    }
    if (this._referenceDts === null) {
      this._referenceDts = data.dts;
    }
    if (data.pts !== undefined) {
      data.pts = handleRollover(data.pts, this._referenceDts);
    }
    data.dts = handleRollover(data.dts, this._referenceDts);
    this._lastDts = data.dts;
  }

  public signalEndOfSegment(): void {
    this._referenceDts = this._lastDts;
  }
}
