import { expect, it } from "vitest";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import initializeWasm, {
  Dispatcher,
  JsTimeRanges,
  MediaSourceReadyState,
  MediaObservation,
  PlaybackTickReason,
  MediaType,
  SegmentParsingErrorCode,
  PushedSegmentErrorCode,
} from "../../../build/es6/wasm/js/index.js";

// Keep host callbacks synchronous, like the real WASM bindings. Network and MSE
// completions are delivered explicitly after the dispatcher has returned.
async function createProbeHarness({
  audioInit = false,
  singleTrack = false,
  audioDuration = 4,
  mediaDuration = 12,
  observeBeforeReady = false,
  failAppend,
} = {}) {
  const requests = [];
  const resources = new Map();
  const resourceUrls = new Map();
  const buffers = new Map();
  const appends = [];
  const removals = [];
  const offsets = [];
  const errors = [];
  let ended = 0;
  let nextResource = 1;
  let nextBuffer = 1;
  const bindings = new Proxy(
    {
      fetch(url) {
        const id = requests.length + 1;
        requests.push({ id, url, finished: false, aborted: false });
        return id;
      },
      abortRequest(id) {
        requests.find((request) => request.id === id).aborted = true;
        return true;
      },
      getResourceData: (id) => resources.get(id),
      freeResource: (id) => resources.delete(id),
      isTypeSupported: () => true,
      inspectSegment(id) {
        const audio = resourceUrls.get(id).includes("-audio");
        return {
          value: {
            mediaType: audio ? MediaType.Audio : MediaType.Video,
            mimeType: audio ? "audio/mp4" : "video/mp4",
            codec: audio ? "mp4a.40.2" : "avc1.42E01E",
          },
        };
      },
      addSourceBuffer(mediaType) {
        const id = nextBuffer++;
        buffers.set(id, { mediaType, pending: [], end: 2 });
        return { value: id };
      },
      appendBuffer(bufferId, resourceId, hints) {
        const url = resourceUrls.get(resourceId);
        const buffer = buffers.get(bufferId);
        const fetches = requests
          .filter((request) => !request.aborted)
          .map((request) => request.url);
        const append = {
          url,
          resourceId,
          bufferId,
          hints: { ...hints },
          fetches,
        };
        appends.push(append);
        if (failAppend?.(append)) {
          return {
            errorCode: SegmentParsingErrorCode.TransmuxerError,
            description: "Injected preparation failure",
          };
        }
        buffer.pending.push(resourceId);
        const index = /-(\d+)\.ts$/.exec(url)?.[1];
        if (index === undefined) return { value: undefined };
        const duration =
          buffer.mediaType === MediaType.Audio ? audioDuration : 4;
        const start = Number(index) * duration + 2;
        buffer.end = start + duration;
        return {
          value: {
            start: { hi: 0, lo: start },
            end: { hi: 0, lo: start + duration },
            timescale: 1,
          },
        };
      },
      removeBuffer(bufferId, start, end) {
        removals.push({ bufferId, start, end });
        return { value: true };
      },
      setMediaOffset: (offset) => offsets.push(offset),
      endOfStream() {
        ended++;
        return { value: true };
      },
      monotonicNow: () => 1000,
      getRandom: () => 0.5,
      timer: () => 1,
    },
    {
      get(target, key) {
        if (key in target) return target[key];
        if (String(key).startsWith("send"))
          return (...args) => errors.push({ key, args });
        return () => ({ value: true });
      },
    },
  );
  await initializeWasm(
    await (await fetch(EmbeddedWasm)).arrayBuffer(),
    bindings,
  );
  const dispatcher = new Dispatcher(1_000_000);

  function finish(suffix, body = "media", { obsolete = false } = {}) {
    const request = requests.find(
      (request) =>
        !request.finished &&
        (obsolete || !request.aborted) &&
        request.url.endsWith(suffix),
    );
    expect(request, `pending request for ${suffix}`).toBeDefined();
    request.finished = true;
    const id = nextResource++;
    resources.set(id, new TextEncoder().encode(body));
    resourceUrls.set(id, request.url);
    dispatcher.on_request_finished(
      request.id,
      id,
      body.length,
      request.url,
      100,
    );
    return id;
  }

  function start(name = "first") {
    dispatcher.load_content(`https://example.com/${name}-master.m3u8`);
    finish(
      `${name}-master.m3u8`,
      singleTrack
        ? `#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000\n${name}-video.m3u8\n`
        : `#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",DEFAULT=YES,URI="${name}-audio.m3u8"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,AUDIO="audio"\n${name}-video.m3u8\n`,
    );
    for (const type of singleTrack ? ["video"] : ["video", "audio"]) {
      const duration = type === "audio" ? audioDuration : 4;
      const init =
        type === "audio" && audioInit
          ? `#EXT-X-MAP:URI="${name}-audio-init.mp4"\n`
          : "";
      const segments = Array.from(
        { length: mediaDuration / duration },
        (_, index) =>
          `${index * duration === 8 ? "#EXT-X-DISCONTINUITY\n" : ""}#EXTINF:${duration},\n${name}-${type}-${index}.ts\n`,
      ).join("");
      finish(
        `${name}-${type}.m3u8`,
        `#EXTM3U\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-TARGETDURATION:${duration}\n${init}${segments}#EXT-X-ENDLIST\n`,
      );
    }
    dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
    const videoProbe = finish(`${name}-video-0.ts`);
    if (singleTrack) return { videoProbe };
    expect(
      appends.filter((append) => append.resourceId === videoProbe),
    ).toEqual([]);
    if (observeBeforeReady) {
      dispatcher.on_playback_tick(
        new MediaObservation(
          PlaybackTickReason.RegularInterval,
          0,
          0,
          new JsTimeRanges([]),
          true,
          false,
          false,
          12,
          undefined,
          undefined,
        ),
      );
    }
    const audioProbe = finish(
      `${name}-audio-${audioInit ? "init.mp4" : "0.ts"}`,
    );
    return { videoProbe, audioProbe };
  }

  function completeAppends() {
    for (const [id, buffer] of buffers) {
      while (buffer.pending.length) {
        buffer.pending.shift();
        dispatcher.on_source_buffer_update(
          id,
          new JsTimeRanges(new Float64Array([2, buffer.end])),
        );
      }
    }
  }
  return {
    dispatcher,
    start,
    finish,
    completeAppends,
    requests,
    resources,
    appends,
    removals,
    offsets,
    errors,
    buffers,
    get ended() {
      return ended;
    },
  };
}

