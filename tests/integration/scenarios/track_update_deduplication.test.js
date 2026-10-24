import { describe, expect, it } from "vitest";
import EmbeddedWorker from "../../../build/embedded/worker.js";
import { onTrackUpdateMessage } from "../../../build/es6/ts-main/worker-message-handlers.js";
import { MediaType } from "../../../build/es6/wasm/js/enums.js";
import { waitForLoadedState } from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import setupPlayer from "../utils/player_setup";

it.each([MediaType.Audio, MediaType.Text])(
  "filters repeated track IDs, preserving initial and cleared selections (type: %s)",
  (mediaType) => {
    const metadata = {
      contentId: "current",
      currentAudioTrack: undefined,
      currentSubtitleTrack: undefined,
    };
    function update(id, contentId = "current") {
      return onTrackUpdateMessage(
        {
          type: "track-upd",
          value: {
            contentId,
            mediaType,
            audioTrack: id === undefined ? undefined : { current: id },
            subtitleTrack: id === undefined ? undefined : { current: id },
          },
        },
        metadata,
      );
    }

    expect(update(undefined, "old-content")).toBe(false);
    expect(update(undefined)).toBe(true);
    expect(update(undefined)).toBe(false);
    expect(update(0)).toBe(true);
    expect(update(0)).toBe(false);
    // Distinct IDs still count as changes when track information is unavailable.
    expect(update(1)).toBe(true);
    expect(update(undefined)).toBe(true);
    expect(update(undefined)).toBe(false);
  },
);

describe("Track events with duplicate worker messages", () => {
  const ctx = setupPlayer({
    createWorker() {
      const source = `
        const send = self.postMessage.bind(self);
        self.postMessage = (message, ...args) => {
          send(message, ...args);
          if (message.type === "track-upd") {
            send(message);
          }
        };
        importScripts(${JSON.stringify(EmbeddedWorker)});
      `;
      const url = URL.createObjectURL(
        new Blob([source], { type: "application/javascript" }),
      );
      return { url, dispose: () => URL.revokeObjectURL(url) };
    },
  });

  it("announces absent tracks once per load", { timeout: 60_000 }, async () => {
    const audio = [];
    const subtitles = [];
    ctx.player.addEventListener("audioTrackUpdate", (track) => {
      expect(ctx.player.getCurrentAudioTrack()).toBeUndefined();
      audio.push(track);
    });
    ctx.player.addEventListener("subtitleTrackUpdate", (track) => {
      expect(ctx.player.getCurrentSubtitleTrack()).toBeNull();
      subtitles.push(track);
    });

    for (let load = 1; load <= 2; load++) {
      ctx.player.load(getVodScenarioUrl("fmp4-direct-media"));
      await waitForLoadedState(
        ctx.player,
        ctx.videoElement,
        () => ctx.lastPlayerError,
      );
      expect(audio).toEqual(Array(load).fill(undefined));
      expect(subtitles).toEqual(Array(load).fill(null));
    }
  });
});
