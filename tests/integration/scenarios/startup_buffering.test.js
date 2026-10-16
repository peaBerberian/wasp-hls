import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import {
  JsTimeRanges,
  MediaSourceReadyState,
} from "../../../build/es6/wasm/js/index.js";

// XXX TODO: A one-second buffer goal is satisfied by a two-second segment, so no
// further download is scheduled. Startup must then be able to finish instead
// of waiting forever for more than the fixed two-second rebuffering threshold.
it("finishes startup when a small buffer goal has been satisfied", async () => {
  const ctx = await createDispatcher({ segmentDuration: 2 });
  ctx.dispatcher.set_buffer_goal(1);
  ctx.dispatcher.load_content("https://example.com/master.m3u8");
  ctx.finish(
    "master.m3u8",
    '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\nlow-video.m3u8\n',
  );
  ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  ctx.finish("low-video-0.ts");
  ctx.dispatcher.on_source_buffer_update(
    1,
    new JsTimeRanges(new Float64Array([0, 2])),
  );
  ctx.observe(2);
  ctx.observe(2);

  expect(ctx.rebufferingEvents).toEqual(["start", "stop"]);
  ctx.dispatcher.stop();
  ctx.dispatcher.free();
});
