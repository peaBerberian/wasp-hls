import { expect, it } from "vitest";
import { inspectSegment } from "../../../src/ts-worker/bindings.ts";
import { jsMemoryResources } from "../../../src/ts-worker/globals.ts";
import { MediaType } from "../../../src/wasm/index.js";
import { ensureVodScenarioReady } from "../../utils/vod_scenarios.js";

it("probes binary MPEG-TS without decoding its header as UTF-8", async () => {
  const playlistUrl = await ensureVodScenarioReady("mpegts-direct-media");
  const playlist = await (await fetch(playlistUrl)).text();
  const segmentUrl = playlist
    .split("\n")
    .find((line) => line.trim() !== "" && !line.startsWith("#"));
  const response = await fetch(new URL(segmentUrl, playlistUrl));
  expect(response.ok).toBe(true);
  const segment = new Uint8Array(await response.arrayBuffer());
  expect(() =>
    new TextDecoder("utf-8", { fatal: true }).decode(segment.subarray(0, 7)),
  ).toThrow();
  const id = jsMemoryResources.create(segment);
  try {
    expect(inspectSegment(id)).toMatchObject({
      value: { mediaType: MediaType.Video, mimeType: "video/mp2t" },
      errorCode: undefined,
    });
  } finally {
    jsMemoryResources.delete(id);
  }
});

it.each(["", "\ufeff"])("probes WebVTT (BOM: %j)", (bom) => {
  const id = jsMemoryResources.create(
    new TextEncoder().encode(`${bom}WEBVTT\n\n`),
  );
  try {
    expect(inspectSegment(id)).toMatchObject({
      value: { mediaType: MediaType.Text, mimeType: "text/vtt", codec: "wvtt" },
      errorCode: undefined,
    });
  } finally {
    jsMemoryResources.delete(id);
  }
});
