import logger from "../../ts-common/logger";
import { parseSyncSafeInteger, frameParsers } from "./id3-utils";
import { TimedMetadataPacket } from "./read-aac";

const METADATA_STREAM_TYPE = 0x15;

export interface ParsedTimedMetadata {
  data: Uint8Array;
  frames: any[];
  pts: number;
  dts: number;
}

export default class TimedMetadataParser {
  public dispatchType: string;

  private _descriptor: number[] | undefined;
  // the total size in bytes of the ID3 tag being parsed
  private _tagSize: number;
  // tag data that is not complete enough to be parsed
  private _buffer: TimedMetadataPacket[];
  // the total number of bytes currently in the buffer
  private _bufferSize: number;
  private _onTimestamp: ((val: any) => void) | null;

  constructor(
    onTimestamp: ((val: any) => void) | null,
    options?: { descriptor?: number[] | undefined } | undefined,
  ) {
    this._onTimestamp = onTimestamp;
    this._descriptor = options?.descriptor;
    this._tagSize = 0;
    this._buffer = [];
    this._bufferSize = 0;

    // calculate the text track in-band metadata track dispatch type
    // eslint-disable-next-line max-len
    // https://html.spec.whatwg.org/multipage/embedded-content.html#steps-to-expose-a-media-resource-specific-text-track
    this.dispatchType = METADATA_STREAM_TYPE.toString(16);
    if (this._descriptor !== undefined) {
      this._descriptor.forEach((desc) => {
        this.dispatchType += ("00" + desc.toString(16)).slice(-2);
      });
    }
  }

  public parsePacket(packet: TimedMetadataPacket): ParsedTimedMetadata | null {
    // if data_alignment_indicator is set in the PES header,
    // we must have the start of a new ID3 tag. Assume anything
    // remaining in the buffer was malformed and throw it out
    if (packet.dataAlignmentIndicator === true) {
      this._bufferSize = 0;
      this._buffer.length = 0;
    }

    // ignore packets that don't look like ID3 data
    if (
      this._buffer.length === 0 &&
      (packet.data.length < 10 ||
        packet.data[0] !== "I".charCodeAt(0) ||
        packet.data[1] !== "D".charCodeAt(0) ||
        packet.data[2] !== "3".charCodeAt(0))
    ) {
      logger.warn("Transmuxer: Skipping unrecognized metadata packet");
      return null;
    }

    // add this packet to the data we've collected so far

    this._buffer.push(packet);
    this._bufferSize += packet.data.byteLength;

    // grab the size of the entire frame from the ID3 header
    if (this._buffer.length === 1) {
      // the frame size is transmitted as a 28-bit integer in the
      // last four bytes of the ID3 header.
      // The most significant bit of each byte is dropped and the
      // results concatenated to recover the actual value.
      this._tagSize = parseSyncSafeInteger(packet.data.subarray(6, 10));

      // ID3 reports the tag size excluding the header but it's more
      // convenient for our comparisons to include it
      this._tagSize += 10;
    }

    // if the entire frame has not arrived, wait for more data
    if (this._bufferSize < this._tagSize) {
      return null;
    }

    // collect the entire frame so it can be parsed
    const tag: ParsedTimedMetadata = {
      data: new Uint8Array(this._tagSize),
      frames: [],

      // TODO?
      pts: (this._buffer[0] as any).pts,
      dts: (this._buffer[0] as any).dts,
    };
    for (let i = 0; i < this._tagSize; ) {
      tag.data.set(this._buffer[0].data.subarray(0, this._tagSize - i), i);
      i += this._buffer[0].data.byteLength;
      this._bufferSize -= this._buffer[0].data.byteLength;
      this._buffer.shift();
    }

    // find the start of the first frame and the end of the tag
    let frameStart = 10;
    if (tag.data[5] & 0x40) {
      // advance the frame start past the extended header
      frameStart += 4; // header size field
      frameStart += parseSyncSafeInteger(tag.data.subarray(10, 14));

      // clip any padding off the end
      this._tagSize -= parseSyncSafeInteger(tag.data.subarray(16, 20));
    }

    let frameSize: number;

    // parse one or more ID3 frames
    // http://id3.org/id3v2.3.0#ID3v2_frame_overview
    do {
      // determine the number of bytes in this frame
      frameSize = parseSyncSafeInteger(
        tag.data.subarray(frameStart + 4, frameStart + 8),
      );
      if (frameSize < 1) {
        logger.warn(
          "Transmuxer: ",
          "Malformed ID3 frame encountered. Skipping remaining metadata parsing.",
        );
        // If the frame is malformed, don't parse any further frames but allow
        // previous valid parsed frames to be sent along.
        break;
      }
      const frameHeader = String.fromCharCode(
        tag.data[frameStart],
        tag.data[frameStart + 1],
        tag.data[frameStart + 2],
        tag.data[frameStart + 3],
      );

      const frame: any = {
        id: frameHeader,
        data: tag.data.subarray(frameStart + 10, frameStart + frameSize + 10),
        key: frameHeader,
      };

      // parse frame values
      if (frameParsers[frame.id as keyof typeof frameParsers] !== undefined) {
        // use frame specific parser
        frameParsers[frame.id as keyof typeof frameParsers](frame);
      } else if (frame.id[0] === "T") {
        // use text frame generic parser
        frameParsers["T*"](frame);
      } else if (frame.id[0] === "W") {
        // use URL link frame generic parser
        frameParsers["W*"](frame);
      }

      // handle the special PRIV frame used to indicate the start
      // time for raw AAC data
      if (frame.owner === "com.apple.streaming.transportStreamTimestamp") {
        const d = frame.data;
        let size =
          ((d[3] & 0x01) << 30) |
          (d[4] << 22) |
          (d[5] << 14) |
          (d[6] << 6) |
          (d[7] >>> 2);

        size *= 4;
        size += d[7] & 0x03;
        frame.timeStamp = size;
        // in raw AAC, all subsequent data will be timestamped based
        // on the value of this frame
        // we couldn't have known the appropriate pts and dts before
        // parsing this ID3 tag so set those values now
        if (tag.pts === undefined && tag.dts === undefined) {
          tag.pts = frame.timeStamp;
          tag.dts = frame.timeStamp;
        }
        if (this._onTimestamp !== null) {
          this._onTimestamp(frame);
        }
      }

      tag.frames.push(frame);

      frameStart += 10; // advance past the frame header
      frameStart += frameSize; // advance past the frame body
    } while (frameStart < this._tagSize);
    return tag;
  }

  public reset() {
    this._tagSize = 0;
    this._buffer = [];
    this._bufferSize = 0;
  }
}
