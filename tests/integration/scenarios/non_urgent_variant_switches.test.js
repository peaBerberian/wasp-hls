import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { MediaSourceReadyState } from "../../../build/es6/wasm/js/index.js";

// A retained download must remain interruptible after the other media type has
// already moved the global variant down.
it.each(
  ["audio", "video"].flatMap((retainedType) =>
    [false, true].flatMap((withInit) =>
      [false, true].map((lockBeforeDrain) => ({
        retainedType,
        withInit,
        lockBeforeDrain,
      })),
    ),
  ),
)(
  "reconsiders retained $retainedType download (init=$withInit, locked=$lockBeforeDrain)",
  async ({ retainedType, withInit, lockBeforeDrain }) => {
    const ctx = await createDispatcher();
    try {
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
          let playlist = ctx.playlist(`${quality}-${type}`);
          if (withInit) {
            playlist = playlist.replace(
              "#EXTM3U\n",
              `#EXTM3U\n#EXT-X-MAP:URI="${quality}-${type}-init.mp4"\n`,
            );
          }
          ctx.finish(`${quality}-${type}.m3u8`, playlist);
        }
      }
      ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
      // Keep one high-quality download pending while the other type completes.
      ctx.observe(8.1);
      const retainedSuffix = withInit ? "init.mp4" : "0.ts";
      const retained = ctx.requests.find((request) =>
        request.url.endsWith(`high-${retainedType}-${retainedSuffix}`),
      );
      expect(retained).toBeDefined();
      const otherType = retainedType === "audio" ? "video" : "audio";
      if (withInit) {
        ctx.finish(`high-${otherType}-init.mp4`);
      }
      ctx.dispatcher.unlock_variant();
      ctx.finish(`high-${otherType}-0.ts`);
      expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
      expect(ctx.aborted).not.toContain(retained.id);
      ctx.observe(8.1);
      expect(ctx.aborted).not.toContain(retained.id);

      if (lockBeforeDrain) {
        ctx.dispatcher.lock_variant(ctx.variants[0]);
        // Explicit locks already abort downloads and flush buffers immediately.
        expect(ctx.aborted).toContain(retained.id);
      }
      // Keep the pending request inside the playback window: cancellation must
      // come from adaptive urgency, rather than a seek or obsolete time range.
      ctx.observe(1);
      expect(ctx.aborted).toContain(retained.id);
      const replacement = ctx.requests.find((request) =>
        request.url.endsWith(`low-${retainedType}-${retainedSuffix}`),
      );
      expect(replacement).toBeDefined();
      expect(ctx.aborted).not.toContain(replacement.id);
      ctx.observe(1);
      expect(ctx.aborted).not.toContain(replacement.id);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);
