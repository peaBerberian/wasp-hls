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
  it(
    "exposes subtitle metadata without making subtitle requests gate playback",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const master = await readPlaylist(url);
      const tag =
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Français",LANGUAGE="fr",DEFAULT=YES,AUTOSELECT=YES,FORCED=YES,CHARACTERISTICS="public.accessibility.transcribes-spoken-dialog",URI="missing-subs.m3u8"';
      const body = master
        .replace("#EXTM3U\n", `#EXTM3U\n${tag}\n`)
        .replace("#EXT-X-STREAM-INF:", '#EXT-X-STREAM-INF:SUBTITLES="subs",');
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        const announced = [];
        ctx.player.addEventListener("subtitleTrackListUpdate", (tracks) =>
          announced.push(tracks),
        );
        await expectPlayback(ctx, url);
        const tracks = ctx.player.getSubtitleTrackList();
        expect(tracks).toHaveLength(1);
        expect(tracks[0]).toMatchObject({
          name: "Français",
          language: "fr",
          groupId: "subs",
          default: true,
          autoselect: true,
          forced: true,
          characteristics: ["public.accessibility.transcribes-spoken-dialog"],
        });
        expect(announced.at(-1)).toEqual(tracks);
        expect(ctx.player.getCurrentSubtitleTrack()).toBeUndefined();
        expect(() => ctx.player.setSubtitleTrack(-1)).toThrow(
          "Unknown subtitle track",
        );
        ctx.player.setSubtitleTrack(tracks[0].id);
        ctx.player.setSubtitleTrack(null);
        ctx.player.stop();
        expect(ctx.player.getSubtitleTrackList()).toEqual([]);
      } finally {
        ctx.dispose();
      }
    },
  );

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
  it.each(["REQ-FUTURE=YES", "HDCP-LEVEL=FUTURE", "VIDEO-RANGE=FUTURE"])(
    "plays a variant carrying %s",
    { timeout: TEST_TIMEOUT_MS },
    async (attribute) => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const master = await readPlaylist(url);
      const body = master.replace(
        "#EXT-X-STREAM-INF:",
        `#EXT-X-STREAM-INF:${attribute},`,
      );
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        await expectPlayback(ctx, url);
        expect(ctx.player.getVariantList()).toHaveLength(1);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "retains renditions and initialization tags with unknown attributes and enum values",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const mediaUrl = new URL("video.m3u8", url).href;
      const [master, media] = await Promise.all([
        readPlaylist(url),
        readPlaylist(mediaUrl),
      ]);
      const define = '#EXT-X-DEFINE:NAME="unused",VALUE="value",FUTURE=123\n';
      const compatibilityTags = [
        '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="Future",DEFAULT=FUTURE,AUTOSELECT=FUTURE,FORCED=FUTURE,URI="audio-fr.m3u8"',
        '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="Required",REQ-FUTURE=YES,URI="audio-fr.m3u8"',
        "#EXT-X-START:TIME-OFFSET=0,PRECISE=FUTURE,REQ-FUTURE=YES",
      ].join("\n");
      const ctx = await createPlayerHarness({
        fetchRules: [
          playlistRule(
            url,
            master.replace(
              "#EXTM3U\n",
              `#EXTM3U\n${define}${compatibilityTags}\n`,
            ),
          ),
          playlistRule(
            mediaUrl,
            media
              .replace("#EXTM3U\n", `#EXTM3U\n${define}`)
              .replace("#EXT-X-MAP:", "#EXT-X-MAP:REQ-FUTURE=YES,"),
          ),
        ],
      });
      try {
        await expectPlayback(ctx, url);
        expect(ctx.player.getAudioTrackList()).toHaveLength(4);
        expect(ctx.player.getCurrentAudioTrack()?.language).toBe("en");
        expect(ctx.player.getPosition()).toBeLessThan(5);
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each([true, false])(
    "validates variables in GAP URIs (defined: %s)",
    { timeout: TEST_TIMEOUT_MS },
    async (defined) => {
      const url = await ensureVodScenarioReady("fmp4-direct-media");
      const original = await readPlaylist(url);
      const definition = defined
        ? '#EXT-X-DEFINE:NAME="gap",VALUE="missing"\n'
        : "";
      const body = original
        .replace("#EXTM3U\n", `#EXTM3U\n${definition}`)
        .replace(
          "#EXT-X-ENDLIST",
          "#EXTINF:4,\n#EXT-X-GAP\n{$gap}.m4s\n#EXT-X-ENDLIST",
        );
      expect(body).toContain("{$gap}.m4s");
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        if (defined) {
          await expectPlayback(ctx, url);
          expect(
            ctx.workerHandle.telemetry
              .getEvents()
              .some(
                (event) =>
                  event.type === "fetch-start" &&
                  event.url.endsWith("/missing.m4s"),
              ),
          ).toBe(false);
        } else {
          const errorPromise = waitForPlayerEvent(ctx.player, "error");
          ctx.player.load(url);
          const error = await errorPromise;
          expect(error.code).toBe("MediaPlaylistVariableDefinitionError");
          expect(ctx.player.getPlayerState()).toBe("Error");
        }
      } finally {
        ctx.dispose();
      }
    },
  );
});
