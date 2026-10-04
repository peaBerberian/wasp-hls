import { describe, expect, it } from "vitest";
import {
  eventListener,
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createMediaPlaylistFetchRule,
  createPlayerHarness,
  createTopLevelFetchRule,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Deselected media playlist failures", function () {
  it(
    "still reports a fatal parse failure for the selected media playlist",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const ctx = await createPlayerHarness({
        fetchRules: [
          createMediaPlaylistFetchRule([
            { type: "response", status: 200, body: "invalid playlist" },
          ]),
        ],
      });
      try {
        const failed = waitForPlayerEvent(ctx.player, "error");
        const stopped = waitForPlayerEvent(
          ctx.player,
          "playerStateChange",
          (state) => state === "Error",
        );
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"));
        const [error] = await Promise.all([failed, stopped]);
        expect(error.name).toBe("WaspMediaPlaylistParsingError");
        expect(ctx.player.getError()).toBe(error);
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each(["status", "network", "timeout", "parse"])(
    "ignores an obsolete %s failure and allows selecting the playlist again",
    { timeout: TEST_TIMEOUT_MS },
    async (failure) => {
      const baseUrl = new URL(
        ".",
        getVodScenarioUrl("fmp4-multivariant-no-codecs"),
      );
      const alternativeUrl = `${baseUrl}variant.m3u8?rendition=alternative`;
      const action =
        failure === "status"
          ? { type: "response", status: 500, waitForRelease: true }
          : failure === "network"
            ? { type: "error", waitForRelease: true }
            : failure === "timeout"
              ? { type: "timeout", waitForRelease: true }
              : {
                  type: "response",
                  status: 200,
                  body: "invalid playlist",
                  waitForRelease: true,
                };
      const ctx = await createPlayerHarness({
        initialBandwidth: 2_500_000,
        playerConfig: {
          // Keep this 12-second fixture from reaching end-of-stream before switching.
          bufferGoal: 4,
          mediaPlaylistMaxRetry: 1,
          mediaPlaylistRequestTimeout: 700,
          mediaPlaylistBackoffBase: 1,
          mediaPlaylistBackoffMax: 1,
        },
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              // Partial scores keep automatic selection on the initial variant.
              body: `#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1900000
${baseUrl}variant.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4000000000,SCORE=1
${alternativeUrl}
`,
            },
          ]),
          {
            id: "alternative-playlist",
            match: { urlEndsWith: "variant.m3u8?rendition=alternative" },
            actions: [action, { type: "passthrough" }],
          },
        ],
      });

      try {
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"));
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const initialVariant = ctx.player.getCurrentVariant();
        const alternativeVariant = ctx.player
          .getVariantList()
          .find((variant) => variant.id !== initialVariant.id);
        const warnings = eventListener(ctx.player, "warning");

        ctx.player.lockVariant(alternativeVariant.id);
        const pendingRequest = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" && event.url === alternativeUrl,
        );
        const restored = waitForPlayerEvent(
          ctx.player,
          "variantUpdate",
          (variant) => variant.id === initialVariant.id,
        );
        ctx.player.lockVariant(initialVariant.id);
        await restored;
        // variantUpdate is sent after the worker has applied the deselection.
        // Only now allow the obsolete response (or its real timeout) to finish.
        ctx.workerHandle.telemetry.releaseFetch(pendingRequest.requestId);
        const completedRequest = await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.requestId === pendingRequest.requestId &&
            ["fetch-resolve", "fetch-reject", "fetch-abort"].includes(
              event.type,
            ),
        );
        expect(completedRequest.type).toBe(
          failure === "network"
            ? "fetch-reject"
            : failure === "timeout"
              ? "fetch-abort"
              : "fetch-resolve",
        );

        await playUntilProgress(ctx.player, ctx.videoElement);
        expect(ctx.player.getCurrentVariant().id).toBe(initialVariant.id);
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        expect(ctx.getLastPlayerError()).toBeNull();
        expect(warnings.getCurrentCount()).toBe(0);
        expect(alternativeRequests(ctx)).toHaveLength(1);

        // A failed obsolete request must not remain pending or poison the cache.
        const changed = waitForPlayerEvent(
          ctx.player,
          "variantUpdate",
          (variant) => variant.id === alternativeVariant.id,
        );
        ctx.player.lockVariant(alternativeVariant.id);
        await changed;
        await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-resolve" &&
            event.url === alternativeUrl &&
            event.attempt === 2,
        );
        await playUntilProgress(ctx.player, ctx.videoElement);
        expect(alternativeRequests(ctx)).toHaveLength(2);
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        expect(ctx.getLastPlayerError()).toBeNull();
        expect(warnings.getCurrentCount()).toBe(0);
      } finally {
        ctx.dispose();
      }
    },
  );
});

function alternativeRequests(ctx) {
  return ctx.workerHandle.telemetry
    .getEvents()
    .filter(
      (event) =>
        event.type === "fetch-start" && event.ruleId === "alternative-playlist",
    );
}

async function playUntilProgress(player, videoElement) {
  const initialTime = videoElement.currentTime;
  try {
    await player.resume();
    await expect
      .poll(() => videoElement.currentTime, { timeout: 20_000 })
      .toBeGreaterThan(initialTime + 0.25);
  } finally {
    // Keep the next request/selection waits from consuming the rest of the clip.
    videoElement.pause();
  }
}
