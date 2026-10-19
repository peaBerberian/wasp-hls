import { TrackSelectionErrorCode } from "../../wasm/index.js";
import type { MediaType } from "../../wasm/index.js";
import { WaspErrorCode } from "./common.ts";

/** A requested track could not be selected. */
export default class WaspTrackSelectionError extends Error {
  public readonly name: "WaspTrackSelectionError";
  public readonly code: "UnknownTrack" | "NoCompatibleVariant";
  public readonly globalCode: keyof typeof WaspErrorCode;
  public readonly message: string;
  public readonly mediaType: MediaType;
  /** Requested track ID. */
  public readonly trackId: number;

  constructor(
    reason: TrackSelectionErrorCode,
    mediaType: MediaType,
    trackId: number,
    message?: string,
  ) {
    super();
    Object.setPrototypeOf(this, WaspTrackSelectionError.prototype);
    this.name = "WaspTrackSelectionError";
    this.code =
      reason === TrackSelectionErrorCode.NoCompatibleVariant
        ? WaspErrorCode.NoCompatibleVariant
        : WaspErrorCode.UnknownTrack;
    this.globalCode = this.code;
    this.mediaType = mediaType;
    this.trackId = trackId;
    this.message = message ?? "The requested track could not be selected";
  }
}
