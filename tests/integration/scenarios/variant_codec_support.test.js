import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import { TEST_TIMEOUT_MS } from "../../utils/timeouts.js";
import {
  ContentCompatibilityErrorCode,
  MediaSourceReadyState,
  MediaType,
  TrackSelectionErrorCode,
} from "../../../build/es6/wasm/js/index.js";

async function createAudioSwitchDispatcher(frenchAudioCodec = "") {
  const ctx = await createDispatcher({ unsupportedCodec: "ec-3" });
  ctx.dispatcher.load_content("https://example.com/master.m3u8");
  ctx.finish(
    "master.m3u8",
    `#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en-audio.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="fr-audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="en"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4000000000,CODECS="avc1.42E01E${frenchAudioCodec ? `,${frenchAudioCodec}` : ""}",AUDIO="fr"
fr-video.m3u8
`,
  );
  ctx.finish("en-video.m3u8", ctx.playlist("en-video"));
  ctx.finish("en-audio.m3u8", ctx.playlist("en-audio"));
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  expect(ctx.audioTrackIds).toHaveLength(2);
  expect(ctx.errors).toEqual([]);
  return ctx;
}

it("warns and keeps playing the previous audio when the requested track is known unsupported", async () => {
  const ctx = await createAudioSwitchDispatcher("ec-3");
  try {
    ctx.dispatcher.lock_variant(ctx.variants[0]);
    const events = ctx.selectionEvents.slice();
    const requests = ctx.requests.length;
    const aborted = ctx.aborted.slice();
    ctx.dispatcher.set_audio_track(ctx.audioTrackIds[1]);
    expect(ctx.errors).toEqual([
      {
        name: "sendTrackSelectionError",
        args: [
          TrackSelectionErrorCode.NoCompatibleVariant,
          MediaType.Audio,
          ctx.audioTrackIds[1],
          "Unable to select the audio track: no compatible variant",
        ],
      },
    ]);
    expect(ctx.selectionEvents).toEqual(events);
    expect(ctx.requests).toHaveLength(requests);
    expect(ctx.aborted).toEqual(aborted);
    ctx.observe(8.1);
    expect(ctx.variantLockUpdates).toEqual([ctx.variants[0]]);
    expect(ctx.requests.some((request) => request.url.includes("/fr-"))).toBe(
      false,
    );
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

it("fails fatally when probing rules out all variants for the chosen audio", async () => {
  const ctx = await createAudioSwitchDispatcher();
  try {
    ctx.dispatcher.set_audio_track(ctx.audioTrackIds[1]);
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[1]);
    expect(ctx.selectionEvents).toContainEqual({
      type: "audio",
      id: ctx.audioTrackIds[1],
    });
    ctx.finish("fr-video.m3u8", ctx.playlist("fr-video"));
    ctx.finish("fr-audio.m3u8", ctx.playlist("fr-audio"));
    expect(ctx.errors).toEqual([]);
    ctx.finish("fr-audio-0.ts");
    expect(ctx.supportChecks).toContainEqual({
      type: MediaType.Audio,
      codec: "ec-3",
    });
    expect(ctx.errors).toEqual([
      {
        name: "sendContentCompatibilityError",
        args: [
          true,
          ContentCompatibilityErrorCode.NoSupportedVariant,
          "No supported variant was found in the MultivariantPlaylist",
        ],
      },
    ]);
    const requests = ctx.requests.length;
    ctx.observe(8.1);
    expect(ctx.requests).toHaveLength(requests);
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[1]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

async function createSubtitleSwitchDispatcher(frenchVideoCodec = "") {
  const ctx = await createDispatcher({
    unsupportedCodec: "hev1.1.6.L153.B0",
  });
  ctx.dispatcher.load_content("https://example.com/master.m3u8");
  ctx.finish(
    "master.m3u8",
    `#EXTM3U
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="en",NAME="English",LANGUAGE="en",URI="en-subtitles.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="fr",NAME="French",LANGUAGE="fr",URI="fr-subtitles.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E",SUBTITLES="en"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4000000000${frenchVideoCodec ? `,CODECS="${frenchVideoCodec}"` : ""},SUBTITLES="fr"
fr-video.m3u8
`,
  );
  ctx.finish("en-video.m3u8", ctx.playlist("en-video"));
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  expect(ctx.subtitleTrackIds).toHaveLength(2);
  ctx.dispatcher.set_subtitle_track(ctx.subtitleTrackIds[0]);
  expect(ctx.selectionEvents).toContainEqual({
    type: "subtitles",
    id: ctx.subtitleTrackIds[0],
  });
  expect(ctx.errors).toEqual([]);
  return ctx;
}

it("warns and keeps the previous subtitles when all variants providing the requested track are known unsupported", async () => {
  const ctx = await createSubtitleSwitchDispatcher("hev1.1.6.L153.B0");
  try {
    ctx.dispatcher.lock_variant(ctx.variants[0]);
    const events = ctx.selectionEvents.slice();
    const requests = ctx.requests.length;
    const aborted = ctx.aborted.slice();
    ctx.dispatcher.set_subtitle_track(ctx.subtitleTrackIds[1]);
    expect(ctx.errors).toEqual([
      {
        name: "sendTrackSelectionError",
        args: [
          TrackSelectionErrorCode.NoCompatibleVariant,
          MediaType.Text,
          ctx.subtitleTrackIds[1],
          "Unable to select the subtitle track: no compatible variant",
        ],
      },
    ]);
    expect(ctx.selectionEvents).toEqual(events);
    expect(ctx.requests).toHaveLength(requests);
    expect(ctx.aborted).toEqual(aborted);
    ctx.observe(8.1);
    expect(ctx.variantLockUpdates).toEqual([ctx.variants[0]]);
    expect(ctx.requests.some((request) => request.url.includes("/fr-"))).toBe(
      false,
    );
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

it("fails fatally when probing rules out all variants providing the chosen subtitles", async () => {
  const ctx = await createSubtitleSwitchDispatcher();
  try {
    ctx.dispatcher.set_subtitle_track(ctx.subtitleTrackIds[1]);
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[1]);
    expect(ctx.selectionEvents).toContainEqual({
      type: "subtitles",
      id: ctx.subtitleTrackIds[1],
    });
    ctx.finish("fr-video.m3u8", ctx.playlist("fr-video"));
    expect(ctx.errors).toEqual([]);
    // The variant's video codec is unsupported; subtitles themselves are not probed.
    ctx.finish("fr-video-0.ts");
    expect(ctx.supportChecks).toContainEqual({
      type: MediaType.Video,
      codec: "hev1.1.6.L153.B0",
    });
    expect(ctx.errors).toEqual([
      {
        name: "sendContentCompatibilityError",
        args: [
          true,
          ContentCompatibilityErrorCode.NoSupportedVariant,
          "No supported variant was found in the MultivariantPlaylist",
        ],
      },
    ]);
    const requests = ctx.requests.length;
    ctx.observe(8.1);
    expect(ctx.requests).toHaveLength(requests);
    expect(ctx.announcedVariants.at(-1)).toBe(ctx.variants[1]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

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
