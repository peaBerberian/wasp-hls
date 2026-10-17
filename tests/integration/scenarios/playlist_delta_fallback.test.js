import { describe, expect, it } from "vitest";
import {
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { ensureVodScenarioReady } from "../../utils/vod_scenarios.js";
import { createPlayerHarness } from "../utils/request_integration_test_tools.js";

const TEST_TIMEOUT_MS = 60_000;
const DELTA =
  "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-SKIP:SKIPPED-SEGMENTS=2\n";
const response = (body) => ({ type: "response", status: 200, body });

async function fixture(scenario) {
  const url = await ensureVodScenarioReady(scenario);
  const mediaUrl =
    scenario === "fmp4-direct-media" ? url : new URL("variant.m3u8", url).href;
  const full = await (await fetch(mediaUrl)).text();
  return { url, mediaUrl, full };
}

function rule(url, actions) {
  return {
    id: "playlist",
    match: { urlIncludes: new URL(url).pathname },
    actions,
  };
}

function playlistRequests(ctx) {
  return ctx.workerHandle.telemetry
    .getEvents()
    .filter(
      (event) => event.type === "fetch-start" && event.ruleId === "playlist",
    );
}

async function waitLoaded(ctx) {
  await waitForLoadedState(ctx.player, ctx.videoElement, () =>
    ctx.getLastPlayerError(),
  );
}

describe("Playlist delta fallback", () => {
  it.each(["fmp4-direct-media", "fmp4-multivariant-no-codecs"])(
    "loads a full playlist after receiving a delta (%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (scenario) => {
      const { url, mediaUrl, full } = await fixture(scenario);
      const fetchRules = [rule(mediaUrl, [response(DELTA), response(full)])];
      if (scenario !== "fmp4-direct-media") {
        const master = await (await fetch(url)).text();
        fetchRules.push({
          ...rule(url, [
            response(
              master.replace("variant.m3u8", "variant.m3u8?_HLS_skip=YES"),
            ),
          ]),
          id: "master",
        });
      }
      const ctx = await createPlayerHarness({ fetchRules });
      try {
        ctx.player.load(
          url +
            (scenario === "fmp4-direct-media"
              ? "?token=a%2Fb&repeat=1&_HLS_skip=v2&repeat=2"
              : ""),
        );
        await waitLoaded(ctx);
        const requests = playlistRequests(ctx);
        expect(requests).toHaveLength(2);
        expect(requests[1].url).toBe(
          mediaUrl +
            (scenario === "fmp4-direct-media"
              ? "?token=a%2Fb&repeat=1&repeat=2"
              : ""),
        );
        expect(ctx.player.getError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "accepts SKIPPED-SEGMENTS=0 without another request",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const { url, full } = await fixture("fmp4-direct-media");
      const body = full.replace(
        "#EXTM3U\n",
        "#EXTM3U\n#EXT-X-SKIP:SKIPPED-SEGMENTS=0\n",
      );
      const ctx = await createPlayerHarness({
        fetchRules: [rule(url, [response(body)])],
      });
      try {
        ctx.player.load(url);
        await waitLoaded(ctx);
        expect(playlistRequests(ctx)).toHaveLength(1);
      } finally {
        ctx.dispose();
      }
    },
  );

  it.each([false, true])(
    "uses the final URL for recovery and stops if redirects reintroduce skipping (%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (repeatDelta) => {
      const { url, full } = await fixture("fmp4-direct-media");
      const finalUrl = new URL(
        "redirected/playlist.m3u8?token=a%2Fb&_HLS_skip=YES",
        url,
      ).href;
      const cleanUrl = finalUrl.replace("&_HLS_skip=YES", "");
      const ctx = await createPlayerHarness({
        fetchRules: [
          rule(url, [{ ...response(DELTA), finalUrl }]),
          rule(cleanUrl, [
            { ...response(repeatDelta ? DELTA : full), finalUrl },
          ]),
        ],
      });
      try {
        const errorPromise = repeatDelta
          ? waitForPlayerEvent(ctx.player, "error")
          : null;
        ctx.player.load(url + "?_HLS_skip=YES");
        if (repeatDelta) {
          expect((await errorPromise).message).toContain("EXT-X-SKIP");
        } else {
          await waitLoaded(ctx);
        }
        const requests = playlistRequests(ctx);
        expect(requests).toHaveLength(2);
        expect(requests[1].url).toBe(cleanUrl);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "continues refreshing the primary variant without skipping after recovery",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const { url, mediaUrl, full } = await fixture(
        "fmp4-multivariant-no-codecs",
      );
      const master = await (await fetch(url)).text();
      const live = full
        .replace("#EXT-X-ENDLIST", "")
        .replace("#EXT-X-PLAYLIST-TYPE:VOD", "#EXT-X-PLAYLIST-TYPE:EVENT");
      const ctx = await createPlayerHarness({
        fetchRules: [
          {
            ...rule(url, [
              response(
                master.replace("variant.m3u8", "variant.m3u8?_HLS_skip=YES"),
              ),
            ]),
            id: "master",
          },
          rule(mediaUrl, [
            response(live),
            response(DELTA),
            response(live),
            response(full),
          ]),
        ],
      });
      try {
        ctx.player.load(url);
        await waitLoaded(ctx);
        const requests = await ctx.workerHandle.telemetry.waitForCount(
          (event) =>
            event.type === "fetch-start" && event.ruleId === "playlist",
          4,
        );
        expect(requests[2].url).toBe(mediaUrl);
        expect(requests[3].url).toBe(mediaUrl);
        await expect.poll(() => ctx.player.isVod()).toBe(true);
        expect(ctx.player.getError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "stops after a second delta instead of retrying indefinitely",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const { url } = await fixture("fmp4-direct-media");
      const ctx = await createPlayerHarness({
        fetchRules: [rule(url, [response(DELTA), response(DELTA)])],
      });
      try {
        const errorPromise = waitForPlayerEvent(ctx.player, "error");
        ctx.player.load(url + "?_HLS_skip=YES");
        const error = await errorPromise;
        expect(error.name).toBe("WaspMediaPlaylistParsingError");
        expect(error.message).toContain("EXT-X-SKIP");
        expect(ctx.player.getPlayerState()).toBe("Error");
        expect(playlistRequests(ctx)).toHaveLength(2);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "rejects an unsolicited delta without repeating the same request",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const { url } = await fixture("fmp4-direct-media");
      const ctx = await createPlayerHarness({
        fetchRules: [rule(url, [response(DELTA)])],
      });
      try {
        const errorPromise = waitForPlayerEvent(ctx.player, "error");
        ctx.player.load(url);
        expect((await errorPromise).message).toContain("EXT-X-SKIP");
        expect(playlistRequests(ctx)).toHaveLength(1);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "keeps the current playlist while recovering from a delta refresh",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const { url, full } = await fixture("fmp4-direct-media");
      const live = full
        .replace("#EXT-X-ENDLIST", "")
        .replace("#EXT-X-PLAYLIST-TYPE:VOD", "#EXT-X-PLAYLIST-TYPE:EVENT");
      const ctx = await createPlayerHarness({
        fetchRules: [
          rule(url, [
            response(live),
            response(DELTA),
            { ...response(full), waitForRelease: true },
          ]),
        ],
      });
      try {
        ctx.player.load(url + "?_HLS_skip=YES");
        await waitLoaded(ctx);
        const previousMaximum = ctx.player.getMaximumPosition();
        const attempts = await ctx.workerHandle.telemetry.waitForCount(
          (event) =>
            event.type === "fetch-start" && event.ruleId === "playlist",
          3,
        );
        expect(ctx.player.getMaximumPosition()).toBeGreaterThanOrEqual(
          previousMaximum,
        );
        expect(ctx.player.getError()).toBeNull();
        expect(ctx.player.isVod()).toBe(false);
        ctx.workerHandle.telemetry.releaseFetch(attempts[2].requestId);
        await expect.poll(() => ctx.player.isVod()).toBe(true);
        expect(ctx.player.getMaximumPosition()).toBeGreaterThan(0);
        expect(ctx.player.getError()).toBeNull();
        expect(playlistRequests(ctx)).toHaveLength(3);
      } finally {
        ctx.dispose();
      }
    },
  );
});
