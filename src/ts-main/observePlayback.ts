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

/**
 * Function emitting "playback observations" at regular intervals, so that the
 * `WaspHlsPlayer` can react on various playback-related events and can
 * re-synchronize regularly with the current playback conditions.
 *
 * @param {HTMLMediaElement} videoElement - The `HTMLMediaElement` to observe.
 * @param {Function} onNewObservation - Callback through which playback
 * observations will be sent. The first observation is sent through a microtask
 * directly after this call.
 * @returns {Function} - Callback allowing to stop producing observations and
 * to free all resources this function was reserving.
 */
export default function observePlayback(
  videoElement: HTMLMediaElement,
  onNewObservation: (observation: PlaybackObserverObservation) => void
): () => void {
  /**
   * When set to `true`, "playback observations" have been stopped.
   */
  let isStopped = false;

  /**
   * Identifier for the `setTimeout` which will trigger a new observation.
   */
  let timeoutId: number | undefined;
  const listenerRemovers = OBSERVATION_EVENTS.map(([evtName, reason]) => {
    videoElement.addEventListener(evtName, onEvent);
    function onEvent() {
      generateObservation(reason);
    }
    return () => videoElement.removeEventListener(evtName, onEvent);
  });

  /* eslint-disable @typescript-eslint/no-floating-promises */
  Promise.resolve().then(() => generateObservation(PlaybackTickReason.Init));

  return () => {
    if (isStopped) {
      return;
    }
    isStopped = true;
    listenerRemovers.forEach((removeCb) => removeCb());
    listenerRemovers.length = 0;
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
      timeoutId = undefined;
    }
  };

  function generateObservation(reason: PlaybackTickReason) {
    if (isStopped) {
      return;
    }
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
      timeoutId = undefined;
    }

    const buffered = timeRangesToFloat64Array(videoElement.buffered);
    const { currentTime, readyState, paused, seeking, ended, duration } =
      videoElement;
    onNewObservation({
      reason,
      currentTime,
      readyState,
      buffered,
      paused,
      seeking,
      ended,
      duration,
    });

    timeoutId = window.setTimeout(() => {
      if (isStopped) {
        timeoutId = undefined;
        return;
      }
      generateObservation(PlaybackTickReason.RegularInterval);
    }, 1000);
  }
}
