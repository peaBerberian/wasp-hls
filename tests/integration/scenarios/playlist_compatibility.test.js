import { describe, expect, it } from "vitest";
import {
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { ensureVodScenarioReady } from "../../utils/vod_scenarios.js";
import { createPlayerHarness } from "../utils/request_integration_test_tools.js";

const TEST_TIMEOUT_MS = 120_000;

async function readPlaylist(url) {
  const response = await fetch(url);
  expect(response.ok).toBe(true);
  return response.text();
}

function playlistRule(url, body) {
  return {
    match: { urlEndsWith: new URL(url).pathname },
    actions: [{ type: "response", status: 200, body }],
  };
}

async function expectPlayback(ctx, url) {
  ctx.player.load(url);
  await waitForLoadedState(ctx.player, ctx.videoElement, () =>
    ctx.getLastPlayerError(),
  );
  const timeUpdate = waitForPlayerEvent(
    ctx.videoElement,
    "timeupdate",
    () => ctx.videoElement.currentTime > 0.5,
  );
  ctx.player.resume();
  await timeUpdate;
  expect(ctx.player.getError()).toBeNull();
}

describe("Playlist compatibility", () => {
  it.each(["en", "fr"])(
    "selects the DEFAULT=YES audio rendition (%s) regardless of declaration order",
    { timeout: TEST_TIMEOUT_MS },
    async (defaultLanguage) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const original = await readPlaylist(url);
      const body = original.replace(
        /(#EXT-X-MEDIA:TYPE=AUDIO,[^\n]+)/g,
        (line) =>
          line.replace(
            /DEFAULT=(YES|NO)/,
            `DEFAULT=${line.includes(`LANGUAGE="${defaultLanguage}"`) ? "YES" : "NO"}`,
          ),
      );
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        await expectPlayback(ctx, url);
        expect(ctx.player.getCurrentAudioTrack()?.language).toBe(
          defaultLanguage,
        );
        expect(ctx.player.getAudioTrackList()).toHaveLength(2);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "plays through unknown tags in multivariant and media playlists",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const mediaUrl = new URL("variant.m3u8", url).href;
      const [master, media] = await Promise.all([
        readPlaylist(url),
        readPlaylist(mediaUrl),
      ]);
      const addUnknownTags = (body) =>
        body.replace(
          "#EXTM3U\n",
          "#EXTM3U\n#EXT-X-FUTURE\n#EXT-X-FUTURE-ATTR:VALUE=YES\n",
        );
      const ctx = await createPlayerHarness({
        fetchRules: [
          playlistRule(url, addUnknownTags(master)),
          playlistRule(mediaUrl, addUnknownTags(media)),
        ],
      });
      try {
        await expectPlayback(ctx, url);
        expect(ctx.player.getVariantList()).toHaveLength(1);
      } finally {
        ctx.dispose();
      }
    },
  );
});
