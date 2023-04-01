import EventEmitter from "./EventEmitter";
import noop from "./noop";

/**
 * Possible verbosity level for the `WaspHlsPlayer`'s Logger.
 * A lower numberical value means less verbose.
 */
export const enum LoggerLevel {
  None = 0,
  Error = 1,
  Warning = 2,
  Info = 3,
  Debug = 4,
}

/**
 * Define the `Logger`'s console functions.
 * We're here restricting the types that can be logged to limit memory usage
 * when an inspector is displayed on a tested page.
 */
type ConsoleFunction = (
  ...args: Array<boolean | string | number | Error | null | undefined>
) => void;

/** Logger level initially set on `Logger`. */
const DEFAULT_LOG_LEVEL = LoggerLevel.None;

/**
 * Events sent by `Logger` where the keys are the events' name and the values
 * are the corresponding payloads.
 */
interface LoggerEvents {
  onLogLevelChange: LoggerLevel;
}

/**
 * Logger implementation.
 * @class Logger
 */
export class Logger extends EventEmitter<LoggerEvents> {
  public error: ConsoleFunction;
  public warn: ConsoleFunction;
  public info: ConsoleFunction;
  public debug: ConsoleFunction;
  private _currentLevel: LoggerLevel;

  /**
   * Create a whole new `Logger`, independent of other `Logger`.
   */
  constructor() {
    super();
    this.error = noop;
    this.warn = noop;
    this.info = noop;
    this.debug = noop;
    this._currentLevel = DEFAULT_LOG_LEVEL;
  }

  /**
   * Update the `Logger`'s verbosity level to the given one.
   * @param {number} level
   */
  public setLevel(level: LoggerLevel): void {
    const actualLevel =
      level < 0 || level > LoggerLevel.Debug ? LoggerLevel.None : level;
    this._currentLevel = actualLevel;

    /* eslint-disable no-invalid-this */
    /* eslint-disable no-console */
    this.error =
      actualLevel >= LoggerLevel.Error ? console.error.bind(console) : noop;
    this.warn =
      actualLevel >= LoggerLevel.Warning ? console.warn.bind(console) : noop;
    this.info =
      actualLevel >= LoggerLevel.Info ? console.info.bind(console) : noop;
    this.debug =
      actualLevel >= LoggerLevel.Debug ? console.debug.bind(console) : noop;
    /* eslint-enable no-console */
    /* eslint-enable no-invalid-this */

    this.trigger("onLogLevelChange", actualLevel);
  }

  /**
   * Returns the `Logger`'s current verbosity level.
   * @returns {number}
   */
  public getLevel(): LoggerLevel {
    return this._currentLevel;
  }

  /**
   * Returns `true` if the currently set level includes logs of the level given
   * in argument.
   * @param {number} logLevel
   * @returns {boolean}
   */
  public hasLevel(logLevel: LoggerLevel): boolean {
    return logLevel >= this._currentLevel;
  }
}

const logger = new Logger();
export default logger;
