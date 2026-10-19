import { describe, expect, it } from "vitest";
import {
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { ensureVodScenarioReady } from "../../utils/vod_scenarios.js";
import {
  createPlayerHarness,
  createTopLevelFetchRule,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Startup variant selection", function () {
  it(
    "uses default selection when no variant supports the initial audio track",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const response = await fetch(url);
      const master = await response.text();
      const body = master.replace(
        "#EXTM3U\n",
        '#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"\n',
      );
      const ctx = await createPlayerHarness({
        fetchRules: [
          createTopLevelFetchRule([{ type: "response", status: 200, body }]),
        ],
      });

      try {
        ctx.player.load(url, { initialAudioTrack: { language: "fr" } });
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        expect(ctx.player.getCurrentVariant()).toBeDefined();
        expect(ctx.player.getCurrentAudioTrack()?.name).not.toBe("French");

        const progressed = waitForPlayerEvent(
          ctx.videoElement,
          "timeupdate",
          () => ctx.videoElement.currentTime > 0.5,
        );
        ctx.player.resume();
        await progressed;
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        expect(ctx.player.getError()).toBeNull();
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );
});
