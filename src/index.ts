import logger, { LoggerLevel } from "./ts-common/logger";
import WaspHlsPlayer from "./ts-main";
export {
  PlayerState,
  WaspError,
  WaspErrorCode,
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
} from "./ts-main";

// TODO only debug mode?
/* eslint-disable */
(window as any).WaspHlsPlayer = WaspHlsPlayer;
/* eslint-enable */

// Re-definition for easier usage by JavaScript and TypeScript applications
export const LoggerLevels = {
  Debug: LoggerLevel.Debug,
  Error: LoggerLevel.Error,
  Warning: LoggerLevel.Warning,
  Info: LoggerLevel.Info,
  None: LoggerLevel.None,
} as const;

export { logger };

export default WaspHlsPlayer;
