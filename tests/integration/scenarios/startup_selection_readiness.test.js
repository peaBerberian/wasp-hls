import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { MediaSourceReadyState } from "../../../build/es6/wasm/js/index.js";

it("rechecks playlist readiness after a selection change while awaiting MediaSource", async () => {
  const ctx = await createDispatcher();
  try {
    ctx.dispatcher.load_content("https://example.com/master.m3u8");
    ctx.finish(
      "master.m3u8",
      '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\nlow-video.m3u8\n' +
        '#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS="avc1.42E01E"\nhigh-video.m3u8\n',
    );
    ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
    expect(ctx.variants).toHaveLength(2);

    ctx.dispatcher.lock_variant(ctx.variants[1]);
    ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
    expect(
      ctx.requests.filter((request) => request.url.endsWith(".ts")),
    ).toEqual([]);

    ctx.finish("high-video.m3u8", ctx.playlist("high-video"));
    expect(
      ctx.requests.some((request) => request.url.endsWith("high-video-0.ts")),
    ).toBe(true);
    expect(
      ctx.requests.some((request) => request.url.endsWith("low-video-0.ts")),
    ).toBe(false);
    expect(ctx.errors).toEqual([]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});
