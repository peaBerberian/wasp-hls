import logger from "../ts-common/logger.js";
import Transmuxer from "../ts-transmux";
import { MediaType } from "../wasm/wasp_hls.js";
import { canTransmux } from "./utils.js";

export function getTransmuxedType(typ: string, mediaType: MediaType): string {
  if (!canTransmux(typ)) {
    return typ;
  }
  if (typ.startsWith("audio/aac;")) {
    return typ.replace(/^audio\/aac;/i, "audio/mp4;");
  }
  let mimeType = typ.replace(/mp2t/i, "mp4");
  if (mediaType === MediaType.Audio) {
    mimeType = mimeType.replace(/video/i, "audio");
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
        logger.error("Impossible regex catch");
      }
      newProfile = "6400";
    }

    // Convert the level to hex and append to the codec string.
    const level = Number(match[2]);
    if (level >= 256) {
      logger.error("Invalid legacy avc1 level number.");
    }
    const newLevel = (level >> 4).toString(16) + (level & 0xf).toString(16);
    mimeType = `avc1.${newProfile}${newLevel}`;
  }
  return mimeType;
}

export function createTransmuxer(): Transmuxer {
  return new Transmuxer();
}
