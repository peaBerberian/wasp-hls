import logger from "../../ts-common/logger";
import { ONE_SECOND_IN_TS } from "./clock-utils";

const ADTS_SAMPLING_FREQUENCIES = [
  96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025,
  8000, 7350,
];

export interface AdtsPacket {
  type: string;
  pts: number;
  dts: number;
  data: Uint8Array;
}

export interface AacFrame {
  pts: number;
  dts: number;
  sampleCount: number;
  audioobjecttype: number;
  channelcount: number;
  samplerate: number;
  samplingfrequencyindex: number;
  // assume ISO/IEC 14496-12 AudioSampleEntry default of 16
  samplesize: 16;
  // data is the frame without its header
  data: Uint8Array;
}

export interface UnprocessedAdtsPacketsData {
  buffer: Uint8Array | null;
  // TODO partial segments unclear yet
  // frameNumber: number
}

function skipWarn(start: number, end: number, frameNum: number): void {
  logger.warn(
    `Transmux: adts skiping bytes ${start} to ${end} ` +
      `in frame ${frameNum} outside syncword`,
  );
}

/**
 * Convert previously parsed ADTS packets into AAC frames.
 * @param {Array.<Object>} packets - The parsed ADTS packets.
 * @param {Object|null} previousUnfinishedState - If
 * `adtsPacketsToAacFrames` was already called earlier, it might have
 * remaining data that could not have been completely processed in that call.
 * This argument allows to re-communicate that data so it can use it as a base.
 * @returns {Array.<Object>} - Returns a tuple of two elements:
 *   1. The AAC frame parsed
 *   2. Metadata about the remaining data that could not have yet been
 *      processed.
 *      This is mainly useful when `adtsPacketsToAacFrames` is called with only
 *      partial data about a full media segment.
 *      If you're calling `adtsPacketsToAacFrames` multiple times with a subset
 *      of the data, you may want to re-use that metadata in the next
 *      `adtsPacketsToAacFrames` call for that same full media segment.
 */
export default class AdtsPacketParser {
  private _buffer: Uint8Array | null;
  // TODO partial segments unclear yet
  // private _frameNumber: number;

  constructor() {
    this._buffer = null;
  }

  public parsePacket(packet: AdtsPacket): AacFrame[] {
    let buffer = this._buffer;
    const frames: AacFrame[] = [];
    if (packet.type !== "audio") {
      // ignore non-audio data
      return [];
    }
    let frameNum = 0;

    // Prepend any data in the buffer to the input data so that we can parse
    // aac frames the cross a PES packet boundary
    if (buffer !== null && buffer.length > 0) {
      const oldBuffer = buffer;
      buffer = new Uint8Array(oldBuffer.byteLength + packet.data.byteLength);
      buffer.set(oldBuffer);
      buffer.set(packet.data, oldBuffer.byteLength);
    } else {
      buffer = packet.data;
    }

    // unpack any ADTS frames which have been fully received
    // for details on the ADTS header, see http://wiki.multimedia.cx/index.php?title=ADTS
    let skip: number | undefined;

    let i = 0;

    // We use i + 7 here because we want to be able to parse the entire header.
    // If we don't have enough bytes to do that, then we definitely won't have a
    // full frame.
    while (i + 7 < buffer.length) {
      // Look for the start of an ADTS header..
      if (buffer[i] !== 0xff || (buffer[i + 1] & 0xf6) !== 0xf0) {
        if (typeof skip !== "number") {
          skip = i;
        }
        // If a valid header was not found,  jump one forward and attempt to
        // find a valid ADTS header starting at the next byte
        i++;
        continue;
      }

      if (typeof skip === "number") {
        skipWarn(skip, i, frameNum);
        skip = undefined;
      }

      // The protection skip bit tells us if we have 2 bytes of CRC data at the
      // end of the ADTS header
      const protectionSkipBytes = (~buffer[i + 1] & 0x01) * 2;

      // Frame length is a 13 bit integer starting 16 bits from the
      // end of the sync sequence
      // NOTE: frame length includes the size of the header
      const frameLength =
        ((buffer[i + 3] & 0x03) << 11) |
        (buffer[i + 4] << 3) |
        ((buffer[i + 5] & 0xe0) >> 5);

      const sampleCount = ((buffer[i + 6] & 0x03) + 1) * 1024;
      const adtsFrameDuration =
        (sampleCount * ONE_SECOND_IN_TS) /
        ADTS_SAMPLING_FREQUENCIES[(buffer[i + 2] & 0x3c) >>> 2];

      // If we don't have enough data to actually finish this ADTS frame,
      // then we have to wait for more data
      if (buffer.byteLength - i < frameLength) {
        break;
      }

      // Otherwise, deliver the complete AAC frame
      frames.push({
        pts: packet.pts + frameNum * adtsFrameDuration,
        dts: packet.dts + frameNum * adtsFrameDuration,
        sampleCount,
        audioobjecttype: ((buffer[i + 2] >>> 6) & 0x03) + 1,
        channelcount:
          ((buffer[i + 2] & 1) << 2) | ((buffer[i + 3] & 0xc0) >>> 6),
        samplerate: ADTS_SAMPLING_FREQUENCIES[(buffer[i + 2] & 0x3c) >>> 2],
        samplingfrequencyindex: (buffer[i + 2] & 0x3c) >>> 2,
        // assume ISO/IEC 14496-12 AudioSampleEntry default of 16
        samplesize: 16,
        // data is the frame without its header
        data: buffer.subarray(i + 7 + protectionSkipBytes, i + frameLength),
      });

      frameNum++;
      i += frameLength;
    }

    if (typeof skip === "number") {
      skipWarn(skip, i, frameNum);
    }

    // remove processed bytes from the buffer.
    this._buffer = buffer.subarray(i);
    if (buffer.byteLength === 0) {
      this._buffer = null;
    }
    return frames;
  }

  public reset(): void {
    this._buffer = null;
  }
}
