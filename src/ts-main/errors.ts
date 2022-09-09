import { InitializationErrorCode } from "../ts-common/types";

export default class InitializationError extends Error {
  public readonly name: "InitializationError";
  public readonly code: InitializationErrorCode;
  public readonly wasmHttpStatus: number | undefined;
  public readonly message : string;

  /**
   * @param {string} message
   */
  constructor(
    code: InitializationErrorCode,
    wasmHttpStatus: number | undefined,
    message : string
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, InitializationError.prototype);

    this.name = "InitializationError";
    this.code = code;
    this.wasmHttpStatus = wasmHttpStatus;
    this.message = message;
  }
}
