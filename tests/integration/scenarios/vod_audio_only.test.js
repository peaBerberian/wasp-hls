import { afterEach, describe, expect, it, vi } from "vitest";
import {
  getPlayerStateSnapshot,
  waitForLoadedState,
} from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import setupPlayer from "../utils/player_setup";

describe("Generated VoD content - audio only", function () {
  const ctx = setupPlayer();
  afterEach(() => vi.restoreAllMocks());

  it(
    "plays an audio-only variant without an AUDIO group",
    { timeout: 180_000 },
    async () => {
      let appendCount = 0;
      let appendFailure = null;
      const observedBuffers = new WeakSet();
      const appendBuffer = SourceBuffer.prototype.appendBuffer;
      const recordFailure = (error) => {
        // The fatal-error handler disposes the media source. Capture native details before that.
        appendFailure = {
          appendCount,
          cause:
            error instanceof Error
              ? `${error.name}: ${error.message}`
              : "SourceBuffer error event",
          mediaError: ctx.videoElement.error?.message,
          mediaErrorCode: ctx.videoElement.error?.code,
          ...getPlayerStateSnapshot(
            ctx.player,
            ctx.videoElement,
            ctx.lastPlayerError,
          ),
        };
      };
      vi.spyOn(SourceBuffer.prototype, "appendBuffer").mockImplementation(
        function (data) {
          appendCount++;
          if (!observedBuffers.has(this)) {
            observedBuffers.add(this);
            this.addEventListener("error", recordFailure);
          }
          try {
            appendBuffer.call(this, data);
          } catch (error) {
            recordFailure(error);
            throw error;
          }
        },
      );
      ctx.player.load(getVodScenarioUrl("fmp4-audio-only"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);
      const startPosition = ctx.player.getPosition();
      await ctx.player.resume();
      // Allow slow machines time to start playback after reaching Loaded.
      await checkAfterSleepWithBackoff({ maxTimeMs: 60_000 }, () => {
        expect(ctx.lastPlayerError, JSON.stringify(appendFailure)).toBeNull();
        expect(ctx.player.getPosition()).toBeGreaterThan(startPosition + 1);
      });
      expect(ctx.lastPlayerError).toBeNull();
    },
  );
});
