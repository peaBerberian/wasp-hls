import { describe, expect, it } from "vitest";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";
import sleep from "../../utils/sleep.js";
import { waitForLoadedState } from "../../utils/player_test_tools.js";
import setupPlayer from "../utils/player_setup";
import { assertStartupBehavior } from "../utils/startup_test_tools.js";

const LIVE_PLAYBACK_TEST_TIMEOUT_MS = 240_000;
const LIVE_MAX_INITIAL_SEEK_DELAY_MS = 20_000;
const LIVE_MAX_LOADED_DELAY_MS = 45_000;
const LIVE_POSITION_TOLERANCE_S = 2.5;
const LIVE_WINDOW_DRIFT_TOLERANCE_S = 4;
const LIVE_PROGRAM_DATE_TIME_TOLERANCE_S = 6;

describe("Live packaged content", function () {
  const ctx = setupPlayer({ packageLiveContent: true });

  const LIVE_STARTING_POSITION_CASES = [
    {
      name: "defaults to a safe distance from the live edge without `startingPosition`",
      options: undefined,
      expectInitialSeek: true,
      assertLoadedSnapshot(snapshot, timings, context) {
        const initialSeek = assertInitialSeekSurvivesLoad(snapshot, timings);
        const gap = initialSeek.maximumPosition - initialSeek.mediaPosition;
        expect(gap).toBeGreaterThanOrEqual(context.segmentDuration * 3 - 0.5);
        expect(gap).toBeLessThanOrEqual(context.segmentDuration * 4 + 1);
        const loadedGap = snapshot.maximumPosition - snapshot.position;
        expect(loadedGap).toBeGreaterThanOrEqual(
          context.segmentDuration * 3 - 0.5,
        );
        expect(loadedGap).toBeLessThanOrEqual(
          context.segmentDuration * 4 + 1 + LIVE_WINDOW_DRIFT_TOLERANCE_S,
        );
      },
    },
    {
      name: "honors numeric absolute `startingPosition` on live",
      options: { startingPosition: 6 },
      expectInitialSeek: true,
      assertLoadedSnapshot(snapshot, timings) {
        const position = assertInitialSeekSurvivesLoad(
          snapshot,
          timings,
        ).mediaPosition;
        expect(position).toBeGreaterThanOrEqual(6 - LIVE_POSITION_TOLERANCE_S);
        expect(position).toBeLessThanOrEqual(6 + LIVE_POSITION_TOLERANCE_S);
        expect(snapshot.position).toBeGreaterThanOrEqual(
          6 - LIVE_POSITION_TOLERANCE_S,
        );
        expect(snapshot.position).toBeLessThanOrEqual(
          6 + LIVE_POSITION_TOLERANCE_S,
        );
      },
    },
    {
      name: "honors object absolute `startingPosition` on live",
      options: {
        startingPosition: {
          startType: "Absolute",
          position: 8,
        },
      },
      expectInitialSeek: true,
      assertLoadedSnapshot(snapshot, timings) {
        const position = assertInitialSeekSurvivesLoad(
          snapshot,
          timings,
        ).mediaPosition;
        expect(position).toBeGreaterThanOrEqual(8 - LIVE_POSITION_TOLERANCE_S);
        expect(position).toBeLessThanOrEqual(8 + LIVE_POSITION_TOLERANCE_S);
        expect(snapshot.position).toBeGreaterThanOrEqual(
          8 - LIVE_POSITION_TOLERANCE_S,
        );
        expect(snapshot.position).toBeLessThanOrEqual(
          8 + LIVE_POSITION_TOLERANCE_S,
        );
      },
    },
    {
      name: "honors `FromBeginning` `startingPosition` on live",
      options: {
        startingPosition: {
          startType: "FromBeginning",
          position: 4,
        },
      },
      expectInitialSeek: true,
      assertLoadedSnapshot(snapshot, timings) {
        const initialSeek = assertInitialSeekSurvivesLoad(snapshot, timings);
        const expectedPosition = initialSeek.minimumPosition + 4;
        expect(initialSeek.mediaPosition).toBeGreaterThanOrEqual(
          expectedPosition - LIVE_POSITION_TOLERANCE_S,
        );
        expect(initialSeek.mediaPosition).toBeLessThanOrEqual(
          expectedPosition + LIVE_POSITION_TOLERANCE_S,
        );
        expect(snapshot.position).toBeGreaterThanOrEqual(
          4 - LIVE_POSITION_TOLERANCE_S,
        );
        expect(snapshot.position).toBeLessThanOrEqual(
          4 + LIVE_POSITION_TOLERANCE_S + LIVE_WINDOW_DRIFT_TOLERANCE_S,
        );
      },
    },
    {
      name: "honors `FromEnd` `startingPosition` on live",
      options: {
        startingPosition: {
          startType: "FromEnd",
          position: 8,
        },
      },
      expectInitialSeek: true,
      assertLoadedSnapshot(snapshot, timings) {
        const initialSeek = assertInitialSeekSurvivesLoad(snapshot, timings);
        const gap = initialSeek.maximumPosition - initialSeek.mediaPosition;
        expect(gap).toBeGreaterThanOrEqual(8 - LIVE_POSITION_TOLERANCE_S);
        expect(gap).toBeLessThanOrEqual(8 + LIVE_POSITION_TOLERANCE_S);
        const loadedGap = snapshot.maximumPosition - snapshot.position;
        expect(loadedGap).toBeGreaterThanOrEqual(8 - LIVE_POSITION_TOLERANCE_S);
        expect(loadedGap).toBeLessThanOrEqual(
          8 + LIVE_POSITION_TOLERANCE_S + LIVE_WINDOW_DRIFT_TOLERANCE_S,
        );
      },
    },
  ];

  it(
    "should fetch, update and play the Manifest",
    { timeout: LIVE_PLAYBACK_TEST_TIMEOUT_MS },
    async function () {
      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(ctx.liveInfo.playlistUrl);
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      expect(ctx.player.isLive()).toEqual(true);

      const basePos = ctx.player.getPosition();
      const baseMin = ctx.player.getMinimumPosition();
      const baseMax = ctx.player.getMaximumPosition();
      const baseSeekableMin = ctx.player.getSeekableMinimumPosition();
      const baseSeekableMax = ctx.player.getSeekableMaximumPosition();
      const baseGap = baseMax - basePos;

      expect(baseGap).toBeGreaterThan(3);
      expect(baseGap).toBeLessThan(20);

      await checkAfterSleepWithBackoff(
        {
          minTimeMs: 2000,
          maxTimeMs: 45000,
          stepMs: 1000,
        },
        () => {
          const shortMin = ctx.player.getMinimumPosition();
          const shortMax = ctx.player.getMaximumPosition();
          const shortSeekableMin = ctx.player.getSeekableMinimumPosition();
          const shortSeekableMax = ctx.player.getSeekableMaximumPosition();
          expect(shortMin - baseMin).toBeGreaterThanOrEqual(1.5);
          expect(shortMax - baseMax).toBeGreaterThanOrEqual(1.5);
          expect(shortSeekableMin).toBeGreaterThanOrEqual(baseSeekableMin);
          expect(shortSeekableMax).toBeGreaterThanOrEqual(baseSeekableMax);
        },
      );

      const secondsWaiting = 100;
      await sleep(secondsWaiting * 1000);

      const newPos = ctx.player.getPosition();
      const newMin = ctx.player.getMinimumPosition();
      const newMax = ctx.player.getMaximumPosition();
      const newSeekableMin = ctx.player.getSeekableMinimumPosition();
      const newSeekableMax = ctx.player.getSeekableMaximumPosition();

      expect(newMax - baseMax).toBeGreaterThanOrEqual(secondsWaiting * 0.8);
      expect(newMin - baseMin).toBeGreaterThanOrEqual(secondsWaiting * 0.8);
      expect(newSeekableMax - baseSeekableMax).toBeGreaterThanOrEqual(
        secondsWaiting * 0.8,
      );
      expect(newSeekableMin - baseSeekableMin).toBeGreaterThanOrEqual(
        secondsWaiting * 0.8,
      );
      expect(newMax - newPos).toBeGreaterThan(3);
      expect(newMax - newPos).toBeLessThan(20);
      expect(newPos - basePos).toBeGreaterThanOrEqual(secondsWaiting * 0.8);
      expect(ctx.liveInfo.segmentDuration).toBeGreaterThan(0);
      expect(ctx.liveInfo.timeShiftBufferDepth).toBeGreaterThan(0);
    },
  );

  for (const testCase of LIVE_STARTING_POSITION_CASES) {
    it(testCase.name, { timeout: LIVE_PLAYBACK_TEST_TIMEOUT_MS }, async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(ctx.liveInfo.playlistUrl, testCase.options);
        },
        expectInitialSeek: testCase.expectInitialSeek,
        maxInitialSeekDelayMs: LIVE_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: LIVE_MAX_LOADED_DELAY_MS,
        loadedSnapshotContext: {
          segmentDuration: ctx.liveInfo.segmentDuration,
          timeShiftBufferDepth: ctx.liveInfo.timeShiftBufferDepth,
        },
        assertLoadedSnapshot(snapshot, timings) {
          expect(snapshot.playerState).toEqual("Loaded");
          expect(snapshot.playerError).toBeNull();
          expect(snapshot.maximumPosition).toBeGreaterThan(
            snapshot.minimumPosition,
          );
          testCase.assertLoadedSnapshot(snapshot, timings, {
            segmentDuration: ctx.liveInfo.segmentDuration,
            timeShiftBufferDepth: ctx.liveInfo.timeShiftBufferDepth,
          });
        },
      });
    });
  }
});

