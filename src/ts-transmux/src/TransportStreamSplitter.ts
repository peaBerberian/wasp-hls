const PAT_PID = 0x0000;
const MP2T_PACKET_LENGTH = 188;
const SYNC_BYTE = 0x47;

/**
 * Splits an incoming stream of binary data into single MPEG-2 Transport
 * Stream packets.
 * @class TransportPacketStream
 */
export default class TransportStreamSplitter {
  private _input: Uint8Array | null;
  private _incompletePacketBuffer: Uint8Array;
  private _bytesInIncompletePacketBuffer: number;
  private _startIndex: number;
  private _endIndex: number;

  constructor() {
    this._input = null;
    this._incompletePacketBuffer = new Uint8Array(MP2T_PACKET_LENGTH);
    this._bytesInIncompletePacketBuffer = 0;
    this._startIndex = 0;
    this._endIndex = MP2T_PACKET_LENGTH;
  }

  public feed(bytes: Uint8Array): void {
    // If there are bytes remaining from the last segment, prepend them to the
    // bytes that were pushed in
    if (this._bytesInIncompletePacketBuffer > 0) {
      const newInput = new Uint8Array(
        bytes.byteLength + this._bytesInIncompletePacketBuffer
      );
      newInput.set(
        this._incompletePacketBuffer.subarray(
          0,
          this._bytesInIncompletePacketBuffer
        )
      );
      newInput.set(bytes, this._bytesInIncompletePacketBuffer);
      this._input = newInput;
      this._bytesInIncompletePacketBuffer = 0;
    } else {
      this._input = bytes;
    }

    this._startIndex = 0;
    this._endIndex = MP2T_PACKET_LENGTH;
  }

  public readNextPacket(): [Uint8Array | null, boolean] {
    if (this._input === null) {
      return [null, true];
    }

    // While we have enough data for a packet
    while (this._endIndex < this._input.byteLength) {
      // Look for a pair of start and end sync bytes in the data..
      if (
        this._input[this._startIndex] === SYNC_BYTE &&
        this._input[this._endIndex] === SYNC_BYTE
      ) {
        // We found a packet so emit it and jump one whole packet forward
        const data = this._input.subarray(this._startIndex, this._endIndex);
        this._startIndex += MP2T_PACKET_LENGTH;
        this._endIndex += MP2T_PACKET_LENGTH;
        const isEnded = this._startIndex >= this._input.byteLength;
        return [data, isEnded];
      }
      // If we get here, we have somehow become de-synchronized and we need to step
      // forward one byte at a time until we find a pair of sync bytes that denote
      // a packet
      this._startIndex++;
      this._endIndex++;
    }

    // If there was some data left over at the end of the segment that couldn't
    // possibly be a whole packet, keep it because it might be the start of a packet
    // that continues in the next segment
    if (this._startIndex < this._input.byteLength) {
      this._incompletePacketBuffer.set(
        this._input.subarray(this._startIndex),
        0
      );
      this._bytesInIncompletePacketBuffer =
        this._input.byteLength - this._startIndex;
    }
    // If the buffer contains a whole packet when we are being flushed, emit it
    // and empty the buffer. Otherwise hold onto the data because it may be
    // important for decoding the next segment
    if (
      this._bytesInIncompletePacketBuffer === MP2T_PACKET_LENGTH &&
      this._incompletePacketBuffer[0] === SYNC_BYTE
    ) {
      this._bytesInIncompletePacketBuffer = 0;
      this._input = null;
      return [this._incompletePacketBuffer, true];
    }
    return [null, true];
  }

  public reset(): void {
    this._input = null;
    this._bytesInIncompletePacketBuffer = 0;
    this._startIndex = 0;
    this._endIndex = MP2T_PACKET_LENGTH;
  }
}

export { PAT_PID, MP2T_PACKET_LENGTH };
