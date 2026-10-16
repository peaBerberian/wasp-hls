import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { MediaSourceReadyState } from "../../../build/es6/wasm/js/index.js";

// XXX TODO: A non-urgent downgrade can retain a high-bitrate video download
// while audio moves to the low variant. If the buffer then becomes small, that
// old video download must still be eligible for urgent cancellation. Comparing
// only with the globally chosen variant loses the downgrade in this situation.
it("can make a retained high-bitrate request urgent after audio switches down", async () => {
  const ctx = await createDispatcher();
  ctx.dispatcher.load_content("https://example.com/master.m3u8");
  ctx.finish(
    "master.m3u8",
    '#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="low",NAME="English",DEFAULT=YES,URI="low-audio.m3u8"\n' +
      '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="high",NAME="English",DEFAULT=YES,URI="high-audio.m3u8"\n' +
      '#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="low"\nlow-video.m3u8\n' +
      '#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="high"\nhigh-video.m3u8\n',
  );
  ctx.dispatcher.lock_variant(ctx.variants[1]);
  for (const quality of ["low", "high"]) {
    for (const type of ["audio", "video"]) {
      ctx.finish(`${quality}-${type}.m3u8`, ctx.playlist(`${quality}-${type}`));
    }
  }
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  // Simulate existing buffered media; keep the high video download pending.
  ctx.observe(8.1);
  const highVideo = ctx.requests.find((request) =>
    request.url.endsWith("high-video-0.ts"),
  );
  expect(highVideo).toBeDefined();
  ctx.dispatcher.unlock_variant();
  ctx.finish("high-audio-0.ts");
  expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
  expect(ctx.aborted).not.toContain(highVideo.id);

  // Report little buffered time without seeking or moving the pending request
  // outside the playback window (which would cancel it for another reason).
  ctx.observe(1);
  expect(ctx.aborted).toContain(highVideo.id);
  ctx.dispatcher.stop();
  ctx.dispatcher.free();
});
