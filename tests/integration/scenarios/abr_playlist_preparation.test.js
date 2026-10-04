import { describe, expect, it } from "vitest";
import {
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createPlayerHarness,
  createTopLevelFetchRule,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("ABR playlist preparation", () => {
  it.each([false, true])(
    "continues safe additions and rechecks a prepared playlist (stale=%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (makeRecommendationStale) => {
      const masterUrl = getVodScenarioUrl("fmp4-multivariant-no-codecs");
      const baseUrl = new URL("/vod/generated/fmp4-abr/", masterUrl);
      const response = await fetch(`${baseUrl}main.m3u8`);
      expect(response.ok).toBe(true);
      const playlist = await response.text();
      const mediaPlaylist = (quality) =>
        playlist.replace(
          /(init\.mp4|seg-\d+\.m4s)/g,
          `${baseUrl}$1?quality=${quality}`,
        );
      const ctx = await createPlayerHarness({
        initialBandwidth: 1_000_000,
        playerConfig: { bufferGoal: 40 },
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              body: `#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.64001f,mp4a.40.2"
${baseUrl}low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=12000000,CODECS="avc1.64001f,mp4a.40.2"
${baseUrl}high.m3u8
`,
            },
          ]),
          {
            match: { urlEndsWith: "low.m3u8" },
            actions: [{ type: "response", body: mediaPlaylist("low") }],
          },
          {
            id: "best-playlist",
            match: { urlEndsWith: "high.m3u8" },
            actions: [
              {
                type: "response",
                body: mediaPlaylist("high"),
                waitForRelease: true,
              },
            ],
          },
          {
            id: "safe-segments",
            match: { urlMatches: "seg-\\d+\\.m4s\\?quality=low$" },
            actions: [{ type: "passthrough", delayMs: 500 }],
          },
          {
            id: "best-segments",
            match: { urlMatches: "seg-\\d+\\.m4s\\?quality=high$" },
            actions: [{ type: "passthrough", delayMs: 500 }],
          },
        ],
      });
      try {
        const initial = waitForPlayerEvent(ctx.player, "variantUpdate");
        ctx.player.load(masterUrl);
        const low = await initial;
        ctx.player.lockVariant(low.id);
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        await expect
          .poll(() => bufferedEnd(ctx.videoElement), { timeout: 15_000 })
          .toBeGreaterThanOrEqual(20);
        const originalEnd = bufferedEnd(ctx.videoElement);
        ctx.player.unlockVariant();
        const preparing = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" && event.ruleId === "best-playlist",
        );
        const fallback = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" &&
            event.ruleId === "safe-segments" &&
            event.requestId > preparing.requestId,
        );
        expect(segmentStart(fallback)).toBeGreaterThanOrEqual(
          originalEnd - 0.1,
        );
        expect(
          ctx.workerHandle.telemetry
            .getEvents()
            .some(
              (event) =>
                event.type === "fetch-start" &&
                event.ruleId === "best-segments",
            ),
        ).toBe(false);
        if (makeRecommendationStale) {
          await ctx.workerHandle.telemetry.waitFor(
            (event) =>
              event.type === "fetch-resolve" &&
              event.requestId === fallback.requestId,
          );
          ctx.player.setSpeed(8);
          await expect
            .poll(() => ctx.player.getCurrentVariant().bandwidth)
            .toBe(1_000_000);
        }
        ctx.workerHandle.telemetry.releaseFetch(preparing.requestId);
        if (makeRecommendationStale) {
          await ctx.workerHandle.telemetry.waitFor(
            (event) =>
              event.type === "fetch-resolve" &&
              event.requestId === preparing.requestId,
          );
          await ctx.workerHandle.telemetry.waitFor(
            (event) =>
              event.type === "fetch-start" &&
              event.ruleId === "safe-segments" &&
              event.requestId > fallback.requestId,
          );
          expect(
            ctx.workerHandle.telemetry
              .getEvents()
              .some(
                (event) =>
                  event.type === "fetch-start" &&
                  event.ruleId === "best-segments",
              ),
          ).toBe(false);
        } else {
          const best = await ctx.workerHandle.telemetry.waitFor(
            (event) =>
              event.type === "fetch-start" && event.ruleId === "best-segments",
          );
          expect(segmentStart(best)).toBeGreaterThanOrEqual(
            segmentStart(fallback) + 2,
          );
        }
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );
});

function bufferedEnd(videoElement) {
  return videoElement.buffered.length === 0
    ? 0
    : videoElement.buffered.end(videoElement.buffered.length - 1);
}

function segmentStart(event) {
  return Number(/seg-(\d+)\.m4s/.exec(event.url)[1]) * 2;
}
