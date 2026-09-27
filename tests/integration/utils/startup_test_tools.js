import { expect } from "vitest";
import {
  getPlayerStateSnapshot,
  waitForLoadedState,
} from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";

const DEFAULT_PLAYBACK_SETTLE_MS = 1_500;

function createStartupEventTracker(videoElement) {
  const timestamps = {
    seekingAt: undefined,
    loadedMetadataAt: undefined,
    loadedDataAt: undefined,
  };

  const listeners = [
    ["seeking", "seekingAt"],
    ["loadedmetadata", "loadedMetadataAt"],
    ["loadeddata", "loadedDataAt"],
  ].map(([eventName, key]) => {
    const onEvent = () => {
      timestamps[key] ??= performance.now();
    };
    videoElement.addEventListener(eventName, onEvent);
    return [eventName, onEvent];
  });

  return {
    timestamps,
    cleanup() {
      for (const [eventName, listener] of listeners) {
        videoElement.removeEventListener(eventName, listener);
      }
    },
  };
}

export async function assertStartupBehavior({
  player,
  videoElement,
  lastPlayerErrorRef,
  loadContent,
  assertLoadedSnapshot,
  loadedSnapshotContext,
  expectInitialSeek = false,
  maxInitialSeekDelayMs = 20_000,
  maxLoadedDelayMs = 45_000,
  playbackSettleMs = DEFAULT_PLAYBACK_SETTLE_MS,
  maxPlaybackSettleMs = 30_000,
}) {
  const tracker = createStartupEventTracker(videoElement);
  const loadStartedAt = performance.now();

  try {
    loadContent();
    await waitForLoadedState(player, videoElement, lastPlayerErrorRef);
  } finally {
    tracker.cleanup();
  }

  const loadedAt = performance.now();
  const snapshot = getPlayerStateSnapshot(
    player,
    videoElement,
    lastPlayerErrorRef(),
  );
  const timings = {
    loadedDelayMs: loadedAt - loadStartedAt,
    initialSeekDelayMs:
      tracker.timestamps.seekingAt === undefined
        ? undefined
        : tracker.timestamps.seekingAt - loadStartedAt,
    initialLoadedMetadataDelayMs:
      tracker.timestamps.loadedMetadataAt === undefined
        ? undefined
        : tracker.timestamps.loadedMetadataAt - loadStartedAt,
    initialLoadedDataDelayMs:
      tracker.timestamps.loadedDataAt === undefined
        ? undefined
        : tracker.timestamps.loadedDataAt - loadStartedAt,
  };

  expect(timings.loadedDelayMs).toBeLessThanOrEqual(maxLoadedDelayMs);

  if (expectInitialSeek) {
    expect(timings.initialSeekDelayMs).toBeDefined();
    expect(timings.initialSeekDelayMs).toBeLessThanOrEqual(
      maxInitialSeekDelayMs,
    );
  }

  assertLoadedSnapshot(snapshot, timings, loadedSnapshotContext);

  const startPosition = player.getPosition();
  player.resume();
  await checkAfterSleepWithBackoff(
    {
      minTimeMs: playbackSettleMs,
      maxTimeMs: maxPlaybackSettleMs,
      stepMs: 250,
    },
    () => {
      expect(player.getError()).toBeNull();
      expect(player.getPosition()).toBeGreaterThan(startPosition + 0.5);
    },
  );

  return { snapshot, timings };
}
