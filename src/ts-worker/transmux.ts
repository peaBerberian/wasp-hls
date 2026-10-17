import logger from "../ts-common/logger.ts";
import Transmuxer from "../ts-transmux/index.ts";
import { MediaType } from "../wasm/index.js";
import { canTransmux } from "./utils.js";

function normalizeLegacyAvc1Codec(mimeType: string): string {
  const match = /avc1\.(66|77|100)\.(\d+)/.exec(mimeType);
  if (match === null) {
    return mimeType;
  }

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
  return mimeType.replace(match[0], `avc1.${newProfile}${newLevel}`);
}

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
  return normalizeLegacyAvc1Codec(mimeType);
}

export function getFmp4Type(mediaType: MediaType, codecP: string): string {
  const codec = codecP.trim();
  let mimeTypePrefix: string;
  switch (mediaType) {
    case MediaType.Audio:
      mimeTypePrefix = "audio/";
      break;
    case MediaType.Video:
      mimeTypePrefix = "video/";
      break;
    case MediaType.Text:
      mimeTypePrefix = "application/";
      break;
    default:
      logger.error("Unknown MediaType");
      mimeTypePrefix = "video/";
      break;
  }
  return normalizeLegacyAvc1Codec(`${mimeTypePrefix}mp4;codecs="${codec}"`);
}

export function createTransmuxer(): Transmuxer {
  return new Transmuxer();
}
