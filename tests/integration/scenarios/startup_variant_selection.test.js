import { describe, expect, it } from "vitest";
import { waitForPlayerEvent } from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createPlayerHarness,
  createTopLevelFetchRule,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Startup variant selection", function () {
  it(
    "fails when the initial audio track has no compatible variant",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const ctx = await createPlayerHarness({
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              body: `#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"
video.m3u8
`,
            },
          ]),
        ],
      });

      try {
        const errorPromise = waitForPlayerEvent(ctx.player, "error");
        const errorStatePromise = waitForPlayerEvent(
          ctx.player,
          "playerStateChange",
          (state) => state === "Error",
        );

        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"), {
          initialAudioTrack: { language: "fr" },
        });

        const [error] = await Promise.all([errorPromise, errorStatePromise]);
        expect(error.name).toBe("WaspContentCompatibilityError");
        expect(error.code).toBe("NoSupportedVariant");
        expect(ctx.player.getPlayerState()).toBe("Error");
        expect(ctx.player.getError()).toBe(error);
      } finally {
        ctx.dispose();
      }
    },
  );
});
