import { waitForLoadedStateAfterLoad } from "./waitForPlayerState.js";

const PLAYER_LOAD_TIMEOUT_MS = 90_000;

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
    mediaPosition:
      player.getMediaOffset() === undefined
        ? undefined
        : videoElement.currentTime - player.getMediaOffset(),
    readyState: videoElement.readyState,
    networkState: videoElement.networkState,
    paused: videoElement.paused,
    ended: videoElement.ended,
  };
}

export async function waitForLoadedState(
  player,
  videoElement,
  lastPlayerErrorRef,
) {
  let timeoutId;
  const timeoutPromise = new Promise((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(
        new Error(
          "Player did not reach Loaded in time: " +
            JSON.stringify(
              getPlayerStateSnapshot(
                player,
                videoElement,
                lastPlayerErrorRef(),
              ),
            ),
        ),
      );
    }, PLAYER_LOAD_TIMEOUT_MS);
  });

  try {
    await Promise.race([waitForLoadedStateAfterLoad(player), timeoutPromise]);
  } catch (error) {
    throw new Error(
      "Player failed before reaching Loaded: " +
        JSON.stringify({
          error:
            error instanceof Error
              ? { name: error.name, message: error.message, stack: error.stack }
              : String(error),
          snapshot: getPlayerStateSnapshot(
            player,
            videoElement,
            lastPlayerErrorRef(),
          ),
        }),
    );
  } finally {
    clearTimeout(timeoutId);
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
  timeoutMs = 20_000,
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
