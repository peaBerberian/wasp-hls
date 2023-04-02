import EventEmitter from "../ts-common/EventEmitter";
import timeRangesToFloat64Array from "../ts-common/timeRangesToFloat64Array";
import { MediaObservation, PlaybackTickReason } from "../ts-common/types";

/** Items emitted by `observePlayback`. */
export type PlaybackObserverObservation = Omit<
  MediaObservation,
  "sourceBuffersBuffered" | "mediaSourceId"
>;

/**
 * Events for which a new playback observation is sent.
 *
 * Those are actually tuples of two values, with:
 *   - The first value being the actual event being listened to on the
 *     HTMLMediaElement.
 *   - The second value being the corresponding `PlaybackTickReason` sent to the
 *     worker.
 *
 * @see PlaybackTickReason
 */
const OBSERVATION_EVENTS = [
  ["seeking", PlaybackTickReason.Seeking],
  ["seeked", PlaybackTickReason.Seeked],
  ["loadedmetadata", PlaybackTickReason.LoadedMetadata],
  ["loadeddata", PlaybackTickReason.LoadedData],
  ["canplay", PlaybackTickReason.CanPlay],
  ["canplaythrough", PlaybackTickReason.CanPlayThrough],
  ["ended", PlaybackTickReason.Ended],
  ["pause", PlaybackTickReason.Pause],
  ["play", PlaybackTickReason.Play],
  ["ratechange", PlaybackTickReason.RateChange],
  ["stalled", PlaybackTickReason.Stalled],
  // "durationchange",
] as const;

export interface PlaybackObserverEvents {
  /**
   * Event through which playback observations will be sent.
   *
   * The first observation is sent directly after a `start` call.
   */
  newObservation: PlaybackObserverObservation;
}

/**
 * Class emitting "playback observations" at regular intervals, so that the
 * `WaspHlsPlayer` can react on various playback-related events and can
 * re-synchronize regularly with the current playback conditions.
 */
export default class PlaybackObserver extends EventEmitter<PlaybackObserverEvents> {
  private _mediaElement: HTMLMediaElement;
  private _minimumObservationInterval: number;
  private _stop: (() => void) | null;
  private _currentTimeoutId: number | null;
  private _lastObservationTimeStamp: number;

  /**
   * Create a new `PlaybackObserver` associated to the given `HTMLMediaElement`.
   *
   * It will begin to emit events only once the `start` method is called.
   * @param {HTMLMediaElement} mediaElement - The `HTMLMediaElement` to observe.
   * @param {number} minimumObservationInterval
   */
  constructor(mediaElement: HTMLMediaElement, minimumObservationInterval: number) {
    super();
    this._mediaElement = mediaElement;
    this._stop = null;
    this._minimumObservationInterval = minimumObservationInterval;
    this._currentTimeoutId = null;
    this._lastObservationTimeStamp = 0;
  }

  /**
   * Start producing playback observation events.
   *
   * Once you don't need playback observation to be produced anymore, you can
   * call the `stop` method to free all reserved resources and stop emitting
   * events.
   */
  public start(): void {
    if (this._stop !== null) {
      return;
    }

    /**
     * When set to `true`, "playback observations" have been stopped.
     */
    let isStopped = false;

    const listenerRemovers = OBSERVATION_EVENTS.map(([evtName, reason]) => {
      const onEvent = () => {
        this._generateObservation(reason);
      };
      this._mediaElement.addEventListener(evtName, onEvent);
      return () => this._mediaElement.removeEventListener(evtName, onEvent);
    });
    this._stop = () => {
      if (isStopped) {
        return;
      }
      isStopped = true;
      listenerRemovers.forEach((removeCb) => removeCb());
      listenerRemovers.length = 0;
    };
    this._generateObservation(PlaybackTickReason.Init);
  }

  /**
   * Update the minimum interval, in milliseconds, at which playback
   * observations are produced.
   * @param {number} newInterval
   */
  public updateMinimumObservationInterval(newInterval: number): void {
    if (newInterval !== this._minimumObservationInterval) {
      this._minimumObservationInterval = newInterval;
      if (this._stop !== null) {
        const timeSinceLast = performance.now() - this._lastObservationTimeStamp;
        const nextTimeout = this._minimumObservationInterval - timeSinceLast;
        this._currentTimeoutId = window.setTimeout(() => {
          this._generateObservation(PlaybackTickReason.RegularInterval);
        }, nextTimeout);
      }
    }
  }

  /**
   * Stop producing playback observations and free resources reserved by the
   * `PlaybackObserver`.
   */
  public stop() {
    if (this._stop !== null) {
      this._stop();
      this._stop = null;
    }
    if (this._currentTimeoutId !== null) {
      clearTimeout(this._currentTimeoutId);
      this._currentTimeoutId = null;
    }
  }

  /**
   * Generate the observation, trigger it, with the given reason.
   * @param {number} reason
   */
  private _generateObservation(reason: PlaybackTickReason) {
    if (this._stop === null) {
      this._currentTimeoutId = null;
      return;
    }
    if (this._currentTimeoutId !== null) {
      clearTimeout(this._currentTimeoutId);
      this._currentTimeoutId = null;
    }

    const buffered = timeRangesToFloat64Array(this._mediaElement.buffered);
    const { currentTime, readyState, paused, seeking, ended, duration } =
      this._mediaElement;

    this._lastObservationTimeStamp = performance.now();
    this.trigger("newObservation", {
      reason,
      currentTime,
      readyState,
      buffered,
      paused,
      seeking,
      ended,
      duration,
    });

    this._currentTimeoutId = window.setTimeout(() => {
      this._generateObservation(PlaybackTickReason.RegularInterval);
    }, this._minimumObservationInterval);
  }
}
