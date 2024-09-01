import { InitializationErrorCode } from "../../ts-common/types";
import { WaspErrorCode } from "./common";

/**
 * Error used when the `WaspHlsPlayer` initialization (e.g. through the
 * `initialize` method) fails.
 * @class WaspInitializationError
 */
export default class WaspInitializationError extends Error {
  /** Identifies a `WaspInitializationError` */
  public readonly name: "WaspInitializationError";

  /** Specifies the exact error encountered. */
  public readonly code:
    | "AlreadyInitializedError"
    | "WasmRequestError"
    | "UnknownInitializationError";

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
   * Optionally, if the error is due to an HTTP request on error, this is set to
   * the HTTP status of the request.
   */
  public readonly wasmHttpStatus: number | undefined;

  /** Human-readable message describing the error. */
  public readonly message: string;

  /**
   * @param {number} reason
   * @param {number|undefined} wasmHttpStatus
   * @param {string} message
   */
  constructor(
    reason: InitializationErrorCode,
    wasmHttpStatus: number | undefined,
    message: string,
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspInitializationError.prototype);

    this.name = "WaspInitializationError";
    switch (reason) {
      case InitializationErrorCode.AlreadyInitializedError:
        this.code = WaspErrorCode.AlreadyInitializedError;
        break;
      case InitializationErrorCode.WasmRequestError:
        this.code = WaspErrorCode.WasmRequestError;
        break;
      case InitializationErrorCode.UnknownError:
        this.code = WaspErrorCode.UnknownInitializationError;
        break;
    }
    this.globalCode = this.code;
    this.wasmHttpStatus = wasmHttpStatus;
    this.message = message;
  }
}