describe("Live packaged content with EXT-X-PROGRAM-DATE-TIME", function () {
  const ctx = setupPlayer({
    packageLiveContent: { emitProgramDateTime: true },
  });

  it(
    "uses EXT-X-PROGRAM-DATE-TIME as playlist time in the public position API",
    { timeout: LIVE_PLAYBACK_TEST_TIMEOUT_MS },
    async () => {
      const anchorProgramDateTime = await getLiveProgramDateTimeAnchor(
        ctx.liveInfo.playlistUrl,
      );

      ctx.player.addEventListener("playerStateChange", (state) => {
        if (state === "Loaded") {
          ctx.player.resume();
        }
      });

      ctx.player.load(ctx.liveInfo.playlistUrl);
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );

      const minimumPosition = ctx.player.getMinimumPosition();
      const seekableMinimumPosition = ctx.player.getSeekableMinimumPosition();
      const currentPosition = ctx.player.getPosition();
      const currentDate = ctx.player.positionToDate(currentPosition);
      const mediaOffset = ctx.player.getMediaOffset();
      const liveEdgeGap = ctx.player.getMaximumPosition() - currentPosition;

      expect(ctx.player.isLive()).toEqual(true);
      expect(ctx.player.usesProgramDateTime()).toBe(true);
      expect(minimumPosition).toBeGreaterThanOrEqual(anchorProgramDateTime);
      expect(minimumPosition).toBeLessThanOrEqual(
        anchorProgramDateTime + LIVE_PROGRAM_DATE_TIME_TOLERANCE_S,
      );
      expect(seekableMinimumPosition).toBeGreaterThanOrEqual(
        anchorProgramDateTime,
      );
      expect(seekableMinimumPosition).toBeLessThanOrEqual(
        anchorProgramDateTime + LIVE_PROGRAM_DATE_TIME_TOLERANCE_S,
      );
      expect(currentDate).toBeInstanceOf(Date);
      expect(
        Math.abs((currentDate?.getTime() ?? NaN) - currentPosition * 1000),
      ).toBeLessThan(250);
      expect(liveEdgeGap).toBeGreaterThan(3);
      expect(liveEdgeGap).toBeLessThan(20);
      expect(
        Math.abs(
          currentPosition - (ctx.videoElement.currentTime - (mediaOffset ?? 0)),
        ),
      ).toBeLessThan(0.25);
      expect(mediaOffset).toBeDefined();
    },
  );

  it(
    "defaults to a safe distance from the live edge on a PDT timeline",
    { timeout: LIVE_PLAYBACK_TEST_TIMEOUT_MS },
    async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(ctx.liveInfo.playlistUrl);
        },
        expectInitialSeek: true,
        maxInitialSeekDelayMs: LIVE_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: LIVE_MAX_LOADED_DELAY_MS,
        loadedSnapshotContext: {
          segmentDuration: ctx.liveInfo.segmentDuration,
        },
        assertLoadedSnapshot(snapshot, timings, context) {
          const initialSeek = assertInitialSeekSurvivesLoad(snapshot, timings);
          const initialGap =
            initialSeek.maximumPosition - initialSeek.mediaPosition;
          const loadedGap = snapshot.maximumPosition - snapshot.position;

          expect(snapshot.playerState).toEqual("Loaded");
          expect(snapshot.playerError).toBeNull();
          expect(snapshot.usesProgramDateTime).toBe(true);
          expect(initialGap).toBeGreaterThanOrEqual(
            context.segmentDuration * 3 - 0.5,
          );
          expect(initialGap).toBeLessThanOrEqual(
            context.segmentDuration * 4 + 1,
          );
          expect(loadedGap).toBeGreaterThanOrEqual(
            context.segmentDuration * 3 - 0.5 - LIVE_WINDOW_DRIFT_TOLERANCE_S,
          );
          expect(loadedGap).toBeLessThanOrEqual(
            context.segmentDuration * 4 + 1,
          );
        },
      });
    },
  );

  it(
    "interprets `startingPosition` against the live PDT timeline",
    { timeout: LIVE_PLAYBACK_TEST_TIMEOUT_MS },
    async () => {
      const anchorProgramDateTime = await getLiveProgramDateTimeAnchor(
        ctx.liveInfo.playlistUrl,
      );

      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(ctx.liveInfo.playlistUrl, {
            startingPosition: {
              startType: "FromBeginning",
              position: 4,
            },
          });
        },
        expectInitialSeek: true,
        maxInitialSeekDelayMs: LIVE_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: LIVE_MAX_LOADED_DELAY_MS,
        assertLoadedSnapshot(snapshot) {
          expect(snapshot.playerState).toEqual("Loaded");
          expect(snapshot.playerError).toBeNull();
          expect(snapshot.usesProgramDateTime).toBe(true);
          expect(snapshot.position).toBeGreaterThanOrEqual(
            anchorProgramDateTime + 4 - LIVE_POSITION_TOLERANCE_S,
          );
          expect(snapshot.position).toBeLessThanOrEqual(
            anchorProgramDateTime + 4 + LIVE_PROGRAM_DATE_TIME_TOLERANCE_S,
          );
        },
      });
    },
  );
});

