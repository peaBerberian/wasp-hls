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

describe("BOLA requests after seeking", function () {
  it.each([false, true])(
    "uses the empty target's buffer level while preserving a manual lock (%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (lockHighVariant) => {
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
        playerConfig: { bufferGoal: 24 },
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
          ...["low", "high"].map((quality) => ({
            match: { urlEndsWith: `${quality}.m3u8` },
            actions: [{ type: "response", body: mediaPlaylist(quality) }],
          })),
          {
            id: "low-segments",
            match: { urlMatches: "seg-\\d+\\.m4s\\?quality=low$" },
            actions: [{ type: "passthrough", delayMs: 500 }],
          },
          {
            id: "high-segments",
            match: { urlMatches: "seg-\\d+\\.m4s\\?quality=high$" },
            actions: [
              { type: "passthrough", delayMs: 5000 },
              { type: "passthrough", delayMs: 500 },
            ],
          },
        ],
      });

      try {
        const initial = waitForPlayerEvent(ctx.player, "variantUpdate");
        ctx.player.load(masterUrl);
        const lowVariant = await initial;
        expect(lowVariant.bandwidth).toBe(1_000_000);
        ctx.player.lockVariant(lowVariant.id);
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        // Paused playback builds enough buffer to fund an above-throughput upgrade.
        await expect
          .poll(() => bufferedEnd(ctx.videoElement), { timeout: 15_000 })
          .toBeGreaterThanOrEqual(20);
        const originalEnd = bufferedEnd(ctx.videoElement);
        ctx.player.unlockVariant();
        const highRequest = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" && event.ruleId === "high-segments",
        );
        expect(segmentStart(highRequest)).toBeGreaterThanOrEqual(originalEnd);
        await expect
          .poll(() => ctx.player.getCurrentVariant().bandwidth)
          .toBe(12_000_000);
        if (lockHighVariant) {
          const highVariant = ctx.player.getCurrentVariant();
          const locked = waitForPlayerEvent(
            ctx.player,
            "variantLockUpdate",
            (variant) => variant?.id === highVariant.id,
          );
          ctx.player.lockVariant(highVariant.id);
          await locked;
          const restartedHighRequest = await ctx.workerHandle.telemetry.waitFor(
            (event) =>
              event.type === "fetch-start" &&
              event.ruleId === "high-segments" &&
              event.requestId > highRequest.requestId &&
              segmentStart(event) < originalEnd,
          );
          expect(segmentStart(restartedHighRequest)).toBeLessThan(originalEnd);
        }

        const seekPosition = 40;
        expect(bufferedEnd(ctx.videoElement)).toBeLessThan(seekPosition);
        ctx.videoElement.currentTime = seekPosition;
        const firstAtTarget = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" &&
            event.requestId > highRequest.requestId &&
            segmentStart(event) >= seekPosition - 2,
        );
        expect(firstAtTarget.ruleId).toBe(
          lockHighVariant ? "high-segments" : "low-segments",
        );

        await ctx.player.resume();
        await expect
          .poll(() => ctx.videoElement.currentTime, { timeout: 10_000 })
          .toBeGreaterThan(seekPosition + 0.25);
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );
});

function bufferedEnd(videoElement) {
  const buffered = videoElement.buffered;
  return buffered.length === 0 ? 0 : buffered.end(buffered.length - 1);
}

function segmentStart(event) {
  const match = /seg-(\d+)\.m4s/.exec(event.url);
  return match === null ? -Infinity : Number(match[1]) * 2;
}
