import { describe, expect, it } from "vitest";
import { waitForLoadedState } from "../../utils/player_test_tools.js";
import sleep from "../../utils/sleep.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";
import setupPlayer from "../utils/player_setup";

const VOD_TEST_TIMEOUT_MS = 60_000;
const PLAYBACK_SETTLE_MS = 2_000;
const PROGRAM_DATE_TIME_START = Date.parse("2024-01-02T03:04:05.000Z") / 1000;

function expectPositionToAdvance(player, startPosition) {
  const newPosition = player.getPosition();
  expect(newPosition).toBeGreaterThan(startPosition + 0.8);
}

describe("Generated VoD content - playback", function () {
  const ctx = setupPlayer();

  it(
    "plays a direct fMP4 media playlist",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-direct-media"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isLive()).toEqual(false);
      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "plays a direct MPEG-TS media playlist",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("mpegts-direct-media"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isLive()).toEqual(false);
      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "plays a direct fMP4 media playlist using EXT-X-BYTERANGE",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-direct-media-byterange"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isLive()).toEqual(false);
      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "treats EXT-X-ENDLIST as finalized VoD even without EXT-X-PLAYLIST-TYPE",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      let latestContentInfoUpdate;

      ctx.player.addEventListener("contentInfoUpdate", (event) => {
        latestContentInfoUpdate = event;
      });
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(
        getVodScenarioUrl("fmp4-direct-media-endlist-without-playlist-type"),
      );
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isLive()).toEqual(false);
      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMaximumPosition()).toBeGreaterThan(10);
      expect(latestContentInfoUpdate?.isLive).toEqual(false);
      expect(latestContentInfoUpdate?.isVod).toEqual(true);

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "plays a multivariant fMP4 playlist without CODECS",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getCurrentVariant()).not.toBeUndefined();

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "plays a multivariant MPEG-TS playlist without CODECS",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("mpegts-multivariant-no-codecs"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getCurrentVariant()).not.toBeUndefined();

      const startPosition = ctx.player.getPosition();
      await sleep(PLAYBACK_SETTLE_MS);
      expectPositionToAdvance(ctx.player, startPosition);
    },
  );

  it(
    "exposes coherent VoD player API values",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      let latestContentInfoUpdate;

      ctx.player.addEventListener("contentInfoUpdate", (event) => {
        latestContentInfoUpdate = event;
      });
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-player-api"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      const minimumPosition = ctx.player.getMinimumPosition();
      const maximumPosition = ctx.player.getMaximumPosition();
      const seekableMinimumPosition = ctx.player.getSeekableMinimumPosition();
      const seekableMaximumPosition = ctx.player.getSeekableMaximumPosition();

      expect(ctx.player.isLive()).toEqual(false);
      expect(ctx.player.isVod()).toEqual(true);
      expect(ctx.player.getMediaDuration()).toBeGreaterThan(10);
      expect(minimumPosition).toBeDefined();
      expect(maximumPosition).toBeDefined();
      expect(seekableMinimumPosition).toBeDefined();
      expect(seekableMaximumPosition).toBeDefined();
      expect(minimumPosition).toBeLessThanOrEqual(0.1);
      expect(seekableMinimumPosition).toBeLessThanOrEqual(0.1);
      expect(maximumPosition).toBeGreaterThan(10);
      expect(seekableMaximumPosition).toBeGreaterThan(10);
      expect(latestContentInfoUpdate?.isLive).toEqual(false);

      ctx.player.seek(4);
      await sleep(1_500);

      const currentPosition = ctx.player.getPosition();
      expect(currentPosition).toBeGreaterThan(3);
      expect(currentPosition).toBeLessThan(7);

      const maximumAfterSeek = ctx.player.getMaximumPosition();
      const seekableMaximumAfterSeek = ctx.player.getSeekableMaximumPosition();

      expect(
        Math.abs((maximumAfterSeek ?? 0) - (maximumPosition ?? 0)),
      ).toBeLessThan(0.3);
      expect(
        Math.abs(
          (seekableMaximumAfterSeek ?? 0) - (seekableMaximumPosition ?? 0),
        ),
      ).toBeLessThan(0.3);
    },
  );

  it("is paused initially", { timeout: VOD_TEST_TIMEOUT_MS }, async () => {
    let receivedPausedEvent = 0;
    let receivedPlayingEvent = 0;
    ctx.player.addEventListener("paused", () => {
      receivedPausedEvent++;
    });
    ctx.player.addEventListener("playing", () => {
      receivedPlayingEvent++;
    });
    ctx.player.load(getVodScenarioUrl("fmp4-player-api"));
    await waitForLoadedState(
      ctx.player,
      ctx.videoElement,
      () => ctx.lastPlayerError,
    );
    expect(receivedPausedEvent).toEqual(0);
    expect(receivedPlayingEvent).toEqual(0);
    expect(ctx.player.isPaused()).toEqual(true);
    expect(ctx.player.isPlaying()).toEqual(false);
    ctx.player.resume();
    await checkAfterSleepWithBackoff(
      { minTimeMs: 200, maxTimeMs: 3000, stepMs: 200 },
      () => {
        expect(receivedPausedEvent).toEqual(0);
        expect(receivedPlayingEvent).toEqual(1);
        expect(ctx.player.isPaused()).toEqual(false);
        expect(ctx.player.isPlaying()).toEqual(true);
      },
    );
    ctx.player.pause();
    await checkAfterSleepWithBackoff(
      { minTimeMs: 200, maxTimeMs: 3000, stepMs: 200 },
      () => {
        expect(receivedPausedEvent).toEqual(1);
        expect(receivedPlayingEvent).toEqual(1);
        expect(ctx.player.isPaused()).toEqual(true);
        expect(ctx.player.isPlaying()).toEqual(false);
      },
    );
  });

  it("sends ended event when stream ends", { timeout: 180_000 }, async () => {
    let receivedEndedEvent = 0;
    ctx.player.addEventListener("ended", () => {
      receivedEndedEvent++;
    });
    ctx.player.load(getVodScenarioUrl("fmp4-player-api"), {
      startingPosition: { startType: "FromEnd", position: 2 },
    });
    await waitForLoadedState(
      ctx.player,
      ctx.videoElement,
      () => ctx.lastPlayerError,
    );
    // Separate loading from playback so slow startup does not consume the end-event wait.
    ctx.player.resume();
    await checkAfterSleepWithBackoff(
      { minTimeMs: 100, maxTimeMs: 30000, stepMs: 500 },
      () => {
        expect(ctx.videoElement.ended).toEqual(true);
        expect(ctx.player.isEnded()).toEqual(true);
        expect(receivedEndedEvent).toEqual(1);
      },
    );
  });

  it(
    "uses media time as playlist time when EXT-X-PROGRAM-DATE-TIME is absent",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-player-api"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.getMinimumPosition()).toBeLessThanOrEqual(0.1);
      expect(ctx.player.getSeekableMinimumPosition()).toBeLessThanOrEqual(0.1);
      expect(ctx.player.usesProgramDateTime()).toBe(false);
      expect(
        ctx.player.positionToDate(ctx.player.getPosition()),
      ).toBeUndefined();
      expect(Math.abs(ctx.player.getMediaOffset() ?? NaN)).toBeLessThan(0.1);
      expect(
        Math.abs(ctx.player.getPosition() - ctx.videoElement.currentTime),
      ).toBeLessThan(0.1);
    },
  );

  it(
    "uses EXT-X-PROGRAM-DATE-TIME as playlist time in the public position API",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(getVodScenarioUrl("fmp4-player-api-program-date-time"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      const minimumPosition = ctx.player.getMinimumPosition();
      const seekableMinimumPosition = ctx.player.getSeekableMinimumPosition();
      const maximumPosition = ctx.player.getMaximumPosition();
      const mediaOffset = ctx.player.getMediaOffset();
      const minimumDate = ctx.player.positionToDate(minimumPosition ?? NaN);
      const currentDate = ctx.player.positionToDate(ctx.player.getPosition());

      expect(ctx.player.usesProgramDateTime()).toBe(true);
      expect(minimumPosition).toBeGreaterThanOrEqual(
        PROGRAM_DATE_TIME_START - 0.1,
      );
      expect(minimumPosition).toBeLessThanOrEqual(
        PROGRAM_DATE_TIME_START + 0.1,
      );
      expect(seekableMinimumPosition).toBeGreaterThanOrEqual(
        PROGRAM_DATE_TIME_START - 0.1,
      );
      expect(seekableMinimumPosition).toBeLessThanOrEqual(
        PROGRAM_DATE_TIME_START + 0.1,
      );
      expect(maximumPosition).toBeGreaterThan(PROGRAM_DATE_TIME_START + 10);
      expect(mediaOffset).toBeLessThan(-PROGRAM_DATE_TIME_START + 0.1);
      expect(minimumDate?.getTime()).toBe(PROGRAM_DATE_TIME_START * 1000);
      expect(currentDate).toBeInstanceOf(Date);
      expect(
        Math.abs(
          ctx.player.getPosition() -
            (ctx.videoElement.currentTime - mediaOffset),
        ),
      ).toBeLessThan(0.1);

      ctx.player.seek(PROGRAM_DATE_TIME_START + 4);
      await sleep(1_500);

      const currentPosition = ctx.player.getPosition();
      expect(currentPosition).toBeGreaterThan(PROGRAM_DATE_TIME_START + 3);
      expect(currentPosition).toBeLessThan(PROGRAM_DATE_TIME_START + 7);
    },
  );
});
