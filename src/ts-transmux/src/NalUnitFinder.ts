/** Split Annex B bytes into NAL units, independently of input chunk boundaries. */
export default class NalUnitFinder {
  private _buffer: Uint8Array | null = null;
  private _cursor = 0;
  private _nalStart: number | undefined;
  private _zeroCount = 0;

  public findNext(data: Uint8Array): Uint8Array[] {
    if (data.length === 0) {
      return [];
    }
    let buffer: Uint8Array;
    if (this._buffer === null) {
      buffer = data;
    } else {
      buffer = new Uint8Array(this._buffer.length + data.length);
      buffer.set(this._buffer);
      buffer.set(data, this._buffer.length);
    }

    const units: Uint8Array[] = [];
    let cursor = this._cursor;
    let nalStart = this._nalStart;
    let zeroCount = this._zeroCount;
    while (cursor < buffer.length) {
      if (zeroCount === 0) {
        // Skip payload bytes in native code. Only zero runs can start a delimiter.
        const zero = buffer.indexOf(0, cursor);
        if (zero === -1) {
          cursor = buffer.length;
          break;
        }
        cursor = zero + 1;
        zeroCount = 1;
        continue;
      }
      const byte = buffer[cursor++];
      if (byte === 0) {
        zeroCount++;
        continue;
      }
      if (byte === 1 && zeroCount >= 2) {
        const end = cursor - 1 - zeroCount;
        if (nalStart !== undefined && end > nalStart) {
          units.push(buffer.subarray(nalStart, end));
        }
        nalStart = cursor;
      }
      zeroCount = 0;
    }

    // Keep the unfinished NAL, or only a possible delimiter before the first NAL.
    // In particular, a zero run at a chunk boundary is not yet a delimiter.
    const retainedStart = nalStart ?? cursor - zeroCount;
    this._buffer =
      retainedStart === buffer.length ? null : buffer.subarray(retainedStart);
    this._cursor = cursor - retainedStart;
    this._nalStart = nalStart === undefined ? undefined : 0;
    this._zeroCount = zeroCount;
    return units;
  }

  /** Emit unfinished NAL bytes, including an unconfirmed trailing zero run. */
  public flush(): Uint8Array | null {
    const unit =
      this._nalStart !== undefined &&
      this._buffer !== null &&
      this._buffer.length > this._nalStart
        ? this._buffer.subarray(this._nalStart)
        : null;
    this.reset();
    return unit;
  }

  public reset(): void {
    this._buffer = null;
    this._cursor = 0;
    this._nalStart = undefined;
    this._zeroCount = 0;
  }
}
