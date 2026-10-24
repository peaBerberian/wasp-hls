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
  it.each(["", "\ufeff"])(
    "rejects a text-only main playlist after probing WebVTT (BOM: %j)",
    { timeout: TEST_TIMEOUT_MS },
    async (bom) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const segmentUrl = new URL("text-only.vtt", url).href;
      const ctx = await createPlayerHarness({
        fetchRules: [
          playlistRule(
            url,
            `#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n${segmentUrl}\n#EXT-X-ENDLIST\n`,
          ),
          playlistRule(
            segmentUrl,
            `${bom}WEBVTT\n\n00:00:00.000 --> 00:00:04.000\nHello\n`,
          ),
        ],
      });
      try {
        const failed = waitForPlayerEvent(ctx.player, "error");
        ctx.player.load(url);
        const error = await failed;
        expect(error.name).toBe("WaspContentCompatibilityError");
        expect(error.message).toContain(
          "Text-only main content is not supported",
        );
        expect(ctx.player.getPlayerState()).toBe("Error");
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each(["same-selector", "ordered", "fallback"])(
    "skips unused audio groups in initial preferences (%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (mode) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const master = await readPlaylist(url);
      const body = master.replace(
        "#EXTM3U\n",
        `#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="Unused",LANGUAGE="en",URI="unused.m3u8"\n`,
      );
      const initialAudioTrack =
        mode === "ordered"
          ? [{ name: "Unused" }, { language: "fr" }]
          : mode === "fallback"
            ? { name: "Unused" }
            : { language: "en" };
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        const updates = [];
        ctx.player.addEventListener("audioTrackUpdate", (track) =>
          updates.push(track),
        );
        ctx.player.load(url, { initialAudioTrack });
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const expectedLanguage = mode === "ordered" ? "fr" : "en";
        expect(ctx.player.getCurrentAudioTrack()?.language).toBe(
          expectedLanguage,
        );
        expect(updates[0]?.language).toBe(expectedLanguage);
        expect(ctx.player.getCurrentAudioTrack()?.name).not.toBe("Unused");
        expect(ctx.player.getError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each([
    "single",
    "ordered",
    "unmatched",
    "forced",
    "off",
    "fallback-off",
    "off-first",
    "match-before-off",
    "empty",
  ])(
    "applies %s initial subtitle preferences before the first track update",
    { timeout: TEST_TIMEOUT_MS },
    async (preferenceKind) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const master = await readPlaylist(url);
      const tags = [
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en-subs.m3u8"',
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="Forced French",LANGUAGE="fr",FORCED=YES,AUTOSELECT=YES,URI="forced-fr.m3u8"',
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="French",LANGUAGE="fr",CHARACTERISTICS="public.accessibility.transcribes-spoken-dialog",URI="fr-subs.m3u8"',
      ].join("\n");
      const body = master
        .replace("#EXTM3U\n", `#EXTM3U\n${tags}\n`)
        .replaceAll(
          "#EXT-X-STREAM-INF:",
          '#EXT-X-STREAM-INF:SUBTITLES="subs",',
        );
      const selection = {
        language: "fr",
        forced: false,
        characteristics: ["public.accessibility.transcribes-spoken-dialog"],
      };
      const initialSubtitleTrack =
        preferenceKind === "off"
          ? null
          : preferenceKind === "fallback-off"
            ? [{ language: "de" }, null, selection]
            : preferenceKind === "off-first"
              ? [null, selection]
              : preferenceKind === "match-before-off"
                ? [selection, null]
                : preferenceKind === "empty"
                  ? []
                  : preferenceKind === "ordered"
                    ? [{ language: "de" }, selection]
                    : preferenceKind === "unmatched"
                      ? { language: "de" }
                      : preferenceKind === "forced"
                        ? { forced: true }
                        : selection;
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        const updates = [];
        ctx.player.addEventListener("subtitleTrackUpdate", (track) =>
          updates.push(track),
        );
        ctx.player.load(url, {
          initialAudioTrack: { language: "fr" },
          initialSubtitleTrack,
        });
        await waitForLoadedState(ctx.player, ctx.videoElement, () =>
          ctx.getLastPlayerError(),
        );
        const expectedName = ["off", "fallback-off", "off-first"].includes(
          preferenceKind,
        )
          ? undefined
          : ["unmatched", "empty"].includes(preferenceKind)
            ? "English"
            : preferenceKind === "forced"
              ? "Forced French"
              : "French";
        expect(ctx.player.getCurrentAudioTrack()?.language).toBe("fr");
        expect(ctx.player.getCurrentSubtitleTrack()?.name).toBe(expectedName);
        expect(updates[0]?.name).toBe(expectedName);
        if (expectedName === undefined) {
          expect(ctx.player.getCurrentSubtitleTrack()).toBeNull();
          expect(updates[0]).toBeNull();
        }
        expect(ctx.player.getError()).toBeNull();

        // A new load without preferences uses automatic selection again.
        ctx.player.load(url);
        await waitForLoadedState(ctx.player, ctx.videoElement, () =>
          ctx.getLastPlayerError(),
        );
        expect(ctx.player.getCurrentSubtitleTrack()?.name).toBe("English");
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each(["missing", "empty"])(
    "preserves %s audio and subtitle names through the public API",
    { timeout: TEST_TIMEOUT_MS },
    async (nameKind) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const master = await readPlaylist(url);
      const nameAttribute = nameKind === "missing" ? "" : ',NAME=""';
      const subtitleTag = `#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs"${nameAttribute},LANGUAGE="fr",DEFAULT=YES,URI="subs.m3u8"`;
      const body = master
        .replace(/,NAME="[^"]*"/g, nameAttribute)
        .replace("#EXTM3U\n", `#EXTM3U\n${subtitleTag}\n`)
        .replaceAll(
          "#EXT-X-STREAM-INF:",
          '#EXT-X-STREAM-INF:SUBTITLES="subs",',
        );
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        const audioUpdates = [];
        const subtitleUpdates = [];
        ctx.player.addEventListener("audioTrackListUpdate", (tracks) =>
          audioUpdates.push(tracks),
        );
        ctx.player.addEventListener("subtitleTrackListUpdate", (tracks) =>
          subtitleUpdates.push(tracks),
        );
        await expectPlayback(ctx, url);
        const expectedName = nameKind === "missing" ? undefined : "";
        const audioTracks = ctx.player.getAudioTrackList();
        expect(audioTracks).toHaveLength(2);
        for (const track of audioTracks) {
          expect(track.name).toBe(expectedName);
          expect(track.characteristics).toEqual([]);
        }
        const subtitleTracks = ctx.player.getSubtitleTrackList();
        expect(subtitleTracks).toHaveLength(1);
        expect(subtitleTracks[0].name).toBe(expectedName);
        expect(ctx.player.getCurrentAudioTrack()?.name).toBe(expectedName);
        expect(ctx.player.getCurrentSubtitleTrack()?.name).toBe(expectedName);
        expect(audioUpdates.at(-1)).toEqual(audioTracks);
        expect(subtitleUpdates.at(-1)).toEqual(subtitleTracks);
      } finally {
        ctx.dispose();
      }
    },
  );

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
          forced: true,
          characteristics: ["public.accessibility.transcribes-spoken-dialog"],
        });
        expect(announced.at(-1)).toEqual(tracks);
        expect(ctx.player.getCurrentSubtitleTrack()?.id).toBe(tracks[0].id);
        const invalidSelection = waitForPlayerEvent(
          ctx.player,
          "warning",
          (warning) => warning.name === "WaspTrackSelectionError",
        );
        expect(() => ctx.player.setSubtitleTrack(0x7ffffffe)).not.toThrow();
        const warning = await invalidSelection;
        expect(warning.name).toBe("WaspTrackSelectionError");
        expect(warning.code).toBe("UnknownTrack");
        expect(warning.trackId).toBe(0x7ffffffe);
        for (const args of [[null], [undefined], []]) {
          expect(() =>
            Reflect.apply(ctx.player.setSubtitleTrack, ctx.player, args),
          ).toThrow(TypeError);
        }
        expect(ctx.player.getCurrentSubtitleTrack()?.id).toBe(tracks[0].id);
        ctx.player.setSubtitleTrack(tracks[0].id);
        const disabled = waitForPlayerEvent(
          ctx.player,
          "subtitleTrackUpdate",
          (track) => track === null,
        );
        ctx.player.disableSubtitleTrack();
        await disabled;
        expect(ctx.player.getCurrentSubtitleTrack()).toBeNull();
        const automatic = waitForPlayerEvent(
          ctx.player,
          "subtitleTrackUpdate",
          (track) => track?.id === tracks[0].id,
        );
        ctx.player.resetSubtitleTrack();
        await automatic;
        expect(ctx.player.getCurrentSubtitleTrack()?.id).toBe(tracks[0].id);
        ctx.player.stop();
        expect(ctx.player.getSubtitleTrackList()).toEqual([]);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "warns when a subtitle track has no matching variant",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const master = await readPlaylist(url);
      const body = master.replace(
        "#EXTM3U\n",
        '#EXTM3U\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"\n',
      );
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        await expectPlayback(ctx, url);
        const previousTrack = ctx.player.getCurrentSubtitleTrack();
        const previousVariant = ctx.player.getCurrentVariant();
        const track = ctx.player.getSubtitleTrackList()[0];
        expect(track).toBeDefined();
        const warningPromise = waitForPlayerEvent(ctx.player, "warning");
        ctx.player.setSubtitleTrack(track.id);
        const warning = await warningPromise;
        expect(warning.name).toBe("WaspTrackSelectionError");
        expect(warning.code).toBe("NoCompatibleVariant");
        expect(warning.trackId).toBe(track.id);
        expect(ctx.player.getCurrentSubtitleTrack()).toEqual(previousTrack);
        expect(ctx.player.getCurrentVariant()).toEqual(previousVariant);
        expect(ctx.player.getError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "selects a subtitle track from another variant group",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-multivariant-no-codecs");
      const master = await readPlaylist(url);
      const variant = master.match(/#EXT-X-STREAM-INF:[^\n]+\n[^\n]+/);
      if (variant === null) {
        throw new Error("Missing test variant");
      }
      const tags = [
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="a",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="a.m3u8"',
        '#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="b",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="b.m3u8"',
      ].join("\n");
      const body = master
        .replace("#EXTM3U\n", `#EXTM3U\n${tags}\n`)
        .replace(
          variant[0],
          variant[0].replace(
            "#EXT-X-STREAM-INF:",
            '#EXT-X-STREAM-INF:SUBTITLES="a",',
          ) +
            "\n" +
            variant[0].replace(
              "#EXT-X-STREAM-INF:",
              '#EXT-X-STREAM-INF:SUBTITLES="b",',
            ),
        );
      const ctx = await createPlayerHarness({
        fetchRules: [playlistRule(url, body)],
      });
      try {
        await expectPlayback(ctx, url);
        const current = ctx.player.getCurrentSubtitleTrack();
        expect(current).toBeTruthy();
        const other = ctx.player
          .getSubtitleTrackList()
          .find((track) => track.id !== current?.id);
        if (other === undefined) {
          throw new Error("Missing alternative subtitle track");
        }
        const selected = waitForPlayerEvent(
          ctx.player,
          "subtitleTrackUpdate",
          (track) => track?.id === other.id,
        );
        ctx.player.setSubtitleTrack(other.id);
        await selected;
        expect(ctx.player.getCurrentSubtitleTrack()?.id).toBe(other.id);
        expect(ctx.player.getError()).toBeNull();
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
