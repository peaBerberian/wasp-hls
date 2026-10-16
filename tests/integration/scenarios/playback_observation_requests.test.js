import { afterEach, expect, it } from "vitest";
import { createPlayerHarness } from "../utils/request_integration_test_tools.js";
import PlaybackObserver from "../../../build/es6/ts-main/observePlayback.js";
import { onRequestPlaybackObservationMessage } from "../../../build/es6/ts-main/worker-message-handlers.js";
import { PlaybackTickReason } from "../../../build/es6/ts-common/types.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import { waitForPlayerEvent } from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";

let observer;
afterEach(() => {
  observer?.stop();
  observer = undefined;
});

function createObserver() {
  const element = Object.assign(new EventTarget(), {
    currentTime: 0,
    readyState: 0,
    paused: true,
    seeking: false,
    ended: false,
    duration: 12,
    buffered: { length: 0, start: () => 0, end: () => 0 },
  });
  observer = new PlaybackObserver(element, 300);
  const samples = [];
  observer.addEventListener("newObservation", (sample) => samples.push(sample));
  observer.start();
  samples.length = 0;
  return { element, samples };
}

it("reads every playback property as soon as an observation is requested", () => {
  const { element, samples } = createObserver();
  Object.assign(element, {
    currentTime: 4,
    readyState: 3,
    paused: false,
    seeking: true,
    ended: true,
    duration: 20,
    buffered: { length: 1, start: () => 2, end: () => 9 },
  });
  observer.requestObservation();
  expect(samples).toHaveLength(1);
  expect(samples[0]).toMatchObject({
    reason: PlaybackTickReason.Requested,
    currentTime: 4,
    readyState: 3,
    paused: false,
    seeking: true,
    ended: true,
    duration: 20,
  });
  expect(Array.from(samples[0].buffered)).toEqual([2, 9]);
  element.currentTime = 5;
  observer.requestObservation();
  expect(samples).toHaveLength(2);
  expect(samples[1].currentTime).toBe(5);
});

it("ignores requests for stopped observers or another MediaSource", () => {
  const { samples } = createObserver();
  const metadata = { mediaSourceId: "current", playbackObserver: observer };
  onRequestPlaybackObservationMessage(
    { value: { mediaSourceId: "old" } },
    metadata,
  );
  onRequestPlaybackObservationMessage(
    { value: { mediaSourceId: "current" } },
    null,
  );
  expect(samples).toHaveLength(0);
  observer.stop();
  onRequestPlaybackObservationMessage(
    { value: { mediaSourceId: "current" } },
    metadata,
  );
  expect(samples).toHaveLength(0);
  observer.start();
  expect(samples.map((sample) => sample.reason)).toEqual([
    PlaybackTickReason.Init,
  ]);
  onRequestPlaybackObservationMessage(
    { value: { mediaSourceId: "current" } },
    metadata,
  );
  expect(samples.map((sample) => sample.reason)).toEqual([
    PlaybackTickReason.Init,
    PlaybackTickReason.Requested,
  ]);
});

const mseLocations = [false];
if (
  typeof MediaSource === "function" &&
  MediaSource.canConstructInDedicatedWorker === true
) {
  mseLocations.push(true);
}

it.each(mseLocations)(
  "leaves startup buffering after an append without a periodic observation (MSE in worker: %s)",
  { timeout: 20_000 },
  async (mseInWorker) => {
    const postMessage = Worker.prototype.postMessage;
    const observations = [];
    let ctx;
    // Remove the timer's help: this test must receive a full observation prompted
    // by the append, rather than merely becoming Loaded before a time limit.
    Worker.prototype.postMessage = function (message, ...args) {
      if (message.type === "init") message.value.hasMseInWorker = mseInWorker;
      if (message.type === "obs") {
        if (message.value.reason === PlaybackTickReason.RegularInterval) return;
        observations.push(message.value);
      }
      return postMessage.call(this, message, ...args);
    };
    try {
      ctx = await createPlayerHarness({
        playerConfig: { bufferGoal: 6 },
        fetchRules: [
          {
            id: "second-segment",
            match: { urlEndsWith: "seg-001.ts" },
            actions: [{ type: "passthrough", waitForRelease: true }],
          },
        ],
      });
      const canplay = new Promise((resolve) =>
        ctx.videoElement.addEventListener("canplay", resolve, { once: true }),
      );
      ctx.player.load(getVodScenarioUrl("mpegts-direct-media"));
      const held = await ctx.workerHandle.telemetry.waitFor(
        (event) =>
          event.type === "fetch-start" && event.ruleId === "second-segment",
      );
      await canplay;
      // The first timestamps start just after zero. Let the normal startup seek
      // enter that range before asserting how much is playable from currentTime.
      await checkAfterSleepWithBackoff(
        { minTimeMs: 10, maxTimeMs: 1_000, stepMs: 10 },
        () => {
          expect(ctx.videoElement.seeking).toBe(false);
          expect(ctx.videoElement.currentTime).toBeGreaterThanOrEqual(
            ctx.videoElement.buffered.start(0),
          );
          expect(
            ctx.videoElement.buffered.end(0) - ctx.videoElement.currentTime,
          ).toBeLessThan(2);
        },
      );
      expect(ctx.player.getPlayerState()).not.toBe("Loaded");
      observations.length = 0;
      const loaded = waitForPlayerEvent(
        ctx.player,
        "playerStateChange",
        (state) => state === "Loaded",
        5_000,
      );
      ctx.workerHandle.telemetry.releaseFetch(held.requestId);
      await loaded;
      expect(
        observations.some(
          (sample) =>
            sample.reason === PlaybackTickReason.Requested &&
            sample.buffered[1] - sample.currentTime >= 2,
        ),
      ).toBe(true);
      expect(ctx.getLastPlayerError()).toBe(null);
    } finally {
      ctx?.dispose();
      Worker.prototype.postMessage = postMessage;
    }
  },
);
