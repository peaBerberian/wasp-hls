import { expect } from "vitest";
import WaspHlsPlayer from "../../../build/es6/ts-main/index.js";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import { createTestWorker } from "../../utils/create_test_worker.js";
import {
  eventListener,
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import sleep from "../../utils/sleep.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";

// Loading alone may take 90 seconds; leave room for the subsequent event waits.
export const TEST_TIMEOUT_MS = 240_000;
const CONFIG_PROPAGATION_DELAY_MS = 100;
const BACKOFF_TOLERANCE_MS = 200;
const BACKOFF_JITTER_FACTOR = 0.3;

export const MULTIVARIANT_CONFIG = {
  multiVariantPlaylistMaxRetry: 1,
  multiVariantPlaylistRequestTimeout: 50,
  multiVariantPlaylistBackoffBase: 1,
  multiVariantPlaylistBackoffMax: 1,
};

export const MEDIA_PLAYLIST_CONFIG = {
  mediaPlaylistMaxRetry: 1,
  mediaPlaylistRequestTimeout: 50,
  mediaPlaylistBackoffBase: 1,
  mediaPlaylistBackoffMax: 1,
};

export const SEGMENT_CONFIG = {
  segmentMaxRetry: 1,
  segmentRequestTimeout: 50,
  segmentBackoffBase: 1,
  segmentBackoffMax: 1,
};

export function createTopLevelFetchRule(actions) {
  return {
    id: "master-playlist",
    match: {
      urlEndsWith: "/vod/scenario/fmp4-multivariant-no-codecs/master.m3u8",
    },
    actions,
  };
}

export function createMediaPlaylistFetchRule(actions) {
  return {
    id: "media-playlist",
    match: {
      urlEndsWith: "/vod/scenario/fmp4-multivariant-no-codecs/variant.m3u8",
    },
    actions,
  };
}

export function createSegmentFetchRule(actions) {
  return {
    id: "segment-request",
    match: {
      urlMatches: "/vod/generated/fmp4-muxed-av/seg-[0-9]+\\.m4s$",
    },
    actions,
  };
}

export function repeatAction(action, count) {
  return Array.from({ length: count }, () => ({ ...action }));
}

export async function createPlayerHarness({
  playerConfig,
  fetchRules,
  initialBandwidth,
}) {
  const videoElement = document.createElement("video");
  document.body.appendChild(videoElement);

  let lastPlayerError = null;
  const workerHandle = createTestWorker({ fetchRules });
  const player = new WaspHlsPlayer(videoElement, playerConfig);
  player.addEventListener("error", (error) => {
    lastPlayerError = error;
  });
  await player.initialize({
    workerUrl: workerHandle.url,
    wasmUrl: EmbeddedWasm,
    initialBandwidth,
  });

  return {
    player,
    videoElement,
    workerHandle,
    getLastPlayerError() {
      return lastPlayerError;
    },
    dispose() {
      player.dispose();
      videoElement.removeAttribute("src");
      workerHandle.dispose();
      document.body.removeChild(videoElement);
    },
  };
}

export async function applyConfigUpdate(player, configUpdate) {
  player.updateConfig(configUpdate);
  await sleep(CONFIG_PROPAGATION_DELAY_MS);
}

export async function expectFatalRequestError({
  scenarioId,
  playerConfig,
  configUpdate,
  fetchRules,
  ruleId,
  expectedErrorName,
  expectedErrorCode,
  expectedAttempts,
  expectedWarnings,
}) {
  const ctx = await createPlayerHarness({ playerConfig, fetchRules });

  try {
    if (configUpdate !== undefined) {
      await applyConfigUpdate(ctx.player, configUpdate);
    }

    const warnings = eventListener(ctx.player, "warning");
    const errorPromise = waitForPlayerEvent(ctx.player, "error");

    ctx.player.load(getVodScenarioUrl(scenarioId));

    const error = await errorPromise;
    expect(error.name).toBe(expectedErrorName);
    expect(error.code).toBe(expectedErrorCode);

    const attempts = await ctx.workerHandle.telemetry.waitForCount(
      (evt) => evt.type === "fetch-start" && evt.ruleId === ruleId,
      expectedAttempts,
    );
    expect(attempts).toHaveLength(expectedAttempts);

    await sleep(50);
    expect(warnings.getCurrentCount()).toBe(expectedWarnings);
  } finally {
    ctx.dispose();
  }
}

export async function expectLoadAfterSingleRetry({
  scenarioId,
  playerConfig,
  configUpdate,
  fetchRules,
  ruleId,
  expectedWarningCodes,
}) {
  const ctx = await createPlayerHarness({ playerConfig, fetchRules });

  try {
    if (configUpdate !== undefined) {
      await applyConfigUpdate(ctx.player, configUpdate);
    }

    const warnings = eventListener(ctx.player, "warning");
    ctx.player.addEventListener("playerStateChange", (state) => {
      if (state === "Loaded") {
        ctx.player.resume();
      }
    });

    ctx.player.load(getVodScenarioUrl(scenarioId));
    await waitForLoadedState(ctx.player, ctx.videoElement, () =>
      ctx.getLastPlayerError(),
    );

    const attempts = await ctx.workerHandle.telemetry.waitForCount(
      (evt) => evt.type === "fetch-start" && evt.ruleId === ruleId,
      2,
    );
    expect(attempts.length).toBeGreaterThanOrEqual(2);
    expect(attempts[0].attempt).toBe(1);
    expect(attempts[1].attempt).toBe(2);

    await sleep(50);
    expect(warnings.getCurrentCount()).toBe(1);
    if (expectedWarningCodes !== undefined) {
      expect(getEventCodes(warnings)).toEqual(expectedWarningCodes);
    }
    expect(ctx.player.getError()).toBeNull();
    expect(ctx.getLastPlayerError()).toBeNull();
  } finally {
    ctx.dispose();
  }
}

export function getEventCodes(eventCollector) {
  return Array.from(
    { length: eventCollector.getCurrentCount() },
    (_, index) => {
      return eventCollector.getPayloadFor(index).code;
    },
  );
}

export async function expectRetryBackoffRangeSequence({
  scenarioId,
  playerConfig,
  configUpdate,
  fetchRules,
  ruleId,
  expectedNominalDelaysMs,
}) {
  const ctx = await createPlayerHarness({ playerConfig, fetchRules });

  try {
    if (configUpdate !== undefined) {
      await applyConfigUpdate(ctx.player, configUpdate);
    }

    ctx.player.addEventListener("playerStateChange", (state) => {
      if (state === "Loaded") {
        ctx.player.resume();
      }
    });

    ctx.player.load(getVodScenarioUrl(scenarioId));
    await waitForLoadedState(ctx.player, ctx.videoElement, () =>
      ctx.getLastPlayerError(),
    );

    const attempts = await ctx.workerHandle.telemetry.waitForCount(
      (evt) => evt.type === "fetch-start" && evt.ruleId === ruleId,
      expectedNominalDelaysMs.length + 1,
    );

    for (let i = 0; i < expectedNominalDelaysMs.length; i++) {
      const observedDelayMs =
        attempts[i + 1].timestampMs - attempts[i].timestampMs;
      expectDelayInBackoffRange(observedDelayMs, expectedNominalDelaysMs[i]);
    }
  } finally {
    ctx.dispose();
  }
}

export async function expectMixedRetryFailure({
  scenarioId,
  playerConfig,
  configUpdate,
  fetchRules,
  ruleId,
  expectedWarningCodes,
  expectedFinalErrorCode,
  expectedAttempts,
}) {
  const ctx = await createPlayerHarness({ playerConfig, fetchRules });

  try {
    if (configUpdate !== undefined) {
      await applyConfigUpdate(ctx.player, configUpdate);
    }

    const warnings = eventListener(ctx.player, "warning");
    const errorPromise = waitForPlayerEvent(ctx.player, "error");

    ctx.player.load(getVodScenarioUrl(scenarioId));
    const error = await errorPromise;

    expect(error.code).toBe(expectedFinalErrorCode);
    const attempts = await ctx.workerHandle.telemetry.waitForCount(
      (evt) => evt.type === "fetch-start" && evt.ruleId === ruleId,
      expectedAttempts,
    );
    expect(attempts).toHaveLength(expectedAttempts);

    await sleep(50);
    expect(getEventCodes(warnings)).toEqual(expectedWarningCodes);
  } finally {
    ctx.dispose();
  }
}

function expectDelayInBackoffRange(actualDelayMs, nominalDelayMs) {
  const minDelayMs =
    nominalDelayMs * (1 - BACKOFF_JITTER_FACTOR) - BACKOFF_TOLERANCE_MS;
  const maxDelayMs =
    nominalDelayMs * (1 + BACKOFF_JITTER_FACTOR) + BACKOFF_TOLERANCE_MS;
  expect(actualDelayMs).toBeGreaterThanOrEqual(minDelayMs);
  expect(actualDelayMs).toBeLessThanOrEqual(maxDelayMs);
}
