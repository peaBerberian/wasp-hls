import { describe, expect, it } from "vitest";
import {
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import sleep from "../../utils/sleep.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createMediaPlaylistFetchRule,
  createPlayerHarness,
  createSegmentFetchRule,
  createTopLevelFetchRule,
  expectFatalRequestError,
  MEDIA_PLAYLIST_CONFIG,
  MULTIVARIANT_CONFIG,
  repeatAction,
  SEGMENT_CONFIG,
  TEST_TIMEOUT_MS,
  applyConfigUpdate,
} from "../utils/request_integration_test_tools.js";

describe("Worker request injection - errors", function () {
  describe("multivariant playlist requests", function () {
    it(
      "surfaces the documented bad-status error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: MULTIVARIANT_CONFIG,
          fetchRules: [
            createTopLevelFetchRule([{ type: "response", status: 404 }]),
          ],
          ruleId: "master-playlist",
          expectedErrorName: "WaspMultivariantPlaylistRequestError",
          expectedErrorCode: "MultivariantPlaylistBadHttpStatus",
          expectedAttempts: 2,
          expectedWarnings: 1,
        });
      },
    );

    it(
      "surfaces the documented timeout error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: MULTIVARIANT_CONFIG,
          fetchRules: [createTopLevelFetchRule([{ type: "timeout" }])],
          ruleId: "master-playlist",
          expectedErrorName: "WaspMultivariantPlaylistRequestError",
          expectedErrorCode: "MultivariantPlaylistRequestTimeout",
          expectedAttempts: 2,
          expectedWarnings: 1,
        });
      },
    );

    it(
      "surfaces the documented request error without retrying network failures",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: MULTIVARIANT_CONFIG,
          fetchRules: [
            createTopLevelFetchRule([
              { type: "error", message: "Injected top-level failure" },
            ]),
          ],
          ruleId: "master-playlist",
          expectedErrorName: "WaspMultivariantPlaylistRequestError",
          expectedErrorCode: "MultivariantPlaylistRequestError",
          expectedAttempts: 1,
          expectedWarnings: 0,
        });
      },
    );
  });

  describe("media playlist requests", function () {
    it(
      "surfaces the documented bad-status error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          fetchRules: [
            createMediaPlaylistFetchRule([{ type: "response", status: 404 }]),
          ],
          ruleId: "media-playlist",
          expectedErrorName: "WaspMediaPlaylistRequestError",
          expectedErrorCode: "MediaPlaylistBadHttpStatus",
          expectedAttempts: 2,
          expectedWarnings: 1,
          configUpdate: MEDIA_PLAYLIST_CONFIG,
        });
      },
    );

    it(
      "surfaces the documented timeout error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          fetchRules: [createMediaPlaylistFetchRule([{ type: "timeout" }])],
          ruleId: "media-playlist",
          expectedErrorName: "WaspMediaPlaylistRequestError",
          expectedErrorCode: "MediaPlaylistRequestTimeout",
          expectedAttempts: 2,
          expectedWarnings: 1,
          configUpdate: MEDIA_PLAYLIST_CONFIG,
        });
      },
    );

    it(
      "surfaces the documented request error without retrying network failures",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          fetchRules: [
            createMediaPlaylistFetchRule([
              { type: "error", message: "Injected media playlist failure" },
            ]),
          ],
          ruleId: "media-playlist",
          expectedErrorName: "WaspMediaPlaylistRequestError",
          expectedErrorCode: "MediaPlaylistRequestError",
          expectedAttempts: 1,
          expectedWarnings: 0,
          configUpdate: MEDIA_PLAYLIST_CONFIG,
        });
      },
    );
  });

  describe("segment requests", function () {
    it(
      "surfaces the documented bad-status error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-direct-media",
          fetchRules: [
            createSegmentFetchRule([{ type: "response", status: 404 }]),
          ],
          ruleId: "segment-request",
          expectedErrorName: "WaspSegmentRequestError",
          expectedErrorCode: "SegmentBadHttpStatus",
          expectedAttempts: 2,
          expectedWarnings: 1,
          configUpdate: SEGMENT_CONFIG,
        });
      },
    );

    it(
      "surfaces the documented timeout error after retries are exhausted",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-direct-media",
          fetchRules: [createSegmentFetchRule([{ type: "timeout" }])],
          ruleId: "segment-request",
          expectedErrorName: "WaspSegmentRequestError",
          expectedErrorCode: "SegmentRequestTimeout",
          expectedAttempts: 2,
          expectedWarnings: 1,
          configUpdate: SEGMENT_CONFIG,
        });
      },
    );

    it(
      "surfaces the documented request error without retrying network failures",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-direct-media",
          fetchRules: [
            createSegmentFetchRule([
              { type: "error", message: "Injected segment failure" },
            ]),
          ],
          ruleId: "segment-request",
          expectedErrorName: "WaspSegmentRequestError",
          expectedErrorCode: "SegmentRequestError",
          expectedAttempts: 1,
          expectedWarnings: 0,
          configUpdate: SEGMENT_CONFIG,
        });
      },
    );

    it(
      "surfaces a segment timeout that happens after playback already started",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        const ctx = await createPlayerHarness({
          playerConfig: {
            bufferGoal: 4,
            segmentMaxRetry: 0,
            // Successful startup requests must have time to complete on slow runners.
            segmentRequestTimeout: 30_000,
          },
          fetchRules: [
            createSegmentFetchRule([
              ...repeatAction({ type: "passthrough" }, 3),
              { type: "timeout" },
            ]),
          ],
        });

        try {
          ctx.player.load(getVodScenarioUrl("fmp4-direct-media"));
          await waitForLoadedState(
            ctx.player,
            ctx.videoElement,
            ctx.getLastPlayerError,
          );
          const positionAtLoaded = ctx.player.getPosition();
          ctx.player.resume();
          await expect
            .poll(() => ctx.player.getPosition(), { timeout: 30_000 })
            .toBeGreaterThan(positionAtLoaded + 0.25);

          expect(ctx.getLastPlayerError()).toBeNull();
          const error = await waitForPlayerEvent(
            ctx.player,
            "error",
            () => true,
            60_000,
          );
          expect(error.name).toBe("WaspSegmentRequestError");
          expect(error.code).toBe("SegmentRequestTimeout");
        } finally {
          ctx.dispose();
        }
      },
    );
  });
});
