import type { MediaType } from "../../wasm/wasp_hls";
import { RequestErrorReason } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

/**
 * Error used to describe a problem with the Media Playlist request.
 * @class WaspMediaPlaylistRequestError
 */
export default class WaspMediaPlaylistRequestError extends Error {
  /** Identifies a `WaspMediaPlaylistRequestError` */
  public readonly name: "WaspMediaPlaylistRequestError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Optionally set to the URL of the Media Playlist requested. */
  public readonly url: string | undefined;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "MediaPlaylistBadHttpStatus"
    | "MediaPlaylistRequestTimeout"
    | "MediaPlaylistRequestError"
    | "MediaPlaylistRequestOtherError";

  /**
   * The MediaType of the corresponding Media Playlist.
   * `undefined` if unknown.
   */
  public readonly mediaType: MediaType | undefined;

  /**
   * The failed request's HTTP(S) response status.
   *
   * `undefined` if unknown, not checked or if not yet received (e.g. in case of a
   * timeout).
   */
  public readonly status: number | undefined;

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
   * @param {number|undefined} mediaType
   * @param {string|undefined} [message]
   */
  constructor(
    url: string | undefined,
    reason: RequestErrorReason,
    status: number | undefined,
    mediaType: MediaType | undefined,
    message?: string | undefined,
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspMediaPlaylistRequestError.prototype);

    this.name = "WaspMediaPlaylistRequestError";
    this.status = status;
    this.mediaType = mediaType;
    this.url = url;
    let formattedMsg = message;
    switch (reason) {
      case RequestErrorReason.Status:
        formattedMsg =
          formattedMsg ??
          (status === undefined
            ? "A Media Playlist HTTP(S) request(s) " +
              "responded with an invalid status"
            : "A Media Playlist HTTP(S) request(s) " +
              `responded with a ${status} status`);
        this.code = WaspErrorCode.MediaPlaylistBadHttpStatus;
        break;

      case RequestErrorReason.Timeout:
        formattedMsg =
          formattedMsg ?? "A Media Playlist HTTP(S) request(s) did not respond";
        this.code = WaspErrorCode.MediaPlaylistRequestTimeout;
        break;

      case RequestErrorReason.Error:
        formattedMsg =
          formattedMsg ??
          "A Media Playlist HTTP(S) request(s) failed due to an error.";
        this.code = WaspErrorCode.MediaPlaylistRequestError;
        break;

      case RequestErrorReason.Other:
        formattedMsg =
          formattedMsg ??
          "A Media Playlist HTTP(S) request(s) failed for an unknown reason.";
        this.code = WaspErrorCode.MediaPlaylistRequestOtherError;
        break;

      default:
        this.code = WaspErrorCode.MediaPlaylistRequestOtherError;
        break;
    }
    this.globalCode = this.code;
    this.message =
      formattedMsg ??
      "An error arised while trying to perform a segment request";
  }
}
