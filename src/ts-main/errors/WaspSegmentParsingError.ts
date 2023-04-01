import { AppendBufferErrorCode, MediaType } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

// TODO add `isInit` property?

/**
 * Error used to describe a problem when parsing a segment.
 * @class WaspSegmentParsingError
 */
export default class WaspSegmentParsingError extends Error {
  /** Identifies a `WaspSegmentParsingError` */
  public readonly name: "WaspSegmentParsingError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code: "SegmentTransmuxingError" | "SegmentParsingOtherError";

  /**
   * Specifies the exact error encountered.
   *
   * This is actually the same value as `code` but with a type common to all
   * `WaspHlsPlayer` Errors. The goal is to simplify your code would you ever
   * want to use the code without having to first check the `name` property in
   * your TypeScript code.
   */
  public readonly globalCode: keyof typeof WaspErrorCode;

  /**
   * The media type associated to the segment associated to this error.
   *
   * `undefined` if unknown or if the concept cannot be applied here.
   */
  public readonly mediaType: MediaType | undefined;

  /**
   * @param {number} reason
   * @param {number|undefined} mediaType
   * @param {string} message
   */
  constructor(
    reason: AppendBufferErrorCode,
    mediaType: MediaType | undefined,
    message?: string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSegmentParsingError.prototype);

    this.name = "WaspSegmentParsingError";
    switch (reason) {
      case AppendBufferErrorCode.TransmuxerError:
        this.code = "SegmentTransmuxingError";
        break;
      case AppendBufferErrorCode.NoResource:
      case AppendBufferErrorCode.NoSourceBuffer:
      case AppendBufferErrorCode.UnknownError:
        this.code = "SegmentParsingOtherError";
        break;
      default:
        this.code = "SegmentParsingOtherError";
        break;
    }
    this.globalCode = this.code;
    this.mediaType = mediaType;
    this.message = message ?? "Unknown error when parsing a segment";
  }
}
