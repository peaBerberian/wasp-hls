import { WaspErrorCode } from "./common.ts";

/** A fatal error reported by the browser's media element. */
export default class WaspMediaError extends Error {
  public readonly name: "WaspMediaError";
  public readonly code:
    | "MediaAbortedError"
    | "MediaNetworkError"
    | "MediaDecodeError"
    | "MediaSourceNotSupportedError"
    | "MediaOtherError";
  public readonly globalCode: keyof typeof WaspErrorCode;
  public readonly message: string;
  /** Original HTMLMediaElement.error.code. */
  public readonly mediaErrorCode: number;

  constructor(error: MediaError) {
    super();
    Object.setPrototypeOf(this, WaspMediaError.prototype);
    this.name = "WaspMediaError";
    this.mediaErrorCode = error.code;
    let fallbackMessage: string;
    switch (error.code) {
      case 1:
        this.code = WaspErrorCode.MediaAbortedError;
        fallbackMessage = "Loading of the media element's resource was aborted";
        break;
      case 2:
        this.code = WaspErrorCode.MediaNetworkError;
        fallbackMessage = "The media element reported a network error";
        break;
      case 3:
        this.code = WaspErrorCode.MediaDecodeError;
        fallbackMessage = "The browser could not decode media data";
        break;
      case 4:
        this.code = WaspErrorCode.MediaSourceNotSupportedError;
        fallbackMessage =
          "The media element could not use the supplied media resource";
        break;
      default:
        this.code = WaspErrorCode.MediaOtherError;
        fallbackMessage = "The browser reported an unknown media error";
        break;
    }
    this.globalCode = this.code;
    this.message = error.message || fallbackMessage;
  }
}
