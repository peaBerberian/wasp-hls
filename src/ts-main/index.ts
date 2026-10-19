import logger, {
  LoggerLevel as innerLoggerLevel,
} from "../ts-common/logger.ts";
import type {
  InitialAudioTrackPreference,
  InitialAudioTrackSelection,
  WaspHlsPlayerConfig,
} from "../ts-common/types.ts";
import WaspHlsPlayer from "./api.ts";
import { PlayerState as innerPlayerState } from "./types.ts";

export type {
  InitialAudioTrackPreference,
  InitialAudioTrackSelection,
  WaspHlsPlayerConfig,
};
export type { AudioTrackInfo, VariantInfo } from "./types.ts";
export type { WaspError } from "./errors/index.ts";
export {
  WaspErrorCode,
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
} from "./errors/index.ts";

/** Enumerates the various "states" the WaspHlsPlayer can be in. */
export const PlayerState = {
  /** No content is currently loaded or waiting to load. */
  Stopped: innerPlayerState.Stopped,
  /** A content is currently being loaded but not ready for playback yet. */
  Loading: innerPlayerState.Loading,
  /** A content is loaded. */
  Loaded: innerPlayerState.Loaded,
  /** The last content loaded failed on error. */
  Error: innerPlayerState.Error,
} as const;

// TODO only debug mode?
/* eslint-disable */
(window as any).WaspHlsPlayer = WaspHlsPlayer;
/* eslint-enable */

// Re-definition for easier usage by JavaScript and TypeScript applications
export const LoggerLevel = {
  Debug: innerLoggerLevel.Debug,
  Error: innerLoggerLevel.Error,
  Warning: innerLoggerLevel.Warning,
  Info: innerLoggerLevel.Info,
  None: innerLoggerLevel.None,
} as const;

export { logger };

export default WaspHlsPlayer;
