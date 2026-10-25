import sleep from "./sleep.js";
import { PLAYER_EVENT_TIMEOUT_MS, PLAYER_LOAD_TIMEOUT_MS } from "./timeouts.js";
import { waitForLoadedStateAfterLoad } from "./waitForPlayerState.js";

export function getPlayerStateSnapshot(player, videoElement, lastPlayerError) {
  return {
    playerState: player.getPlayerState(),
    playerError: player.getError() ?? lastPlayerError,
    position: player.getPosition(),
    mediaOffset: player.getMediaOffset(),
    minimumPosition: player.getMinimumPosition(),
    maximumPosition: player.getMaximumPosition(),
    seekableMinimumPosition: player.getSeekableMinimumPosition(),
    seekableMaximumPosition: player.getSeekableMaximumPosition(),
    usesProgramDateTime: player.usesProgramDateTime(),
    currentTime: videoElement.currentTime,
    readyState: videoElement.readyState,
    networkState: videoElement.networkState,
    paused: videoElement.paused,
    ended: videoElement.ended,
    seeking: videoElement.seeking,
    playbackRate: videoElement.playbackRate,
    buffered: Array.from({ length: videoElement.buffered.length }, (_, i) => [
      videoElement.buffered.start(i),
      videoElement.buffered.end(i),
    ]),
    mediaError:
      videoElement.error === null
        ? null
        : {
            code: videoElement.error.code,
            message: videoElement.error.message,
          },
  };
}

// Resume a loaded player and wait for playback to advance.
export function waitForPlayback(
  player,
  videoElement,
  lastPlayerErrorRef,
  timeoutMs = PLAYER_EVENT_TIMEOUT_MS,
) {
  const startTime = videoElement.currentTime;
  return new Promise((resolve, reject) => {
    let settled = false;
    const timeoutId = setTimeout(
      () => fail("Timed out waiting for playback to advance"),
      timeoutMs,
    );

    function cleanup() {
      settled = true;
      clearTimeout(timeoutId);
      player.removeEventListener("error", onPlayerError);
      videoElement.removeEventListener("timeupdate", onTimeUpdate);
    }

    function fail(message, cause) {
      if (settled) {
        return;
      }
      cleanup();
      reject(
        new Error(
          `${message}: ` +
            JSON.stringify(
              getPlayerStateSnapshot(
                player,
                videoElement,
                lastPlayerErrorRef(),
              ),
            ),
          { cause },
        ),
      );
    }

    function onPlayerError(error) {
      fail(
        `Player failed during playback: ${error.name}: ${error.message}`,
        error,
      );
    }

    function onTimeUpdate() {
      if (!settled && videoElement.currentTime > startTime + 0.5) {
        cleanup();
        resolve();
      }
    }

    player.addEventListener("error", onPlayerError);
    videoElement.addEventListener("timeupdate", onTimeUpdate);
    const error = player.getError();
    if (error !== null) {
      onPlayerError(error);
      return;
    }
    Promise.resolve()
      .then(() => player.resume())
      .catch((error) => fail(`Could not resume playback: ${error}`, error));
  });
}

export async function waitForLoadedState(
  player,
  videoElement,
  lastPlayerErrorRef,
) {
  const timeoutPromise = sleep(PLAYER_LOAD_TIMEOUT_MS).then(() => {
    throw new Error(
      "Player did not reach Loaded in time: " +
        JSON.stringify(
          getPlayerStateSnapshot(player, videoElement, lastPlayerErrorRef()),
        ),
    );
  });

  try {
    await Promise.race([waitForLoadedStateAfterLoad(player), timeoutPromise]);
  } catch (error) {
    throw new Error(
      "Player failed before reaching Loaded: " +
        JSON.stringify({
          error,
          snapshot: getPlayerStateSnapshot(
            player,
            videoElement,
            lastPlayerErrorRef(),
          ),
        }),
    );
  }

  if (player.getPlayerState() !== "Loaded") {
    throw new Error(
      "Player did not settle in Loaded state: " +
        JSON.stringify(
          getPlayerStateSnapshot(player, videoElement, lastPlayerErrorRef()),
        ),
    );
  }
}

export function waitForPlayerEvent(
  player,
  eventName,
  predicate = () => true,
  timeoutMs = PLAYER_EVENT_TIMEOUT_MS,
) {
  return new Promise((resolve, reject) => {
    const timeoutId = setTimeout(() => {
      cleanup();
      reject(new Error(`Timed out waiting for player event "${eventName}"`));
    }, timeoutMs);

    function cleanup() {
      clearTimeout(timeoutId);
      player.removeEventListener(eventName, onEvent);
    }

    function onEvent(eventValue) {
      if (!predicate(eventValue)) {
        return;
      }
      cleanup();
      resolve(eventValue);
    }

    player.addEventListener(eventName, onEvent);
  });
}

export function eventListener(player, eventName) {
  let payloads = [];
  player.addEventListener(eventName, (payload) => {
    payloads.push(payload);
  });
  return {
    getCurrentCount() {
      return payloads.length;
    },
    getPayloadFor(idx) {
      return payloads[idx];
    },
    awaitNext(predicate, timeoutMs) {
      return waitForPlayerEvent(player, eventName, predicate, timeoutMs);
    },
  };
}
