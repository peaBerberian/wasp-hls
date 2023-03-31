import { MediaType } from "../../wasm/wasp_hls";
import { WaspErrorCode } from "./common";

export default class WaspMediaPlaylistParsingError extends Error {
  public readonly name: "WaspMediaPlaylistParsingError";
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code: "Unknown";

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
   * The media type associated to the Media Playlist associated to this error.
   *
   * `undefined` if unknown or if the concept cannot be applied here.
   */
  public readonly mediaType: MediaType | undefined;

  /**
   * @param {number|undefined} mediaType
   * @param {string} message
   */
  constructor(mediaType: MediaType | undefined, message?: string | undefined) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspMediaPlaylistParsingError.prototype);

    this.name = "WaspMediaPlaylistParsingError";
    this.code = "Unknown";
    this.globalCode = this.code;
    this.mediaType = mediaType;
    this.message = message ?? "Unknown error when parsing a Media Playlist";
  }
}
