/**
 * Measure playlist processing from response resolution to the next resource
 * request, covering large multivariant/media playlists and sliding live
 * windows whose segment durations match their media.
 */
import WaspHlsPlayer from "wasp-hls";
import EmbeddedWasm from "wasp-hls/wasm";
import { ensureVodScenarioReady } from "../../../utils/vod_scenarios.js";
import { declareTestGroup, reportValue } from "../lib.js";
import { createPerformanceWorker } from "../test_worker.js";
import {
  parseMediaPlaylist,
  waitForFrame,
  waitForLoaded,
} from "../playback.js";

declareTestGroup(
  "playlist processing",
  async () => {
    const masterUrl = "https://performance.invalid/master.m3u8";
    const master = createMultivariantPlaylist(100);
    const media = createMediaPlaylist(2_048);
    const worker = createPerformanceWorker([
      {
        id: "master",
        match: { urlEndsWith: "/master.m3u8" },
        actions: [{ type: "response", body: master, status: 200 }],
      },
      {
        id: "media",
        match: { urlEndsWith: "/variant-0.m3u8" },
        actions: [{ type: "response", body: media, status: 200 }],
      },
      {
        id: "segment",
        match: { urlEndsWith: ".ts" },
        actions: [{ type: "response", body: "", status: 200 }],
      },
    ]);
    const player = new WaspHlsPlayer(document.querySelector("video"));
    await player.initialize({ workerUrl: worker.url, wasmUrl: EmbeddedWasm });
    player.load(masterUrl);

    const masterResolved = await worker.waitFor(isResolved("master"));
    const mediaStarted = await worker.waitFor(isStarted("media"));
    reportValue(
      "100-variant multivariant playlist processing",
      mediaStarted.timestampMs - masterResolved.timestampMs,
    );

    const mediaResolved = await worker.waitFor(isResolved("media"));
    const segmentStarted = await worker.waitFor(isStarted("segment"));
    reportValue(
      "2048-segment media playlist processing",
      segmentStarted.timestampMs - mediaResolved.timestampMs,
    );

    player.dispose();
    worker.dispose();
  },
  20_000,
);

declareTestGroup(
  "sliding live playlist",
  async () => {
    const liveUrl = "https://performance.invalid/live.m3u8";
    const vodUrl = await ensureVodScenarioReady("fmp4-player-api");
    const media = parseMediaPlaylist(
      await (await fetch(vodUrl)).text(),
      vodUrl,
    );
    const worker = createPerformanceWorker([
      {
        id: "live",
        match: { urlEndsWith: "/live.m3u8" },
        actions: Array.from({ length: 4 }, (_, update) => ({
          type: "response",
          body: createLivePlaylist(100 + update, media, update),
        })),
      },
      ...media.segments.map((segment, index) => ({
        id: `segment-${index}`,
        match: { urlEndsWith: new URL(segment.url).pathname },
        actions: [{ type: "passthrough" }],
      })),
    ]);
    const video = document.querySelector("video");
    video.muted = true;
    const player = new WaspHlsPlayer(video);
    try {
      await player.initialize({ workerUrl: worker.url, wasmUrl: EmbeddedWasm });
      player.load(liveUrl);
      await waitForLoaded(player);
      const frame = waitForFrame(video);
      await player.resume();
      await frame;
      for (let update = 1; update <= 3; update++) {
        const responses = await worker.waitForCount(
          isResolved("live"),
          update + 1,
          15_000,
        );
        const requested = await worker.waitFor(
          isStarted(`segment-${update + 2}`),
        );
        reportValue(
          "live media playlist refresh processing",
          requested.timestampMs - responses[update].timestampMs,
        );
      }
      if (player.getPlayerState() === "Error")
        throw new Error("Live playback failed");
    } finally {
      player.dispose();
      worker.dispose();
      video.removeAttribute("src");
    }
  },
  30_000,
  { runEvery: 2 },
);

function isResolved(ruleId) {
  return (event) => event.type === "fetch-resolve" && event.ruleId === ruleId;
}

function isStarted(ruleId) {
  return (event) => event.type === "fetch-start" && event.ruleId === ruleId;
}

function createMultivariantPlaylist(variantCount) {
  let playlist = "#EXTM3U\n#EXT-X-VERSION:6\n";
  for (let index = 0; index < variantCount; index++) {
    playlist += `#EXT-X-STREAM-INF:BANDWIDTH=${100_000 + index * 10_000},CODECS="avc1.4d401f"\nhttps://performance.invalid/variant-${index}.m3u8\n`;
  }
  return playlist;
}

function createMediaPlaylist(segmentCount) {
  let playlist =
    "#EXTM3U\n#EXT-X-VERSION:6\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:1000\n";
  for (let index = 0; index < segmentCount; index++) {
    playlist += `#EXTINF:4,\nhttps://performance.invalid/segment-${index}.ts\n`;
  }
  return playlist + "#EXT-X-ENDLIST\n";
}

function createLivePlaylist(sequence, media, firstSegment) {
  return (
    "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-INDEPENDENT-SEGMENTS\n" +
    `#EXT-X-TARGETDURATION:${media.targetDuration}\n` +
    `#EXT-X-MAP:URI="${media.initUrl}"\n` +
    `#EXT-X-MEDIA-SEQUENCE:${sequence}\n` +
    media.segments
      .slice(firstSegment, firstSegment + 3)
      .map((segment) => `#EXTINF:${segment.duration},\n${segment.url}\n`)
      .join("")
  );
}
