import assertNever from "../../ts-common/assertNever";
import { MediaType, RequestErrorReason } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

// TODO add `MediaType`?
// TODO add `status`?

/**
 * Error used to describe a problem with a segment request.
 * @class WaspSegmentRequestError
 */
export default class WaspSegmentRequestError extends Error {
  /** Identifies a `WaspSegmentRequestError` */
  public readonly name: "WaspSegmentRequestError";

  /** Human-readable message describing the error. */
  public readonly message: string;

  /** Optionally set to the URL of the requested segment. */
  public readonly url: string | undefined;

  /**
   * If `true`, the error concerns an initialization segment.
   * If `false`, it concerns a media segment.
   *
   * If `undefined` it is not known whether it concerns an initialization or
   * media segment.
   */
  public readonly isInit: boolean | undefined;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "SegmentBadHttpStatus"
    | "SegmentRequestTimeout"
    | "SegmentRequestError"
    | "SegmentRequestOtherError";

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
   * @param {Object} args
   * @param {string} message
   */
  constructor(
    args: WaspSegmentRequestErrorArgument,
    message?: string | undefined,
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSegmentRequestError.prototype);

    this.name = "WaspSegmentRequestError";
    this.url = args.url;
    this.isInit = args.isInit;
    let formattedMsg = message;
    const { mediaType } = args;
    const msgStart = mediaTypeAsArticle(mediaType) + " segment";
    switch (args.reason) {
      case RequestErrorReason.Status:
        formattedMsg =
          formattedMsg ??
          (args.status === undefined
            ? `${msgStart}'s HTTP(S) request(s) responded with an invalid status`
            : `${msgStart}'s HTTP(S) request(s) responded with a ${args.status} status`);
        this.code = WaspErrorCode.SegmentBadHttpStatus;
        break;

      case RequestErrorReason.Timeout:
        formattedMsg =
          formattedMsg ?? `${msgStart}'s HTTP(S) request(s) did not respond`;
        this.code = WaspErrorCode.SegmentRequestTimeout;
        break;

      case RequestErrorReason.Error:
        formattedMsg =
          formattedMsg ??
          `${msgStart}'s HTTP(S) request(s) failed due to an error.`;
        this.code = WaspErrorCode.SegmentRequestError;
        break;

      case RequestErrorReason.Other:
        formattedMsg =
          formattedMsg ??
          `${msgStart}'s HTTP(S) request(s) failed for an unknown reason.`;
        this.code = WaspErrorCode.SegmentRequestOtherError;
        break;

      default:
        this.code = WaspErrorCode.SegmentRequestOtherError;
        break;
    }
    this.globalCode = this.code;
    this.message =
      formattedMsg ??
      "An error arised while trying to perform a segment request";
  }
}

export interface WaspSegmentRequestErrorArgument {
  url?: string;
  isInit: boolean;
  start?: number | undefined;
  duration?: number | undefined;
  mediaType: MediaType;
  byteRange?: [number, number] | undefined;
  reason: RequestErrorReason;
  status?: number | undefined;
}

function mediaTypeAsArticle(mediaType: MediaType): string {
  switch (mediaType) {
    case MediaType.Audio:
      return "An audio";
    case MediaType.Video:
      return "A video";
    default:
      assertNever(mediaType);
  }
}
