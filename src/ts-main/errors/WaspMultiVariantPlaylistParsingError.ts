import { MultiVariantPlaylistParsingErrorCode } from "../../wasm/wasp_hls.d";
import { WaspErrorCode } from "./common";

export default class WaspMultiVariantPlaylistParsingError extends Error {
  public readonly name: "WaspMultiVariantPlaylistParsingError";
  public readonly message: string;

  /** Specifies the exact error encountered. */
  public readonly code:
    | "MultiVariantPlaylistMissingUriLineAfterVariant"
    | "MultiVariantPlaylistWithoutVariant"
    | "MultiVariantPlaylistVariantMissingBandwidth"
    | "MultiVariantPlaylistInvalidValue"
    | "MultiVariantPlaylistMediaTagMissingType"
    | "MultiVariantPlaylistMediaTagMissingName"
    | "MultiVariantPlaylistMediaTagMissingGroupId"
    | "MultiVariantPlaylistMissingExtM3uHeader"
    | "MultiVariantPlaylistOtherError";

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
   * @param {number} code
   * @param {string} message
   */
  constructor(
    code: MultiVariantPlaylistParsingErrorCode,
    message?: string | undefined
  ) {
    super();
    // @see https://stackoverflow.com/questions/41102060/typescript-extending-error-class
    Object.setPrototypeOf(this, WaspMultiVariantPlaylistParsingError.prototype);

    switch (code) {
      case MultiVariantPlaylistParsingErrorCode.MissingExtM3uHeader:
        this.code = "MultiVariantPlaylistMissingExtM3uHeader";
        break;
      case MultiVariantPlaylistParsingErrorCode.MultiVariantPlaylistWithoutVariant:
        this.code = "MultiVariantPlaylistWithoutVariant";
        break;
      case MultiVariantPlaylistParsingErrorCode.MissingUriLineAfterVariant:
        this.code = "MultiVariantPlaylistMissingUriLineAfterVariant";
        break;
      case MultiVariantPlaylistParsingErrorCode.VariantMissingBandwidth:
        this.code = "MultiVariantPlaylistVariantMissingBandwidth";
        break;
      case MultiVariantPlaylistParsingErrorCode.InvalidValue:
        this.code = "MultiVariantPlaylistInvalidValue";
        break;
      case MultiVariantPlaylistParsingErrorCode.MediaTagMissingType:
        this.code = "MultiVariantPlaylistMediaTagMissingType";
        break;
      case MultiVariantPlaylistParsingErrorCode.MediaTagMissingName:
        this.code = "MultiVariantPlaylistMediaTagMissingName";
        break;
      case MultiVariantPlaylistParsingErrorCode.MediaTagMissingGroupId:
        this.code = "MultiVariantPlaylistMediaTagMissingGroupId";
        break;
      case MultiVariantPlaylistParsingErrorCode.Unknown:
        this.code = "MultiVariantPlaylistOtherError";
        break;
      default:
        this.code = "MultiVariantPlaylistOtherError";
        break;
    }

    this.globalCode = this.code;

    this.name = "WaspMultiVariantPlaylistParsingError";
    this.message =
      message ?? "Unknown error when parsing the MultiVariant Playlist";
  }
}