function extractPlaylistReferences(playlistText) {
  return playlistText
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

async function getLiveProgramDateTimeAnchor(playlistUrl) {
  const masterResponse = await fetch(playlistUrl);
  const masterText = await masterResponse.text();
  const variantRef = extractPlaylistReferences(masterText).find((line) =>
    line.endsWith(".m3u8"),
  );
  if (variantRef === undefined) {
    throw new Error("Unable to find a media playlist in the live master");
  }

  const mediaPlaylistUrl = new URL(variantRef, playlistUrl).href;
  const mediaResponse = await fetch(mediaPlaylistUrl);
  const mediaText = await mediaResponse.text();
  const lines = mediaText
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  let pendingDuration = null;
  let accumulatedDurationBeforeFirstPdt = 0;
  for (const line of lines) {
    const durationMatch = /^#EXTINF:([0-9.]+)/.exec(line);
    if (durationMatch !== null) {
      pendingDuration = Number.parseFloat(durationMatch[1]);
      continue;
    }

    const pdtMatch = /^#EXT-X-PROGRAM-DATE-TIME:(.+)$/.exec(line);
    if (pdtMatch !== null) {
      return Date.parse(pdtMatch[1]) / 1000 - accumulatedDurationBeforeFirstPdt;
    }

    if (!line.startsWith("#")) {
      accumulatedDurationBeforeFirstPdt += pendingDuration ?? 0;
      pendingDuration = null;
    }
  }

  throw new Error("Unable to find EXT-X-PROGRAM-DATE-TIME in live media");
}

function assertInitialSeekSurvivesLoad(snapshot, timings) {
  const initialSeek = timings.initialSeekSnapshot;
  expect(initialSeek).toBeDefined();
  expect(initialSeek.mediaPosition).toBeDefined();
  expect(
    Math.abs(snapshot.currentTime - initialSeek.currentTime),
  ).toBeLessThanOrEqual(LIVE_POSITION_TOLERANCE_S);
  expect(snapshot.position).toBeGreaterThanOrEqual(snapshot.minimumPosition);
  expect(snapshot.position).toBeLessThanOrEqual(snapshot.maximumPosition);
  return initialSeek;
}
