import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { TEST_TIMEOUT_MS } from "../../utils/timeouts.js";
import {
  MediaSourceReadyState,
  MediaType,
} from "../../../build/es6/wasm/js/index.js";

it(
  "handles an unsupported codec discovered after locking an unprobed variant",
  { timeout: TEST_TIMEOUT_MS },
  async () => {
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
    expect(ctx.variantLockUpdates.at(-1)).toBe(ctx.variants[1]);
    ctx.finish("high-video.m3u8", ctx.playlist("high-video"));

    expect(() => ctx.finish("high-video-0.ts")).not.toThrow();
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
    expect(ctx.variantLockUpdates).toEqual([ctx.variants[1], undefined]);
    expect(ctx.errors).toEqual([]);
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  },
);

it(
  "rechecks a cached variant when its missing audio codec is probed after locking",
  { timeout: TEST_TIMEOUT_MS },
  async () => {
    const ctx = await createDispatcher({ unsupportedCodec: "ec-3" });
    try {
      ctx.dispatcher.load_content("https://example.com/master.m3u8");
      ctx.finish(
        "master.m3u8",
        '#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="low-audio",NAME="Main",DEFAULT=YES,URI="low-audio.m3u8"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="high-audio",NAME="Main",DEFAULT=YES,URI="high-audio.m3u8"\n' +
          '#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="low-audio"\nlow-video.m3u8\n' +
          '#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS="avc1.42E01E",AUDIO="high-audio"\nhigh-video.m3u8\n',
      );
      ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
      ctx.finish("low-audio.m3u8", ctx.playlist("low-audio"));
      ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
      ctx.observe(8.1);
      ctx.dispatcher.lock_variant(ctx.variants[1]);
      ctx.finish("high-video.m3u8", ctx.playlist("high-video"));
      ctx.finish("high-audio.m3u8", ctx.playlist("high-audio"));
      const checksBeforeProbe = ctx.supportChecks.length;
      ctx.finish("high-audio-0.ts");
      expect(ctx.supportChecks.length - checksBeforeProbe).toBe(4);
      expect(ctx.supportChecks.slice(checksBeforeProbe)).toContainEqual({
        type: MediaType.Audio,
        codec: "ec-3",
      });
      expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
      expect(ctx.variantLockUpdates).toEqual([ctx.variants[1], undefined]);
      expect(ctx.errors).toEqual([]);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);
