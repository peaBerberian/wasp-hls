import {
  MediaObservation,
  PlaybackTickReason,
} from "../ts-common/types";

/**
 * Events for which a new playback observation is sent.
 *
 * Those are actually tuples of two values, with:
 *   - The first value being the actual event being listened to on the
 *     HTMLMediaElement.
 *   - The second value being the corresponding `PlaybackTickReason` sent to the
 *     worker.
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

export default function observePlayback(
  videoElement: HTMLMediaElement,
  mediaSourceId: string,
  onNewObservation: (observation: MediaObservation) => void
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
      onNextTick(reason);
    }
    return () => videoElement.removeEventListener(evtName, onEvent);
  });

  /* eslint-disable @typescript-eslint/no-floating-promises */
  Promise.resolve().then(() => onNextTick(PlaybackTickReason.Init));

  return () => {
    if (isStopped) {
      return;
    }
    isStopped = true;
    listenerRemovers.forEach(removeCb => removeCb());
    listenerRemovers.length = 0;
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
      timeoutId = undefined;
    }
  };

  function onNextTick(reason: PlaybackTickReason) {
    if (isStopped) {
      return;
    }
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
      timeoutId = undefined;
    }

    const buffered = new Float64Array(videoElement.buffered.length * 2);
    for (let i = 0; i < videoElement.buffered.length; i++) {
      const offset = i * 2;
      buffered[offset] = videoElement.buffered.start(i);
      buffered[offset + 1] = videoElement.buffered.end(i);
    }

    const { currentTime, readyState, paused, seeking } = videoElement;
    onNewObservation({
      mediaSourceId,
      reason,
      currentTime,
      readyState,
      buffered,
      paused,
      seeking,
    });

    timeoutId = window.setTimeout(() => {
      if (isStopped) {
        timeoutId = undefined;
        return;
      }
      onNextTick(PlaybackTickReason.RegularInterval);
    }, 1000);
  }
}
