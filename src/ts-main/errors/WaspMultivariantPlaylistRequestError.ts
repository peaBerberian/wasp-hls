import { RequestErrorReason } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

/**
 * Error used to describe a problem with the Multivariant Playlist request.
 * @class WaspMultivariantPlaylistRequestError
 */
export default class WaspMultivariantPlaylistRequestError extends Error {
  /** Identifies a `WaspMultivariantPlaylistRequestError` */
  public readonly name: "WaspMultivariantPlaylistRequestError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Optionally set to the URL of the Multivariant Playlist requested. */
  public readonly url: string | undefined;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "MultivariantPlaylistBadHttpStatus"
    | "MultivariantPlaylistRequestTimeout"
    | "MultivariantPlaylistRequestError"
    | "MultivariantPlaylistRequestOtherError";

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
   * @param {string|undefined} url
   * @param {number} reason
   * @param {number|undefined} status
   * @param {string|undefined} [message]
   */
  constructor(
    url: string | undefined,
    reason: RequestErrorReason,
    status: number | undefined,
    message?: string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspMultivariantPlaylistRequestError.prototype);

    this.name = "WaspMultivariantPlaylistRequestError";
    this.url = url;
    let formattedMsg = message;
    switch (reason) {
      case RequestErrorReason.Status:
        formattedMsg =
          formattedMsg ??
          (status === undefined
            ? "A Multivariant Playlist HTTP(S) request(s) " +
              "responded with an invalid status"
            : "A Multivariant Playlist HTTP(S) request(s) " +
              `responded with a ${status} status`);
        this.code = WaspErrorCode.MultivariantPlaylistBadHttpStatus;
        break;

      case RequestErrorReason.Timeout:
        formattedMsg =
          formattedMsg ??
          "A Multivariant Playlist HTTP(S) request(s) did not respond";
        this.code = WaspErrorCode.MultivariantPlaylistRequestTimeout;
        break;

      case RequestErrorReason.Error:
        formattedMsg =
          formattedMsg ??
          "A Multivariant Playlist HTTP(S) request(s) failed due to an error.";
        this.code = WaspErrorCode.MultivariantPlaylistRequestError;
        break;

      case RequestErrorReason.Other:
        formattedMsg =
          formattedMsg ??
          "A Multivariant Playlist HTTP(S) request(s) failed for an unknown reason.";
        this.code = WaspErrorCode.MultivariantPlaylistRequestOtherError;
        break;

      default:
        this.code = WaspErrorCode.MultivariantPlaylistRequestOtherError;
        break;
    }
    this.globalCode = this.code;
    this.message =
      formattedMsg ??
      "An error arised while trying to perform a segment request";
  }
}
