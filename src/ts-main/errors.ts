import { InitializationErrorCode } from "../ts-common/types";
import {
  MediaType,
  PlaylistType,
  RequestErrorReason,
  OtherErrorCode,
  SourceBufferCreationErrorCode,
} from "../wasm/wasp_hls";

export class WaspInitializationError extends Error {
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
    Object.setPrototypeOf(this, WaspInitializationError.prototype);

    this.name = "InitializationError";
    this.code = code;
    this.wasmHttpStatus = wasmHttpStatus;
    this.message = message;
  }
}

export class WaspSegmentRequestError extends Error {
  public readonly name: "WaspRequestError";
  public readonly message : string;
  public readonly url : string | undefined;
  public readonly isInit : boolean | undefined;
  public readonly reason : number;

  /**
   * @param {string} message
   */
  constructor(
    args: WaspSegmentRequestErrorArgument,
    message? : string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSegmentRequestError.prototype);

    this.name = "WaspRequestError";
    this.reason = args.reason;
    this.url = args.url;
    this.isInit = args.isInit;
    let formattedMsg = message;
    if (formattedMsg === undefined) {
      const { mediaType } = args;
      const msgStart = mediaTypeAsArticle(mediaType) + " segment";
      switch (args.reason) {
        case RequestErrorReason.Status:
          formattedMsg = args.status === undefined ?
            `${msgStart}'s HTTP(S) request(s) responded with an invalid status` :
            `${msgStart}'s HTTP(S) request(s) responded with a ${args.status} status`;
          break;

        case RequestErrorReason.Timeout:
          formattedMsg = `${msgStart}'s HTTP(S) request(s) did not respond`;
          break;

        case RequestErrorReason.Error:
          formattedMsg = `${msgStart}'s HTTP(S) request(s) failed due to an error.`;
          break;

        case RequestErrorReason.Other:
          formattedMsg = `${msgStart}'s HTTP(S) request(s) failed for an unknown reason.`;
          break;
      }
    }
    this.message = formattedMsg ??
      "An error arised while trying to perform a segment request";
  }
}

export class WaspOtherError extends Error {
  public readonly name: "WaspOtherError";
  public readonly message : string;
  public readonly code : number;

  /**
   * @param {string} message
   */
  constructor(
    code: OtherErrorCode,
    message? : string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspOtherError.prototype);

    this.name = "WaspOtherError";
    this.code = code;
    this.message = message ?? "Unknown error";
  }
}

export class WaspSourceBufferCreationError extends Error {
  public readonly name: "WaspSourceBufferCreationError";
  public readonly message : string;
  public readonly code : number;

  /**
   * @param {string} message
   */
  constructor(
    code: SourceBufferCreationErrorCode,
    message? : string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferCreationError.prototype);

    this.name = "WaspSourceBufferCreationError";
    this.code = code;
    this.message = message ?? "Unknown error when creating SourceBuffer";
  }
}

export class WaspPlaylistParsingError extends Error {
  public readonly name: "WaspPlaylistParsingError";
  public readonly message : string;
  public readonly mediaType : MediaType | undefined;
  public readonly playlistType : PlaylistType | undefined;

  /**
   * @param {string} message
   */
  constructor(
    playlistType: PlaylistType,
    mediaType: MediaType | undefined,
    message? : string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspSourceBufferCreationError.prototype);

    this.name = "WaspPlaylistParsingError";
    this.playlistType = playlistType;
    this.mediaType = mediaType;
    this.message = message ?? "Unknown error when parsing Playlist";
  }
}

function mediaTypeAsArticle(
  mediaType: MediaType
): string {
  switch (mediaType) {
    case MediaType.Audio:
      return "An audio";
    case MediaType.Video:
      return "A video";
  }
  throw new Error("Unknown MediaType");
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
