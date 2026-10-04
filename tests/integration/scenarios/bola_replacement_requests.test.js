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

describe("BOLA replacement requests", function () {
  it(
    "stops replacing after a bandwidth drop and resumes after recovery",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const masterUrl = getVodScenarioUrl("fmp4-multivariant-no-codecs");
      const baseUrl = new URL("/vod/generated/fmp4-abr/", masterUrl);
      const response = await fetch(`${baseUrl}main.m3u8`);
      expect(response.ok).toBe(true);
      const playlist = await response.text();
      // Reuse encoded bytes: distinct URLs supply the two scheduling quality contexts.
      const mediaPlaylist = (quality) =>
        playlist.replace(
          /(init\.mp4|seg-\d+\.m4s)/g,
          `${baseUrl}$1?quality=${quality}`,
        );
      const ctx = await createPlayerHarness({
        initialBandwidth: 1_000_000,
        playerConfig: { bufferGoal: 48 },
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              body: `#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.64001f,mp4a.40.2"
${baseUrl}low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=8000000,CODECS="avc1.64001f,mp4a.40.2"
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
            actions: [{ type: "passthrough", delayMs: 100 }],
          },
          {
            id: "high-segments",
            match: { urlMatches: "seg-\\d+\\.m4s\\?quality=high$" },
            actions: [
              // The slow request must still stop replacements. The 8 Mb/s
              // variant leaves recovery headroom for real localhost fetches.
              { type: "passthrough", delayMs: 3000 },
              { type: "passthrough", delayMs: 20 },
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
        // Keep playback paused so old lower-quality segments remain available for replacement.
        await expect
          .poll(() => bufferedEnd(ctx.videoElement), { timeout: 10_000 })
          .toBeGreaterThanOrEqual(20);
        const originalEnd = bufferedEnd(ctx.videoElement);

        ctx.player.unlockVariant();
        const firstReplacement = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" && event.ruleId === "high-segments",
        );
        expect(segmentStart(firstReplacement)).toBeLessThan(originalEnd);
        const addition = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" &&
            event.ruleId === "high-segments" &&
            event.attempt === 2,
        );
        expect(segmentStart(addition)).toBeGreaterThanOrEqual(originalEnd);
        expect(ctx.player.getCurrentVariant().bandwidth).toBe(8_000_000);
        const replacementAfterRecovery = await ctx.workerHandle.telemetry
          .waitFor(
            (event) =>
              event.type === "fetch-start" &&
              event.ruleId === "high-segments" &&
              event.requestId > addition.requestId &&
              segmentStart(event) < originalEnd,
            // Recovery requires several downloads, which can take longer on CI.
            60_000,
          )
          .catch((cause) => {
            throw new Error(
              "BOLA replacement recovery failed: " +
                JSON.stringify({
                  originalEnd,
                  bufferedEnd: bufferedEnd(ctx.videoElement),
                  currentVariant: ctx.player.getCurrentVariant(),
                  lastPlayerError: ctx.getLastPlayerError(),
                  recentSegmentEvents: ctx.workerHandle.telemetry
                    .getEvents()
                    .filter(
                      (event) =>
                        event.ruleId === "low-segments" ||
                        event.ruleId === "high-segments",
                    )
                    .slice(-20),
                }),
              { cause },
            );
          });
        expect(segmentStart(replacementAfterRecovery)).toBeGreaterThan(
          segmentStart(firstReplacement),
        );
        expect(ctx.player.getCurrentVariant().bandwidth).toBe(8_000_000);
        expect(
          ctx.workerHandle.telemetry
            .getEvents()
            .filter(
              (event) =>
                event.type === "fetch-start" &&
                event.url.endsWith("init.mp4?quality=high"),
            ),
        ).toHaveLength(1);

        await ctx.player.resume();
        await expect
          .poll(() => ctx.videoElement.currentTime, { timeout: 5000 })
          .toBeGreaterThan(0.25);
        expect(ctx.player.getPlayerState()).toBe("Loaded");
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
  return Number(/seg-(\d+)\.m4s/.exec(event.url)[1]) * 2;
}
