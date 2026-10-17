import { describe, expect, it } from "vitest";
import { waitForLoadedState } from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import setupPlayer from "../utils/player_setup";

describe("Generated VoD content - audio only", function () {
  const ctx = setupPlayer();

  it(
    "plays an audio-only variant without an AUDIO group",
    { timeout: 60_000 },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });
      ctx.player.load(getVodScenarioUrl("fmp4-audio-only"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);
      const startPosition = ctx.player.getPosition();
      await checkAfterSleepWithBackoff({ maxTimeMs: 5_000 }, () => {
        expect(ctx.player.getPosition()).toBeGreaterThan(startPosition + 1);
      });
      expect(ctx.lastPlayerError).toBeNull();
    },
  );
});
