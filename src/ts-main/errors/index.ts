import WaspContentCompatibilityError from "./WaspContentCompatibilityError.ts";
import WaspInitializationError from "./WaspInitializationError.ts";
import WaspMediaPlaylistParsingError from "./WaspMediaPlaylistParsingError.ts";
import WaspMediaPlaylistRequestError from "./WaspMediaPlaylistRequestError.ts";
import WaspMultivariantPlaylistParsingError from "./WaspMultivariantPlaylistParsingError.ts";
import WaspMultivariantPlaylistRequestError from "./WaspMultivariantPlaylistRequestError.ts";
import WaspOtherError from "./WaspOtherError.ts";
import WaspSegmentParsingError from "./WaspSegmentParsingError.ts";
import WaspSegmentRequestError from "./WaspSegmentRequestError.ts";
import WaspSourceBufferCreationError from "./WaspSourceBufferCreationError.ts";
import WaspSourceBufferError from "./WaspSourceBufferError.ts";
import WaspTrackSelectionError from "./WaspTrackSelectionError.ts";

/**
 * General type for all potential errors returned by the `WaspHlsPlayer`.
 */
export type WaspError =
  | WaspContentCompatibilityError
  | WaspMediaPlaylistParsingError
  | WaspMediaPlaylistRequestError
  | WaspMultivariantPlaylistParsingError
  | WaspMultivariantPlaylistRequestError
  | WaspTrackSelectionError
  | WaspOtherError
  | WaspSegmentParsingError
  | WaspSegmentRequestError
  | WaspSourceBufferCreationError
  | WaspSourceBufferError;

export { WaspErrorCode } from "./common.ts";

export {
  WaspContentCompatibilityError,
  WaspInitializationError,
  WaspMediaPlaylistParsingError,
  WaspMediaPlaylistRequestError,
  WaspMultivariantPlaylistParsingError,
  WaspMultivariantPlaylistRequestError,
  WaspOtherError,
  WaspTrackSelectionError,
  WaspSegmentParsingError,
  WaspSegmentRequestError,
  WaspSourceBufferCreationError,
  WaspSourceBufferError,
};
