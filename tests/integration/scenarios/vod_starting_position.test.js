import { describe, expect, it } from "vitest";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import setupPlayer from "../utils/player_setup";
import { assertStartupBehavior } from "../utils/startup_test_tools.js";

const VOD_TEST_TIMEOUT_MS = 120_000;
const VOD_START_POSITION_TOLERANCE_S = 0.35;
const VOD_MAX_INITIAL_SEEK_DELAY_MS = 20_000;
const VOD_MAX_LOADED_DELAY_MS = 45_000;
const PROGRAM_DATE_TIME_START = Date.parse("2024-01-02T03:04:05.000Z") / 1000;

const VOD_STARTING_POSITION_CASES = [
  {
    name: "defaults to the beginning without `startingPosition`",
    options: undefined,
    expectedPosition: 0,
    expectInitialSeek: false,
  },
  {
    name: "honors numeric absolute `startingPosition`",
    options: { startingPosition: 4 },
    expectedPosition: 4,
    expectInitialSeek: true,
  },
  {
    name: "honors object absolute `startingPosition`",
    options: {
      startingPosition: {
        startType: "Absolute",
        position: 6,
      },
    },
    expectedPosition: 6,
    expectInitialSeek: true,
  },
  {
    name: "honors `FromBeginning` `startingPosition`",
    options: {
      startingPosition: {
        startType: "FromBeginning",
        position: 3,
      },
    },
    expectedPosition: 3,
    expectInitialSeek: true,
  },
  {
    name: "honors `FromEnd` `startingPosition`",
    options: {
      startingPosition: {
        startType: "FromEnd",
        position: 5,
      },
    },
    expectedPosition: 7,
    expectInitialSeek: true,
  },
];

describe("Generated VoD content - starting position", function () {
  const ctx = setupPlayer();

  for (const testCase of VOD_STARTING_POSITION_CASES) {
    it(testCase.name, { timeout: VOD_TEST_TIMEOUT_MS }, async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(
            getVodScenarioUrl("fmp4-player-api"),
            testCase.options,
          );
        },
        expectInitialSeek: testCase.expectInitialSeek,
        maxInitialSeekDelayMs: VOD_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: VOD_MAX_LOADED_DELAY_MS,
        assertLoadedSnapshot(snapshot) {
          expect(snapshot.playerState).toEqual("Loaded");
          expect(snapshot.playerError).toBeNull();
          expect(snapshot.position).toBeGreaterThanOrEqual(
            testCase.expectedPosition - VOD_START_POSITION_TOLERANCE_S,
          );
          expect(snapshot.position).toBeLessThanOrEqual(
            testCase.expectedPosition + VOD_START_POSITION_TOLERANCE_S,
          );
        },
      });
    });
  }

  it(
    "interprets `startingPosition` against the PDT-based playlist timeline",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(
            getVodScenarioUrl("fmp4-player-api-program-date-time"),
            {
              startingPosition: {
                startType: "FromBeginning",
                position: 6,
              },
            },
          );
        },
        expectInitialSeek: true,
        maxInitialSeekDelayMs: VOD_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: VOD_MAX_LOADED_DELAY_MS,
        assertLoadedSnapshot(snapshot) {
          expect(snapshot.position).toBeGreaterThanOrEqual(
            PROGRAM_DATE_TIME_START + 5.7,
          );
          expect(snapshot.position).toBeLessThanOrEqual(
            PROGRAM_DATE_TIME_START + 6.3,
          );
        },
      });
    },
  );

  it(
    "defaults to the playlist `EXT-X-START` point when no API override is set",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(getVodScenarioUrl("fmp4-player-api-ext-x-start"));
        },
        expectInitialSeek: true,
        maxInitialSeekDelayMs: VOD_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: VOD_MAX_LOADED_DELAY_MS,
        assertLoadedSnapshot(snapshot) {
          expect(snapshot.position).toBeGreaterThanOrEqual(5.7);
          expect(snapshot.position).toBeLessThanOrEqual(6.3);
        },
      });
    },
  );

  it(
    "snaps `EXT-X-START` to the containing segment start when `PRECISE=NO`",
    { timeout: VOD_TEST_TIMEOUT_MS },
    async () => {
      await assertStartupBehavior({
        player: ctx.player,
        videoElement: ctx.videoElement,
        lastPlayerErrorRef: () => ctx.lastPlayerError,
        loadContent() {
          ctx.player.load(
            getVodScenarioUrl("fmp4-player-api-ext-x-start-imprecise"),
          );
        },
        expectInitialSeek: true,
        maxInitialSeekDelayMs: VOD_MAX_INITIAL_SEEK_DELAY_MS,
        maxLoadedDelayMs: VOD_MAX_LOADED_DELAY_MS,
        assertLoadedSnapshot(snapshot) {
          expect(snapshot.position).toBeGreaterThanOrEqual(3.7);
          expect(snapshot.position).toBeLessThanOrEqual(4.3);
        },
      });
    },
  );
});
