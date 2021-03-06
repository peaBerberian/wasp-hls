// TODO Muxjs is not typed for now, just put any everywhere for the moment.
// We should find a better solution in the future (I'm not against writing my
// own transmuxer).
/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
/* eslint-disable @typescript-eslint/no-unsafe-call */
/* eslint-disable @typescript-eslint/no-unsafe-assignment */
/* eslint-disable @typescript-eslint/no-unsafe-argument */
/* eslint-disable @typescript-eslint/restrict-plus-operands */

import muxjs from "mux.js";
import { MediaType } from "../wasm/wasp_hls.js";

let transmuxer : any;

const MPEG_TS_REGEXP = /^[a-z]+\/mp2t;/i;
export function isMpegTsType(typ: string) : boolean {
  return MPEG_TS_REGEXP.test(typ);
}

export function shouldTransmux(typ: string) {
  return canTransmux(typ) &&
         !MediaSource.isTypeSupported(typ);
}

export function canTransmux(typ: string) : boolean {
  return isMpegTsType(typ);
}

export function getTransmuxedType(
  typ: string,
  mediaType: MediaType
) : string {
  if (!canTransmux(typ)) {
    return typ;
  }
  let mimeType = typ.replace(/mp2t/i, "mp4");
  if (mediaType === MediaType.Audio) {
    mimeType = typ.replace(/video/i, "audio");
  }

  // This is a workaround seen in the Shaka-player, which allows to play some
  // legacy HLS contents made that way for retro-compatibility-reasons:
  // Handle legacy AVC1 codec strings (pre-RFC 6381).
  // Look for "avc1.<profile>.<level>", where profile is:
  //   66 (baseline => 0x42)
  //   77 (main => 0x4d)
  //   100 (high => 0x64)
  // https://github.com/scheib/chromium/blob/b03fc92/media/base/video_codecs.cc#L356
  const match = /avc1\.(66|77|100)\.(\d+)/.exec(mimeType);
  if (match) {
    const profile = match[1];
    let newProfile;
    if (profile === "66") {
      newProfile = "4200";
    } else if (profile === "77") {
      newProfile = "4d00";
    } else {
      if (profile !== "100") {
        console.error("Impossible regex catch");
      }
      newProfile = "6400";
    }

    // Convert the level to hex and append to the codec string.
    const level = Number(match[2]);
    if (level >= 256) {
      console.error("Invalid legacy avc1 level number.");
    }
    const newLevel = (level >> 4).toString(16) + (level & 0xf).toString(16);
    mimeType = `avc1.${newProfile}${newLevel}`;
  }
  return mimeType;
}

export function transmux(
  inputSegment: Uint8Array
) : Uint8Array | null {
  if (transmuxer === undefined) {
    transmuxer = new muxjs.mp4.Transmuxer( );
  }

  const subSegments : Uint8Array[] = [];

  // NOTE: Despite the syntax, mux.js' transmuxing is completely synchronous.
  transmuxer.on("data", function (segment: any) {
    const transmuxedSegment = new Uint8Array(
      segment.initSegment.byteLength + segment.data.byteLength);
    transmuxedSegment.set(segment.initSegment, 0);
    transmuxedSegment.set(segment.data, segment.initSegment.byteLength);
    subSegments.push(transmuxedSegment);
  });

  transmuxer.push(inputSegment);
  transmuxer.flush();

  if (subSegments.length === 0) {
    return null;
  } else if (subSegments.length === 1) {
    return subSegments[0];
  } else {
    const segmentSize = subSegments.reduce((acc, s) => {
      return acc + s.byteLength;
    }, 0);

    const fullSegment = new Uint8Array(segmentSize);
    let currOffset = 0;
    for (const subSegment of subSegments) {
      fullSegment.set(subSegment, currOffset);
      currOffset += subSegment.byteLength;
    }
    return fullSegment;
  }
}
