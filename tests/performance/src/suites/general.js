/**
 * Measure startup readiness, first displayed frames, reloads and seeks for
 * fMP4, MPEG-TS and alternate-audio content. Seek timings end at a displayed
 * frame near the target, for both buffered and unbuffered positions.
 */
import WaspHlsPlayer from "wasp-hls";
import EmbeddedWasm from "wasp-hls/wasm";
import EmbeddedWorker from "wasp-hls/worker";
import { ensureVodScenarioReady } from "../../../utils/vod_scenarios.js";
import { declareTestGroup, testEnd, testStart } from "../lib.js";
import {
  isBuffered,
  waitForFrame,
  waitForLoaded,
  waitUntil,
} from "../playback.js";

for (const [format, scenario] of [
  ["fMP4", "fmp4-player-api"],
  ["MPEG-TS", "mpegts-direct-media"],
  ["fMP4 alternate audio", "fmp4-multivariant-alt-audio"],
]) {
  declareTestGroup(
    `${format} startup and seeking`,
    async () => {
      const contentUrl = await ensureVodScenarioReady(scenario);
      const video = document.querySelector("video");
      video.muted = true;
      // Keep the final seek outside the initially buffered range.
      const player = new WaspHlsPlayer(video, { bufferGoal: 6 });
      try {
        testStart(`${format} cold loading to canplay`);
        testStart(`${format} cold loading to first frame`);
        await player.initialize({
          workerUrl: EmbeddedWorker,
          wasmUrl: EmbeddedWasm,
        });
        player.load(contentUrl);
        await waitForLoaded(player);
        testEnd(`${format} cold loading to canplay`);
        let frame = waitForFrame(video);
        await player.resume();
        await frame;
        testEnd(`${format} cold loading to first frame`);

        testStart(`${format} reload playing content to first frame`);
        player.load(contentUrl);
        await waitForLoaded(player);
        frame = waitForFrame(video);
        await player.resume();
        await frame;
        testEnd(`${format} reload playing content to first frame`);

        await waitUntil(
          () => isBuffered(video, video.currentTime + 0.5),
          "buffered seek target",
        );
        const offset = player.getMediaOffset() ?? 0;
        const bufferedTarget = video.currentTime - offset + 0.5;
        await measureSeek(
          player,
          video,
          bufferedTarget,
          `${format} buffered seek to first frame`,
        );

        const unbufferedTarget = video.duration - offset - 2;
        if (
          !Number.isFinite(unbufferedTarget) ||
          isBuffered(video, unbufferedTarget + offset)
        ) {
          throw new Error(
            "Seek benchmark target must be outside buffered ranges",
          );
        }
        await measureSeek(
          player,
          video,
          unbufferedTarget,
          `${format} unbuffered seek to first frame`,
        );
      } finally {
        player.dispose();
        video.removeAttribute("src");
      }
    },
    60_000,
  );
}

async function measureSeek(player, video, target, name) {
  const mediaTarget = target + (player.getMediaOffset() ?? 0);
  testStart(name);
  const frame = waitForFrame(
    video,
    (metadata) =>
      !video.seeking && Math.abs(metadata.mediaTime - mediaTarget) < 0.5,
  );
  player.seek(target);
  await frame;
  testEnd(name);
}
