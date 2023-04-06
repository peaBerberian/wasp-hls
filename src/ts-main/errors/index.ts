import WaspInitializationError from "./WaspInitializationError";
import WaspMediaPlaylistParsingError from "./WaspMediaPlaylistParsingError";
import WaspMediaPlaylistRequestError from "./WaspMediaPlaylistRequestError";
import WaspMultivariantPlaylistParsingError from "./WaspMultivariantPlaylistParsingError";
import WaspMultivariantPlaylistRequestError from "./WaspMultivariantPlaylistRequestError";
import WaspOtherError from "./WaspOtherError";
import WaspSegmentParsingError from "./WaspSegmentParsingError";
import WaspSegmentRequestError from "./WaspSegmentRequestError";
import WaspSourceBufferCreationError from "./WaspSourceBufferCreationError";
import WaspSourceBufferError from "./WaspSourceBufferError";

/**
 * General type for all potential errors returned by the `WaspHlsPlayer`.
 */
export type WaspError =
  | WaspMediaPlaylistParsingError
  | WaspMediaPlaylistRequestError
  | WaspMultivariantPlaylistParsingError
  | WaspMultivariantPlaylistRequestError
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
  WaspMultivariantPlaylistParsingError,
  WaspMultivariantPlaylistRequestError,
  WaspOtherError,
  WaspSegmentParsingError,
  WaspSegmentRequestError,
  WaspSourceBufferCreationError,
  WaspSourceBufferError,
};
