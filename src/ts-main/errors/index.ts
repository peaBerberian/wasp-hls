import WaspInitializationError from "./WaspInitializationError";
import WaspMediaPlaylistParsingError from "./WaspMediaPlaylistParsingError";
import WaspMediaPlaylistRequestError from "./WaspMediaPlaylistRequestError";
import WaspMultiVariantPlaylistParsingError from "./WaspMultiVariantPlaylistParsingError";
import WaspMultiVariantPlaylistRequestError from "./WaspMultiVariantPlaylistRequestError";
import WaspOtherError from "./WaspOtherError";
import WaspSegmentParsingError from "./WaspSegmentParsingError";
import WaspSegmentRequestError from "./WaspSegmentRequestError";
import WaspSourceBufferCreationError from "./WaspSourceBufferCreationError";
import WaspSourceBufferError from "./WaspSourceBufferError";

/**
 * General type for all potential errors returned by the `WaspHlsPlayer`.
 */
export type WaspError =
  | WaspInitializationError
  | WaspMediaPlaylistParsingError
  | WaspMediaPlaylistRequestError
  | WaspMultiVariantPlaylistParsingError
  | WaspMultiVariantPlaylistRequestError
  | WaspOtherError
  | WaspSegmentParsingError
  | WaspSegmentRequestError
  | WaspSourceBufferCreationError
  | WaspSourceBufferError;

export { WaspErrorCode } from "./common";

export {
  WaspInitializationError,
  WaspMediaPlaylistParsingError,
  WaspMediaPlaylistRequestError,
  WaspMultiVariantPlaylistParsingError,
  WaspMultiVariantPlaylistRequestError,
  WaspOtherError,
  WaspSegmentParsingError,
  WaspSegmentRequestError,
  WaspSourceBufferCreationError,
  WaspSourceBufferError,
};
