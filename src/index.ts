import logger, { LoggerLevel } from "./ts-common/logger";
import WaspHlsPlayer, { PlayerState } from "./ts-main";

// TODO only debug mode?
/* eslint-disable */
(window as any).WaspHlsPlayer = WaspHlsPlayer;
/* eslint-enable */

// Re-definition for easier usage by JavaScript and TypeScript applications
const LoggerLevels = {
  Debug: LoggerLevel.Debug,
  Error: LoggerLevel.Error,
  Warning: LoggerLevel.Warning,
  Info: LoggerLevel.Info,
  None: LoggerLevel.None,
} as const;

export { PlayerState, logger, LoggerLevels };
export default WaspHlsPlayer;
