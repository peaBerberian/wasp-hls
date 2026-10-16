import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { MediaSourceReadyState } from "../../../build/es6/wasm/js/index.js";

// XXX TODO: A variant without CODECS is offered before probing. Locking it and
// then probing an unsupported codec triggers support recovery. Recovery chooses
// that same locked variant again, recursively, until WASM traps. Recover or
// report an unsupported-content error instead of repeatedly selecting it.
it("handles an unsupported codec discovered after locking an unprobed variant", async () => {
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
  ctx.dispatcher.lock_variant(ctx.variants[1]);
  ctx.finish("high-video.m3u8", ctx.playlist("high-video"));

  expect(() => ctx.finish("high-video-0.ts")).not.toThrow();
  // Both a supported fallback and a graceful fatal error are acceptable here;
  // this regression test does not prescribe how an invalid lock is resolved.
  expect(
    ctx.announcedVariants.at(-1) === ctx.variants[0] ||
      ctx.errors.some((error) => error.args[0] === true),
  ).toBe(true);
  ctx.dispatcher.stop();
  ctx.dispatcher.free();
});
