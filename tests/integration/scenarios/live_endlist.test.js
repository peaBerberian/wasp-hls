import { describe, expect, it } from "vitest";
import sleep from "../../utils/sleep.js";
import {
  eventListener,
  getPlayerStateSnapshot,
  waitForLoadedState,
} from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";
import setupPlayer from "../utils/player_setup";

const LIVE_ENDLIST_TEST_TIMEOUT_MS = 90_000;
const EVENT_ENDLIST_SCENARIO_PREFIX = "/live/scenario/event-endlist";

function getBaseUrl() {
  return (
    "http://" + __TEST_CONTENT_SERVER__.URL + ":" + __TEST_CONTENT_SERVER__.PORT
  );
}

async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    throw new Error(`Unexpected status ${response.status} for ${url}`);
  }
  return await response.json();
}

async function resetEventEndlistScenario() {
  return await fetchJson(
    `${getBaseUrl()}${EVENT_ENDLIST_SCENARIO_PREFIX}/reset`,
    {
      method: "POST",
    },
  );
}

describe("Live content - EXT-X-ENDLIST", function () {
  const ctx = setupPlayer();

  it(
    "plays through an EVENT playlist that ends with EXT-X-ENDLIST",
    { timeout: LIVE_ENDLIST_TEST_TIMEOUT_MS },
    async () => {
      const { playlistUrl } = await resetEventEndlistScenario();

      const endedListener = eventListener(ctx.player, "ended");

      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(playlistUrl);
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      const initialMaximumPosition = ctx.player.getMaximumPosition();

      await sleep(2_500);
      const maximumPositionAfterRefresh = ctx.player.getMaximumPosition();
      expect(maximumPositionAfterRefresh).toBeGreaterThan(
        (initialMaximumPosition ?? 0) + 1.5,
      );

      // TODO: Once we have a supported worker-to-main test hook for network
      // requests, assert directly that playlist reloads stop after ENDLIST.
      await checkAfterSleepWithBackoff(
        { minTimeMs: 2_000, maxTimeMs: 20_000, stepMs: 250 },
        () => {
          expect(ctx.player.getPosition()).toBeGreaterThan(6);
          expect(ctx.lastPlayerError).toEqual(null);
          expect(endedListener.getCurrentCount()).toEqual(0);
        },
      );

      const now = performance.now();
      try {
        await endedListener.awaitNext();
      } catch (error) {
        throw new Error(
          `${error instanceof Error ? error.message : String(error)}: ${JSON.stringify(
            getPlayerStateSnapshot(
              ctx.player,
              ctx.videoElement,
              ctx.lastPlayerError,
            ),
          )}`,
        );
      }
      expect(performance.now() - now).toBeLessThan(20_000);
      expect(endedListener.getCurrentCount()).toEqual(1);
      expect(ctx.player.isEnded()).toEqual(true);
      expect(ctx.player.getPosition()).toBeGreaterThan(11.5);
      expect(ctx.player.getPosition()).toBeCloseTo(
        ctx.player.getMaximumPosition(),
        0.5,
      );
      expect(ctx.lastPlayerError).toEqual(null);
      const finalMaximumPosition = ctx.player.getMaximumPosition();
      await sleep(2_500);
      expect(ctx.player.getMaximumPosition()).toBeCloseTo(
        finalMaximumPosition,
        1,
      );
      expect(endedListener.getCurrentCount()).toEqual(1);
      expect(ctx.player.isEnded()).toEqual(true);
      expect(ctx.player.getPosition()).toBeGreaterThan(11.5);
      expect(ctx.player.getPosition()).toBeCloseTo(
        ctx.player.getMaximumPosition(),
        0.5,
      );
      expect(ctx.lastPlayerError).toEqual(null);
    },
  );
});
