import EventEmitter from "../../ts-common/EventEmitter";
import { parseAdtsSize, parseId3TagSize } from "./aac-utils";

export interface TimedMetadataPacket {
  type: "timed-metadata";
  data: Uint8Array;
  dataAlignmentIndicator?: boolean;
}

export interface AudioPacket {
  type: "audio";
  data: Uint8Array;
  pts: number;
  dts: number;
}

export interface AacStreamEvents {
  data: TimedMetadataPacket | AudioPacket;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * TODO?
 */
export function readNextAdtsOrId3(
  input: Uint8Array,
  timestamp: number
): [AudioPacket | TimedMetadataPacket | null, Uint8Array | null] {
  let frameSize = 0;
  let byteIndex = 0;

  while (input.length - byteIndex >= 3) {
    if (
      input[byteIndex] === "I".charCodeAt(0) &&
      input[byteIndex + 1] === "D".charCodeAt(0) &&
      input[byteIndex + 2] === "3".charCodeAt(0)
    ) {
      // Exit early if we don't have enough to parse the ID3 tag header
      if (input.length - byteIndex < 10) {
        break;
      }

      // check framesize
      frameSize = parseId3TagSize(input, byteIndex);

      // Exit early if we don't have enough in the buffer to emit a full packet
      // Add to byteIndex to support multiple ID3 tags in sequence
      if (byteIndex + frameSize > input.length) {
        break;
      }
      const chunk = {
        type: "timed-metadata",
        data: input.subarray(byteIndex, byteIndex + frameSize),
      } as const;
      byteIndex += frameSize;
      const bytesLeft = input.length - byteIndex;
      if (bytesLeft > 0) {
        return [chunk, input.subarray(byteIndex)];
      } else {
        return [chunk, null];
      }
    } else if (
      (input[byteIndex] & 0xff) === 0xff &&
      (input[byteIndex + 1] & 0xf0) === 0xf0
    ) {
      // Exit early because we don't have enough to parse
      // the ADTS frame header
      if (input.length - byteIndex < 7) {
        break;
      }

      frameSize = parseAdtsSize(input, byteIndex);

      // Exit early if we don't have enough in the buffer
      // to emit a full packet
      if (byteIndex + frameSize > input.length) {
        break;
      }

      const packet = {
        type: "audio",
        data: input.subarray(byteIndex, byteIndex + frameSize),
        pts: timestamp,
        dts: timestamp,
      } as const;
      byteIndex += frameSize;
      const bytesLeft = input.length - byteIndex;
      if (bytesLeft > 0) {
        return [packet, input.subarray(byteIndex)];
      } else {
        return [packet, null];
      }
    }
    byteIndex++;
  }
  const bytesLeft = input.length - byteIndex;
  if (bytesLeft > 0) {
    return [null, input.subarray(byteIndex)];
  } else {
    return [null, null];
  }
}

/**
 * Splits an incoming stream of binary data into ADTS and ID3 Frames.
 */
class AacStream extends EventEmitter<AacStreamEvents> {
  private _everything: Uint8Array;
  private _timestamp: number;

  constructor() {
    super();
    this._everything = new Uint8Array();
    this._timestamp = 0;
  }

  public setTimestamp(timestamp: number): void {
    this._timestamp = timestamp;
  }

  public push(bytes: Uint8Array): void {
    let frameSize = 0;
    let byteIndex = 0;

    // If there are bytes remaining from the last segment, prepend them to the
    // bytes that were pushed in
    if (this._everything.length > 0) {
      const tempLength = this._everything.length;
      this._everything = new Uint8Array(bytes.byteLength + tempLength);
      this._everything.set(this._everything.subarray(0, tempLength));
      this._everything.set(bytes, tempLength);
    } else {
      this._everything = bytes;
    }

    const everything = this._everything;
    while (everything.length - byteIndex >= 3) {
      if (
        everything[byteIndex] === "I".charCodeAt(0) &&
        everything[byteIndex + 1] === "D".charCodeAt(0) &&
        everything[byteIndex + 2] === "3".charCodeAt(0)
      ) {
        // Exit early if we don't have enough to parse the ID3 tag header
        if (everything.length - byteIndex < 10) {
          break;
        }

        // check framesize
        frameSize = parseId3TagSize(everything, byteIndex);

        // Exit early if we don't have enough in the buffer to emit a full packet
        // Add to byteIndex to support multiple ID3 tags in sequence
        if (byteIndex + frameSize > everything.length) {
          break;
        }
        const chunk = {
          type: "timed-metadata",
          data: everything.subarray(byteIndex, byteIndex + frameSize),
        } as const;
        this.trigger("data", chunk);
        byteIndex += frameSize;
        continue;
      } else if (
        (everything[byteIndex] & 0xff) === 0xff &&
        (everything[byteIndex + 1] & 0xf0) === 0xf0
      ) {
        // Exit early because we don't have enough to parse
        // the ADTS frame header
        if (everything.length - byteIndex < 7) {
          break;
        }

        frameSize = parseAdtsSize(everything, byteIndex);

        // Exit early if we don't have enough in the buffer
        // to emit a full packet
        if (byteIndex + frameSize > everything.length) {
          break;
        }

        const packet = {
          type: "audio",
          data: everything.subarray(byteIndex, byteIndex + frameSize),
          pts: this._timestamp,
          dts: this._timestamp,
        } as const;
        this.trigger("data", packet);
        byteIndex += frameSize;
        continue;
      }
      byteIndex++;
    }
    const bytesLeft = everything.length - byteIndex;
    if (bytesLeft > 0) {
      this._everything = everything.subarray(byteIndex);
    } else {
      this._everything = new Uint8Array();
    }
  }

  public flush(): void {
    this.trigger("done", null);
  }

  public partialFlush(): void {
    this.trigger("partialdone", null);
  }

  public reset(): void {
    this._everything = new Uint8Array();
    this.trigger("reset", null);
  }

  public endTimeline(): void {
    this._everything = new Uint8Array();
    this.trigger("endedtimeline", null);
  }
}

export default AacStream;
