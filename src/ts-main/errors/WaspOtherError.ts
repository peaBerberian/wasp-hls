import { OtherErrorCode } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

/**
 * Error used when an uncategorized error arised.
 * @class WaspOtherError
 */
export default class WaspOtherError extends Error {
  /** Identifies a `WaspOtherError` */
  public readonly name: "WaspOtherError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "MediaSourceAttachmentError"
    | "NoSupportedVariant"
    | "UnfoundLockedVariant"
    | "Unknown";

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
   * @param {number} reason
   * @param {string} message
   */
  constructor(reason: OtherErrorCode, message?: string | undefined) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspOtherError.prototype);

    this.name = "WaspOtherError";
    switch (reason) {
      case OtherErrorCode.MediaSourceAttachmentError:
        this.code = WaspErrorCode.MediaSourceAttachmentError;
        break;
      case OtherErrorCode.UnfoundLockedVariant:
        this.code = WaspErrorCode.UnfoundLockedVariant;
        break;
      case OtherErrorCode.NoSupportedVariant:
        this.code = WaspErrorCode.NoSupportedVariant;
        break;
      default:
        this.code = WaspErrorCode.Unknown;
        break;
    }
    this.globalCode = this.code;
    this.message = message ?? "Unknown error";
  }
}
