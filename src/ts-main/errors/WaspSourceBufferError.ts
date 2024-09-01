import type { SourceBufferOperation } from "../../ts-common/QueuedSourceBuffer";
import type { MediaType } from "../../wasm/wasp_hls";
import { PushedSegmentErrorCode } from "../../wasm/wasp_hls";
import type { WaspErrorCode } from "./common";

/**
 * Error used when a SourceBuffer operation failed.
 * @class WaspSourceBufferError
 */
export default class WaspSourceBufferError extends Error {
  /** Identifies a `WaspSourceBufferError` */
  public readonly name: "WaspSourceBufferError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "SourceBufferFullError"
    | "SourceBufferAppendError"
    | "SourceBufferRemoveError"
    | "SourceBufferOtherError";

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
   * @param {number} operation
   * @param {number} reason
   * @param {number} mediaType
   * @param {string|undefined} message
   */
  constructor(
    operation: SourceBufferOperation.Push,
    reason: PushedSegmentErrorCode,
    mediaType: MediaType,
    message?: string | undefined,
  );
  constructor(
    operation: SourceBufferOperation.Remove,
    reason: null,
    mediaType: MediaType,
    message?: string | undefined,
  );
  constructor(
    _operation: SourceBufferOperation,
    reason: PushedSegmentErrorCode | null,
    mediaType: MediaType,
    message?: string | undefined,
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferError.prototype);
    this.name = "WaspSourceBufferError";
    this.mediaType = mediaType;

    switch (reason) {
      case null:
        this.code = "SourceBufferRemoveError";
        break;
      case PushedSegmentErrorCode.BufferFull:
        this.code = "SourceBufferFullError";
        break;
      case PushedSegmentErrorCode.UnknownError:
        this.code = "SourceBufferAppendError";
        break;
      default:
        this.code = "SourceBufferOtherError";
        break;
    }
    this.globalCode = this.code;
    this.message = message ?? "Unknown error";
  }
}
