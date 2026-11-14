import { afterEach, expect, it } from "vitest";
import { createPlayerHarness } from "../utils/request_integration_test_tools.js";
import PlaybackObserver from "../../../build/es6/ts-main/observePlayback.js";
import {
  onAttachMediaSourceMessage,
  onCreateMediaSourceMessage,
  onClearMediaSourceMessage,
  onStartPlaybackObservationMessage,
  onStopPlaybackObservationMessage,
  onRequestPlaybackObservationMessage,
} from "../../../build/es6/ts-main/worker-message-handlers.js";
import { PlaybackTickReason } from "../../../build/es6/ts-common/types.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import { waitForPlayerEvent } from "../../utils/player_test_tools.js";
import { checkAfterSleepWithBackoff } from "../../utils/checkAfterSleepWithBackoff.js";

let observer;
afterEach(() => {
  observer?.stop();
  observer = undefined;
});

function createMediaElement() {
  return Object.assign(new EventTarget(), {
    currentTime: 0,
    readyState: 0,
    paused: true,
    seeking: false,
    ended: false,
    duration: 12,
    buffered: { length: 0, start: () => 0, end: () => 0 },
    removeAttribute() {},
  });
}

function createObserver() {
  const element = createMediaElement();
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

it.each([
  { location: "main", create: onCreateMediaSourceMessage },
  { location: "worker", create: onAttachMediaSourceMessage },
])(
  "owns the command number for the MediaSource lifetime (MSE in $location)",
  ({ create }) => {
    const element = createMediaElement();
    const messages = [];
    const worker = { postMessage: (message) => messages.push(message) };
    const metadata = {
      contentId: "content",
      mediaSourceId: null,
      mediaSource: null,
      disposeMediaSource: null,
      sourceBuffers: [],
      playbackObserver: null,
    };
    const message = (mediaSourceId) => ({ value: { mediaSourceId } });
    const attach = (mediaSourceId) =>
      create(
        { value: { contentId: "content", mediaSourceId, src: "mock:media" } },
        metadata,
        element,
        worker,
      );
    try {
      attach("first");
      observer = metadata.playbackObserver;
      expect(observer).toBeInstanceOf(PlaybackObserver);
      observer.updateLastMediaCommandNumber(7);
      element.dispatchEvent(new Event("playing"));
      onRequestPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toEqual([]);

      onStartPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toHaveLength(1);
      expect(messages[0].value).toMatchObject({
        mediaSourceId: "first",
        lastMediaCommandNumber: 7,
      });
      onStartPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toHaveLength(1);

      onStopPlaybackObservationMessage(message("first"), metadata);
      expect(metadata.playbackObserver).toBe(observer);
      observer.updateLastMediaCommandNumber(8);
      element.dispatchEvent(new Event("playing"));
      expect(messages).toHaveLength(1);
      onStartPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toHaveLength(2);
      expect(messages[1].value.lastMediaCommandNumber).toBe(8);
      onRequestPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toHaveLength(3);

      const previousObserver = observer;
      attach("second");
      observer = metadata.playbackObserver;
      expect(observer).not.toBe(previousObserver);
      previousObserver.requestObservation();
      onStartPlaybackObservationMessage(message("first"), metadata);
      expect(messages).toHaveLength(3);
      observer.updateLastMediaCommandNumber(9);
      onStartPlaybackObservationMessage(message("second"), metadata);
      expect(messages).toHaveLength(4);
      expect(messages[3].value).toMatchObject({
        mediaSourceId: "second",
        lastMediaCommandNumber: 9,
      });
      onStopPlaybackObservationMessage(message("first"), metadata);
      element.dispatchEvent(new Event("playing"));
      expect(messages).toHaveLength(5);

      onClearMediaSourceMessage(message("second"), metadata, element);
      expect(metadata.playbackObserver).toBe(null);
      observer.requestObservation();
      expect(messages).toHaveLength(5);
    } finally {
      metadata.disposeMediaSource?.();
    }
  },
);

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

it("tags each snapshot at capture time and keeps native changes independent", () => {
  const { element, samples } = createObserver();
  observer.requestObservation();
  observer.updateLastMediaCommandNumber(1);
  element.currentTime = 4;
  observer.requestObservation();
  // A user seek or native adjustment does not need a worker command.
  element.currentTime = 5;
  observer.requestObservation();
  expect(
    samples.map((sample) => [
      sample.lastMediaCommandNumber,
      sample.currentTime,
    ]),
  ).toEqual([
    [0, 0],
    [1, 4],
    [1, 5],
  ]);
  observer.updateLastMediaCommandNumber(2);
  expect(samples[2].lastMediaCommandNumber).toBe(1);
});

it.each(mseLocations)(
  "carries media command numbers into snapshots (MSE in worker: %s)",
  { timeout: 20_000 },
  async (mseInWorker) => {
    const postMessage = Worker.prototype.postMessage;
    const samples = [];
    let worker;
    let ctx;
    Worker.prototype.postMessage = function (message, ...args) {
      if (message.type === "init") {
        worker = this;
        message.value.hasMseInWorker = mseInWorker;
      }
      if (message.type === "obs") samples.push(message.value);
      return postMessage.call(this, message, ...args);
    };
    try {
      ctx = await createPlayerHarness({ playerConfig: { bufferGoal: 6 } });
      const loaded = waitForPlayerEvent(
        ctx.player,
        "playerStateChange",
        (state) => state === "Loaded",
        5_000,
      );
      ctx.player.load(getVodScenarioUrl("mpegts-direct-media"));
      await loaded;
      const mediaSourceId = samples.at(-1).mediaSourceId;
      let mediaCommandNumber = samples.at(-1).lastMediaCommandNumber + 10;
      const deliver = (type, value, commandNumber = ++mediaCommandNumber) =>
        worker.onmessage({
          data: { type, value, mediaCommandNumber: commandNumber },
        });
      const sample = () => {
        worker.onmessage({
          data: { type: "request-obs", value: { mediaSourceId } },
        });
        return samples.at(-1);
      };
      const expectSample = (commandNumber, position) => {
        const observation = sample();
        expect(observation.lastMediaCommandNumber).toBe(commandNumber);
        expect(observation.currentTime).toBeCloseTo(position, 3);
      };
      expect(sample().lastMediaCommandNumber).toBeGreaterThan(0);
      const position = ctx.videoElement.buffered.start(0) + 0.05;
      deliver("seek", { mediaSourceId, position });
      expectSample(mediaCommandNumber, position);
      // Repeating the target is a new command, even after currentTime has moved.
      ctx.videoElement.currentTime = position + 0.02;
      deliver("seek", { mediaSourceId, position });
      expectSample(mediaCommandNumber, position);
      const currentCommandNumber = mediaCommandNumber;
      deliver("seek", { mediaSourceId: "obsolete", position: 10 });
      expectSample(currentCommandNumber, position);
      // Other main-thread commands advance the same counter.
      deliver("upd-pbr", { mediaSourceId, playbackRate: 0 });
      expect(sample().lastMediaCommandNumber).toBe(mediaCommandNumber);
      ctx.videoElement.currentTime = position + 0.03;
      expectSample(mediaCommandNumber, position + 0.03);
      expect(ctx.getLastPlayerError()).toBe(null);
    } finally {
      ctx?.dispose();
      Worker.prototype.postMessage = postMessage;
    }
  },
);
