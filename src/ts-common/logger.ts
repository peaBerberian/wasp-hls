import EventEmitter from "./EventEmitter";
import noop from "./noop";

export const enum LoggerLevel {
  None = 0,
  Error = 1,
  Warning = 2,
  Info = 3,
  Debug = 4,
}

type ConsoleFunction = (
  // TODO remove unknown here
  ...args: Array<boolean | string | number | Error | null | unknown | undefined>
) => void;

const DEFAULT_LOG_LEVEL = LoggerLevel.None;

interface LoggerEvents {
  onLogLevelChange: LoggerLevel;
}

/**
 * Logger implementation.
 *
 * TODO how to synchronize with worker?
 * @class Logger
 */
export class Logger extends EventEmitter<LoggerEvents> {
  public error: ConsoleFunction;
  public warn: ConsoleFunction;
  public info: ConsoleFunction;
  public debug: ConsoleFunction;
  private _currentLevel: LoggerLevel;

  constructor() {
    super();
    this.error = noop;
    this.warn = noop;
    this.info = noop;
    this.debug = noop;
    this._currentLevel = DEFAULT_LOG_LEVEL;
  }

  /**
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
