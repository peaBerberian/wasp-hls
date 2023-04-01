import { WaspErrorCode } from "./common";

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
    | "SourceBufferQuotaExceededError"
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
   * XXX TODO
   * @param {string|undefined} message
   */
  constructor(message?: string | undefined) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferError.prototype);

    this.name = "WaspSourceBufferError";
    this.code = "SourceBufferOtherError";
    this.globalCode = this.code;
    this.message = message ?? "Unknown error";
  }
}
