import type { MediaType } from "../../wasm/index.js";
import { SourceBufferCreationErrorCode } from "../../wasm/index.js";
import type { WaspErrorCode } from "./common.ts";

export default class WaspSourceBufferCreationError extends Error {
  public readonly name: "WaspSourceBufferCreationError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "SourceBufferAlreadyCreatedWithSameType"
    | "SourceBufferCantPlayType"
    | "SourceBufferEmptyMimeType"
    | "SourceBufferMediaSourceIsClosed"
    | "SourceBufferNoMediaSourceAttached"
    | "SourceBufferQuotaExceededError"
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
    message?: string | undefined,
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferCreationError.prototype);

    this.name = "WaspSourceBufferCreationError";
    this.mediaType = mediaType;
    switch (code) {
      case SourceBufferCreationErrorCode.AlreadyCreatedWithSameType:
        this.code = "SourceBufferAlreadyCreatedWithSameType";
        break;
      case SourceBufferCreationErrorCode.CantPlayType:
        this.code = "SourceBufferCantPlayType";
        break;
      case SourceBufferCreationErrorCode.EmptyMimeType:
        this.code = "SourceBufferEmptyMimeType";
        break;
      case SourceBufferCreationErrorCode.MediaSourceIsClosed:
        this.code = "SourceBufferMediaSourceIsClosed";
        break;
      case SourceBufferCreationErrorCode.NoMediaSourceAttached:
        this.code = "SourceBufferNoMediaSourceAttached";
        break;
      case SourceBufferCreationErrorCode.QuotaExceededError:
        this.code = "SourceBufferQuotaExceededError";
        break;
      case SourceBufferCreationErrorCode.Unknown:
        this.code = "SourceBufferCreationOtherError";
        break;
      default:
        this.code = "SourceBufferCreationOtherError";
    }
    this.globalCode = this.code;
    this.message = message ?? "Unknown error when creating SourceBuffer";
  }
}
