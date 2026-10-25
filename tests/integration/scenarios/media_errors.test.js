import { describe, expect, it, vi } from "vitest";
import { WaspMediaError } from "../../../build/es6/ts-main/index.js";
import {
  waitForLoadedState,
  waitForPlayback,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { TEST_TIMEOUT_MS } from "../../utils/timeouts.js";
import { ensureVodScenarioReady } from "../../utils/vod_scenarios.js";
import { createPlayerHarness } from "../utils/request_integration_test_tools.js";

function injectMediaError(
  videoElement,
  code,
  message = "Injected media error",
  eventName = "error",
) {
  const error = vi.spyOn(videoElement, "error", "get").mockReturnValue({
    code,
    message,
  });
  try {
    videoElement.dispatchEvent(new Event(eventName));
  } finally {
    error.mockRestore();
  }
}

async function load(ctx, url) {
  ctx.player.load(url);
  await waitForLoadedState(
    ctx.player,
    ctx.videoElement,
    ctx.getLastPlayerError,
  );
}

describe("Native media errors", () => {
  it.each([
    [1, "MediaAbortedError"],
    [2, "MediaNetworkError"],
    [3, "MediaDecodeError"],
    [4, "MediaSourceNotSupportedError"],
    [99, "MediaOtherError"],
  ])(
    "reports native code %s as %s and stops the worker without losing the error",
    { timeout: TEST_TIMEOUT_MS },
    async (mediaErrorCode, code) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const ctx = await createPlayerHarness({});
      let messages;
      try {
        await load(ctx, url);
        if (mediaErrorCode === 3) {
          await waitForPlayback(
            ctx.player,
            ctx.videoElement,
            ctx.getLastPlayerError,
          );
        }
        const errors = [];
        const states = [];
        ctx.player.addEventListener("error", (error) => errors.push(error));
        ctx.player.addEventListener("playerStateChange", (state) =>
          states.push(state),
        );
        messages = vi.spyOn(Worker.prototype, "postMessage");
        injectMediaError(
          ctx.videoElement,
          mediaErrorCode,
          "Injected media error",
          mediaErrorCode === 1 ? "abort" : "error",
        );

        expect(errors).toHaveLength(1);
        const error = errors[0];
        expect(error).toBeInstanceOf(WaspMediaError);
        expect(error).toMatchObject({
          name: "WaspMediaError",
          code,
          globalCode: code,
          mediaErrorCode,
          message: "Injected media error",
        });
        expect(ctx.player.getError()).toBe(error);
        expect(ctx.player.getPlayerState()).toBe("Error");
        expect(ctx.videoElement.getAttribute("src")).toBeNull();
        expect(ctx.videoElement.srcObject).toBeNull();
        const stopIndex = messages.mock.calls.findIndex(
          ([message]) => message.type === "stop",
        );
        expect(stopIndex).toBeGreaterThanOrEqual(0);
        const contentId = messages.mock.calls[stopIndex][0].value.contentId;
        expect(contentId).not.toBe("");
        const stopped = waitForPlayerEvent(
          messages.mock.contexts[stopIndex],
          "message",
          (event) =>
            event.data.type === "ctnt-stop" &&
            event.data.value.contentId === contentId,
        );
        injectMediaError(ctx.videoElement, 3, "Later error");
        await stopped;
        expect(errors).toEqual([error]);
        expect(states).toEqual(["Error"]);
        expect(ctx.player.getError()).toBe(error);
        expect(ctx.player.getPlayerState()).toBe("Error");

        // The same player and worker can load and play again after a fatal error.
        await load(ctx, url);
        await waitForPlayback(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        expect(ctx.player.getError()).toBeNull();
      } finally {
        messages?.mockRestore();
        ctx.dispose();
      }
    },
  );

  it(
    "reports a real browser media error through the public API",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const ctx = await createPlayerHarness({});
      const invalidMediaUrl = URL.createObjectURL(
        new Blob(["Invalid MP4 data"], { type: "video/mp4" }),
      );
      try {
        await load(ctx, url);
        const failed = waitForPlayerEvent(ctx.player, "error");
        ctx.videoElement.src = invalidMediaUrl;
        const error = await failed;
        expect(error).toBeInstanceOf(WaspMediaError);
        expect([3, 4]).toContain(error.mediaErrorCode);
        expect(error.message.length).toBeGreaterThan(0);
        expect(ctx.player.getError()).toBe(error);
        expect(ctx.player.getPlayerState()).toBe("Error");
      } finally {
        URL.revokeObjectURL(invalidMediaUrl);
        ctx.dispose();
      }
    },
  );

  it(
    "fails during loading and supplies a message when the browser has none",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const ctx = await createPlayerHarness({});
      const statesAtError = [];
      const onLoadStart = () => {
        statesAtError.push(ctx.player.getPlayerState());
        injectMediaError(ctx.videoElement, 3, "");
      };
      ctx.videoElement.addEventListener("loadstart", onLoadStart, {
        once: true,
      });
      try {
        const failed = waitForPlayerEvent(ctx.player, "error");
        ctx.player.load(url);
        const error = await failed;
        expect(statesAtError).toEqual(["Loading"]);
        expect(error.code).toBe("MediaDecodeError");
        expect(error.message).toBe("The browser could not decode media data");
        expect(ctx.player.getPlayerState()).toBe("Error");
        await load(ctx, url);
        expect(ctx.player.getError()).toBeNull();
      } finally {
        ctx.videoElement.removeEventListener("loadstart", onLoadStart);
        ctx.dispose();
      }
    },
  );

  it.each(["stop", "load", "dispose"])(
    "allows %s from the error listener without emitting a stale Error state",
    { timeout: TEST_TIMEOUT_MS },
    async (action) => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const ctx = await createPlayerHarness({});
      try {
        await load(ctx, url);
        const errors = [];
        const states = [];
        ctx.player.addEventListener("error", (error) => {
          errors.push({
            error,
            state: ctx.player.getPlayerState(),
            stored: ctx.player.getError(),
          });
          if (action === "load") {
            ctx.player.load(url);
          } else {
            ctx.player[action]();
          }
        });
        ctx.player.addEventListener("playerStateChange", (state) =>
          states.push(state),
        );
        injectMediaError(ctx.videoElement, 3);
        expect(errors).toHaveLength(1);
        expect(errors[0].state).toBe("Error");
        expect(errors[0].stored).toBe(errors[0].error);
        expect(states).toEqual(
          action === "load"
            ? ["Loading"]
            : action === "stop"
              ? ["Stopped"]
              : [],
        );
        if (action === "load") {
          await waitForLoadedState(
            ctx.player,
            ctx.videoElement,
            ctx.getLastPlayerError,
          );
          expect(ctx.player.getError()).toBeNull();
        }
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "ignores events without a native error and errors without an attached content",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const url = await ensureVodScenarioReady("fmp4-alt-audio");
      const ctx = await createPlayerHarness({});
      const errors = [];
      ctx.player.addEventListener("error", (error) => errors.push(error));
      try {
        injectMediaError(ctx.videoElement, 3);
        ctx.player.load(url);
        injectMediaError(ctx.videoElement, 3);
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        ctx.videoElement.dispatchEvent(new Event("error"));
        ctx.videoElement.dispatchEvent(new Event("abort"));
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        ctx.player.stop();
        injectMediaError(ctx.videoElement, 3);
        ctx.player.dispose();
        injectMediaError(ctx.videoElement, 3);
        expect(errors).toEqual([]);
      } finally {
        ctx.dispose();
      }
    },
  );
});
