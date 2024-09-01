/**
 * Parser for exponential Golomb codes, a variable-bitwidth number encoding
 * scheme used by h264.
 * @class ExpGolomb
 */
class ExpGolomb {
  private _workingData: Uint8Array;
  // the number of bytes left to examine in workingData
  private _workingBytesAvailable: number;
  // the current word being examined
  private _workingWord: number;
  // the number of bits left to examine in the current word
  private _workingBitsAvailable: number;

  /**
   * @param {Uint8Array} workingData
   */
  constructor(workingData: Uint8Array) {
    this._workingData = workingData;
    this._workingBytesAvailable = workingData.byteLength;
    this._workingWord = 0;
    this._workingBitsAvailable = 0;
    this.loadWord();
  }

  public length(): number {
    return 8 * this._workingBytesAvailable;
  }

  public bitsAvailable(): number {
    return 8 * this._workingBytesAvailable + this._workingBitsAvailable;
  }

  public loadWord(): void {
    const position = this._workingData.byteLength - this._workingBytesAvailable;
    const workingBytes = new Uint8Array(4);
    const availableBytes = Math.min(4, this._workingBytesAvailable);

    if (availableBytes === 0) {
      throw new Error("no bytes available");
    }

    workingBytes.set(
      this._workingData.subarray(position, position + availableBytes),
    );
    this._workingWord = new DataView(workingBytes.buffer).getUint32(0);

    // track the amount of workingData that has been processed
    this._workingBitsAvailable = availableBytes * 8;
    this._workingBytesAvailable -= availableBytes;
  }

  public skipBits(count: number): void {
    if (this._workingBitsAvailable > count) {
      this._workingWord <<= count;
      this._workingBitsAvailable -= count;
    } else {
      let usedCount = count;
      usedCount -= this._workingBitsAvailable;
      const skipBytes = Math.floor(usedCount / 8);
      usedCount -= skipBytes * 8;
      this._workingBytesAvailable -= skipBytes;
      this.loadWord();
      this._workingWord <<= usedCount;
      this._workingBitsAvailable -= usedCount;
    }
  }

  public readBits(size: number): number {
    let bits = Math.min(this._workingBitsAvailable, size);
    const valu = this._workingWord >>> (32 - bits);
    // if size > 31, handle error
    this._workingBitsAvailable -= bits;
    if (this._workingBitsAvailable > 0) {
      this._workingWord <<= bits;
    } else if (this._workingBytesAvailable > 0) {
      this.loadWord();
    }

    bits = size - bits;
    if (bits > 0) {
      return (valu << bits) | this.readBits(bits);
    }
    return valu;
  }

  public skipLeadingZeros(): number {
    let leadingZeroCount = 0;
    for (; leadingZeroCount < this._workingBitsAvailable; ++leadingZeroCount) {
      if ((this._workingWord & (0x80000000 >>> leadingZeroCount)) !== 0) {
        // the first bit of working word is 1
        this._workingWord <<= leadingZeroCount;
        this._workingBitsAvailable -= leadingZeroCount;
        return leadingZeroCount;
      }
    }

    // we exhausted workingWord and still have not found a 1
    this.loadWord();
    return leadingZeroCount + this.skipLeadingZeros();
  }

  public skipUnsignedExpGolomb(): void {
    this.skipBits(1 + this.skipLeadingZeros());
  }

  public skipExpGolomb(): void {
    this.skipBits(1 + this.skipLeadingZeros());
  }

  public readUnsignedExpGolomb(): number {
    const clz = this.skipLeadingZeros();
    return this.readBits(clz + 1) - 1;
  }

  public readExpGolomb(): number {
    const valu = this.readUnsignedExpGolomb(); // :int
    if (0x01 & valu) {
      // the number is odd if the low order bit is set
      return (1 + valu) >>> 1; // add 1 to make it even, and divide by 2
    }
    return -1 * (valu >>> 1); // divide by two then make it negative
  }

  public readBoolean(): boolean {
    return this.readBits(1) === 1;
  }

  public readUnsignedByte(): number {
    return this.readBits(8);
  }
}

export default ExpGolomb;
