import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { MediaSourceReadyState } from "../../../build/es6/wasm/js/index.js";

it("excludes an unprobed variant whose buffer layout is unknown", async () => {
  const ctx = await createDispatcher({
    unsupportedCodec: "hev1.1.6.L153.B0",
  });
  ctx.dispatcher.load_content("https://example.com/master.m3u8");
  ctx.finish(
    "master.m3u8",
    '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\nlow-video.m3u8\n' +
      "#EXT-X-STREAM-INF:BANDWIDTH=2000000\nhigh-video.m3u8\n",
  );
  ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  ctx.observe(8.1);
  expect(ctx.variants).toHaveLength(1);
  ctx.dispatcher.lock_variant(ctx.variants[0] + 1);
  expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
  expect(ctx.variantLockUpdates).toEqual([]);
  expect(ctx.errors).toHaveLength(1);
  expect(ctx.errors[0].name).toBe("sendOtherError");
  expect(ctx.errors[0].args[0]).toBe(false);
  ctx.dispatcher.stop();
  ctx.dispatcher.free();
});