it.each([
  { audioInit: false, singleTrack: false },
  { audioInit: true, singleTrack: false },
  { audioInit: false, singleTrack: true },
])(
  "preserves probe reuse and continuity (audio init: $audioInit, single track: $singleTrack)",
  async ({ audioInit, singleTrack }) => {
    const ctx = await createProbeHarness({ audioInit, singleTrack });
    try {
      const { videoProbe, audioProbe } = ctx.start();
      const video = ctx.appends.find(
        (append) => append.resourceId === videoProbe,
      );
      if (singleTrack) {
        expect(video.fetches).toContain("https://example.com/first-video-1.ts");
      }
      // The host consumes the bytes synchronously. Pending appends must not
      // keep the original downloaded resources alive.
      expect(ctx.resources.has(videoProbe)).toBe(false);
      expect(ctx.resources.has(audioProbe)).toBe(false);
      expect(ctx.offsets).toEqual(audioInit || singleTrack ? [2] : [2, 2]);
      ctx.completeAppends();
      expect(ctx.resources.has(videoProbe)).toBe(false);
      expect(ctx.resources.has(audioProbe)).toBe(false);

      if (audioInit) ctx.finish("first-audio-0.ts");
      for (const type of singleTrack ? ["video"] : ["video", "audio"]) {
        ctx.finish(`first-${type}-1.ts`);
        ctx.finish(`first-${type}-2.ts`);
      }
      for (const type of singleTrack ? ["video"] : ["video", "audio"]) {
        const media = ctx.appends.filter(
          (append) =>
            append.url.includes(`-${type}-`) && append.url.endsWith(".ts"),
        );
        expect(media.map((append) => append.url.split("/").at(-1))).toEqual(
          [0, 1, 2].map((index) => `first-${type}-${index}.ts`),
        );
        expect(media[1].hints).toMatchObject({
          baseDecodeTimeStartLo: 6,
          baseDecodeTimeStartTimescale: 1,
          resetTransmuxerState: false,
        });
        expect(media[2].hints.resetTransmuxerState).toBe(true);
      }
      expect(ctx.ended).toBe(0);
      ctx.completeAppends();
      expect(ctx.ended).toBeGreaterThan(0);
      expect(ctx.errors).toEqual([]);
      expect(ctx.resources.size).toBe(0);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);

it.each([
  { singleTrack: true, audioInit: false },
  { singleTrack: false, audioInit: false },
  { singleTrack: false, audioInit: true },
])(
  "stops after one append error and can reload (single track: $singleTrack, audio init: $audioInit)",
  async ({ singleTrack, audioInit }) => {
    let fail = true;
    const ctx = await createProbeHarness({
      singleTrack,
      audioInit,
      failAppend: (append) =>
        fail && (singleTrack || append.url.includes("-audio-")),
    });
    try {
      const { videoProbe, audioProbe } = ctx.start();
      expect(ctx.errors).toHaveLength(1);
      expect(ctx.appends).toHaveLength(1);
      expect(ctx.appends[0].resourceId).toBe(
        singleTrack ? videoProbe : audioProbe,
      );
      expect(ctx.resources.size).toBe(0);
      expect(ctx.ended).toBe(0);
      const staleRequests = ctx.requests.filter(
        (request) => request.aborted && !request.finished,
      );
      for (const request of staleRequests)
        ctx.finish(request.url.split("/").at(-1), "late data", {
          obsolete: true,
        });
      expect(ctx.resources.size).toBe(0);
      expect(ctx.appends).toHaveLength(1);

      fail = false;
      ctx.start("reload");
      expect(
        ctx.appends.slice(1).every((append) => append.url.includes("/reload-")),
      ).toBe(true);
      ctx.completeAppends();
      ctx.dispatcher.stop();
      expect(ctx.resources.size).toBe(0);
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);

it("requests a failed append again after BufferFull cleanup despite old partial coverage", async () => {
  const ctx = await createProbeHarness({
    singleTrack: true,
    mediaDuration: 40,
  });
  const observe = (ranges) =>
    ctx.dispatcher.on_playback_tick(
      new MediaObservation(
        PlaybackTickReason.RegularInterval,
        2,
        4,
        new JsTimeRanges(new Float64Array(ranges)),
        true,
        false,
        false,
        42,
        undefined,
        new JsTimeRanges(new Float64Array(ranges)),
      ),
    );
  const requestsFor = (suffix) =>
    ctx.requests.filter((request) => request.url.endsWith(suffix));
  try {
    ctx.start();
    ctx.completeAppends();
    for (let index = 1; index < 8; index++) {
      ctx.finish(`first-video-${index}.ts`);
      ctx.completeAppends();
    }
    observe([2, 34]);
    ctx.dispatcher.set_buffer_goal(4);
    // Playlist positions are two seconds earlier than media positions.
    // The 4-8s segment loses its prefix; its old 6-8s data remains.
    observe([2, 6, 8, 34]);
    expect(requestsFor("first-video-1.ts")).toHaveLength(2);
    ctx.finish("first-video-1.ts");
    const [bufferId, buffer] = [...ctx.buffers][0];
    buffer.pending.length = 0;
    ctx.dispatcher.on_append_buffer_error(
      bufferId,
      PushedSegmentErrorCode.BufferFull,
      new JsTimeRanges(new Float64Array([2, 6, 8, 34])),
    );
    expect(ctx.removals).toHaveLength(2);
    expect(ctx.errors).toEqual([]);
    // Complete both cleanup removals while keeping the old partial data.
    for (const removal of ctx.removals) {
      ctx.dispatcher.on_source_buffer_update(
        removal.bufferId,
        new JsTimeRanges(new Float64Array([2, 6, 8, 14])),
      );
    }
    observe([2, 6, 8, 14]);
    expect(requestsFor("first-video-1.ts")).toHaveLength(3);
    ctx.finish("first-video-1.ts");
    ctx.completeAppends();
    observe([2, 10]);
    expect(requestsFor("first-video-1.ts")).toHaveLength(3);
    expect(ctx.errors).toEqual([]);
    expect(ctx.resources.size).toBe(0);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});
