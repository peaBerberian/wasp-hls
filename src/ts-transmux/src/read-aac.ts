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
