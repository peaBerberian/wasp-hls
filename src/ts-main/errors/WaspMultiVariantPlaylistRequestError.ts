import { RequestErrorReason } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

/**
 * Error used to describe a problem with the MultiVariant Playlist request.
 * @class WaspMultiVariantPlaylistRequestError
 */
export default class WaspMultiVariantPlaylistRequestError extends Error {
  /** Identifies a `WaspMultiVariantPlaylistRequestError` */
  public readonly name: "WaspMultiVariantPlaylistRequestError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Optionally set to the URL of the MultiVariant Playlist requested. */
  public readonly url: string | undefined;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "MultiVariantPlaylistBadHttpStatus"
    | "MultiVariantPlaylistRequestTimeout"
    | "MultiVariantPlaylistRequestError"
    | "MultiVariantPlaylistRequestOtherError";

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
    Object.setPrototypeOf(this, WaspMultiVariantPlaylistRequestError.prototype);

    this.name = "WaspMultiVariantPlaylistRequestError";
    this.url = url;
    let formattedMsg = message;
    switch (reason) {
      case RequestErrorReason.Status:
        formattedMsg =
          formattedMsg ??
          (status === undefined
            ? "A MultiVariant Playlist HTTP(S) request(s) " +
              "responded with an invalid status"
            : `A MultiVariant Playlist HTTP(S) request(s) " +
            "responded with a ${status} status`);
        this.code = WaspErrorCode.MultiVariantPlaylistBadHttpStatus;
        break;

      case RequestErrorReason.Timeout:
        formattedMsg =
          formattedMsg ??
          "A MultiVariant Playlist HTTP(S) request(s) did not respond";
        this.code = WaspErrorCode.MultiVariantPlaylistRequestTimeout;
        break;

      case RequestErrorReason.Error:
        formattedMsg =
          formattedMsg ??
          "A MultiVariant Playlist HTTP(S) request(s) failed due to an error.";
        this.code = WaspErrorCode.MultiVariantPlaylistRequestError;
        break;

      case RequestErrorReason.Other:
        formattedMsg =
          formattedMsg ??
          "A MultiVariant Playlist HTTP(S) request(s) failed for an unknown reason.";
        this.code = WaspErrorCode.MultiVariantPlaylistRequestOtherError;
        break;

      default:
        this.code = WaspErrorCode.MultiVariantPlaylistRequestOtherError;
        break;
    }
    this.globalCode = this.code;
    this.message =
      formattedMsg ??
      "An error arised while trying to perform a segment request";
  }
}
