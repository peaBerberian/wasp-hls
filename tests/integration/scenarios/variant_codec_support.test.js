import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { TEST_TIMEOUT_MS } from "../../utils/timeouts.js";
import {
  ContentCompatibilityErrorCode,
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

    const eventsBeforeProbe = ctx.selectionEvents.length;
    expect(() => ctx.finish("high-video-0.ts")).not.toThrow();
    expect(
      ctx.selectionEvents.slice(eventsBeforeProbe, eventsBeforeProbe + 2),
    ).toEqual([
      { type: "lock", id: undefined },
      { type: "variant", id: ctx.variants[0] },
    ]);
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
      const eventsBeforeProbe = ctx.selectionEvents.length;
      ctx.finish("high-audio-0.ts");
      expect(
        ctx.selectionEvents.slice(eventsBeforeProbe, eventsBeforeProbe + 2),
      ).toEqual([
        { type: "lock", id: undefined },
        { type: "variant", id: ctx.variants[0] },
      ]);
      expect(ctx.supportChecks.length - checksBeforeProbe).toBe(1);
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

it("keeps probe rejections when another variant is inspected", async () => {
  const ctx = await createDispatcher({ unsupportedCodec: "hev1.1.6.L153.B0" });
  try {
    ctx.dispatcher.load_content("https://example.com/master.m3u8");
    ctx.finish(
      "master.m3u8",
      "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\nlow-video.m3u8\n" +
        "#EXT-X-STREAM-INF:BANDWIDTH=2000000\nhigh-video.m3u8\n",
    );
    ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
    ctx.finish("low-video-0.ts");
    ctx.finish("high-video.m3u8", ctx.playlist("high-video"));
    ctx.finish("high-video-0.ts");
    expect(ctx.announcedVariants).toEqual(ctx.variants);
    expect(ctx.supportChecks).toHaveLength(1);
    expect(ctx.errors).toContainEqual({
      name: "sendContentCompatibilityError",
      args: [
        true,
        0,
        "No supported variant was found in the MultivariantPlaylist",
      ],
    });
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

it(
  "passes both muxed direct-media codecs to the support check and one SourceBuffer",
  { timeout: TEST_TIMEOUT_MS },
  async () => {
    const codecs = "avc1.42E01E,mp4a.40.2";
    const ctx = await createDispatcher({ probedCodecs: codecs });
    try {
      ctx.dispatcher.load_content("https://example.com/direct.m3u8");
      ctx.finish(
        "direct.m3u8",
        '#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI="init.mp4"\n#EXTINF:4,\nsegment.m4s\n#EXT-X-ENDLIST\n',
      );
      ctx.finish("init.mp4");
      ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);

      expect(ctx.supportChecks).toContainEqual({
        type: MediaType.Video,
        codec: codecs,
      });
      expect(ctx.sourceBuffers).toEqual([
        {
          mediaType: MediaType.Video,
          mimeType: `video/mp4;codecs="${codecs}"`,
        },
      ]);
      expect(ctx.errors).toEqual([]);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);

it.each([
  {
    rejectedType: "audio",
    codecs: "avc1.42E01E,ec-3",
    unsupportedCodec: "ec-3",
  },
  {
    rejectedType: "video",
    codecs: "hev1.1.6.L153.B0,mp4a.40.2",
    unsupportedCodec: "hev1.1.6.L153.B0",
  },
])(
  "rejects muxed direct media when only its $rejectedType codec is unsupported",
  { timeout: TEST_TIMEOUT_MS },
  async ({ codecs, unsupportedCodec }) => {
    const ctx = await createDispatcher({
      probedCodecs: codecs,
      unsupportedCodec,
    });
    try {
      ctx.dispatcher.load_content("https://example.com/direct.m3u8");
      ctx.finish(
        "direct.m3u8",
        '#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI="init.mp4"\n#EXTINF:4,\nsegment.m4s\n#EXT-X-ENDLIST\n',
      );
      ctx.finish("init.mp4");
      ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);

      expect(ctx.supportChecks).toContainEqual({
        type: MediaType.Video,
        codec: codecs,
      });
      expect(ctx.sourceBuffers).toEqual([]);
      expect(ctx.errors).toEqual([
        {
          name: "sendContentCompatibilityError",
          args: [
            true,
            ContentCompatibilityErrorCode.NoSupportedVariant,
            "No supported startup stream was found for the current content",
          ],
        },
      ]);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);

it("continues startup after falling back to a cached VOD playlist", async () => {
  const ctx = await createDispatcher({ unsupportedCodec: "hev1.1.6.L153.B0" });
  try {
    ctx.dispatcher.load_content("https://example.com/master.m3u8");
    ctx.finish(
      "master.m3u8",
      '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\nlow-video.m3u8\n' +
        "#EXT-X-STREAM-INF:BANDWIDTH=2000000\nhigh-video.m3u8\n",
    );
    ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
    ctx.dispatcher.lock_variant(ctx.variants[1]);
    ctx.finish("high-video.m3u8", ctx.playlist("high-video"));
    ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
    ctx.finish("high-video-0.ts");
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[0]);
    expect(
      ctx.requests.some(
        (request) =>
          !request.finished && request.url.endsWith("low-video-0.ts"),
      ),
    ).toBe(true);
    expect(ctx.errors).toEqual([]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});
