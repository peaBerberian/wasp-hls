import { MediaType, SourceBufferCreationErrorCode } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

export default class WaspSourceBufferCreationError extends Error {
  public readonly name: "WaspSourceBufferCreationError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "SourceBufferCantPlayType"
    | "SourceBufferCreationOtherError";

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
   * The media type associated to the `SourceBuffer` associated to this error.
   *
   * `undefined` if unknown or if the concept cannot be applied here.
   */
  public readonly mediaType: MediaType | undefined;

  /**
   * @param {number} code
   * @param {number} mediaType
   * @param {string} message
   */
  constructor(
    code: SourceBufferCreationErrorCode,
    mediaType: MediaType,
    message?: string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferCreationError.prototype);

    this.name = "WaspSourceBufferCreationError";
    this.mediaType = mediaType;
    switch (code) {
      case SourceBufferCreationErrorCode.CantPlayType:
        this.code = "SourceBufferCantPlayType";
        break;
      case SourceBufferCreationErrorCode.AlreadyCreatedWithSameType:
      case SourceBufferCreationErrorCode.EmptyMimeType:
      case SourceBufferCreationErrorCode.MediaSourceIsClosed:
      case SourceBufferCreationErrorCode.NoMediaSourceAttached:
      case SourceBufferCreationErrorCode.QuotaExceededError:
      case SourceBufferCreationErrorCode.Unknown:
        this.code = "SourceBufferCantPlayType";
        break;
      default:
        // WHY TYPESCRIPT? I've done all MFing cases on purpose
        this.code = "SourceBufferCreationOtherError";
    }
    this.globalCode = this.code;
    this.message = message ?? "Unknown error when creating SourceBuffer";
  }
}
