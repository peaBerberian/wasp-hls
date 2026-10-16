import { expect, it } from "vitest";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import initializeWasm, {
  Dispatcher,
  JsTimeRanges,
  MediaObservation,
  MediaSourceReadyState,
  MediaType,
  PlaybackTickReason,
  StartingPosition,
  StartingPositionType,
} from "../../../build/es6/wasm/js/index.js";

// Exercise the real WASM dispatcher with synchronous host stubs. No browser media
// decoding is involved: these assertions cover requests, resource lifetime, and reuse.
it.each([
  { hasInit: true, manifestCodecs: false, opaqueUrls: false },
  { hasInit: false, manifestCodecs: false, opaqueUrls: false },
  { hasInit: true, manifestCodecs: true, opaqueUrls: false },
  { hasInit: false, manifestCodecs: true, opaqueUrls: false },
  { hasInit: true, manifestCodecs: true, opaqueUrls: true },
  { hasInit: false, manifestCodecs: true, opaqueUrls: true },
  {
    hasInit: true,
    manifestCodecs: true,
    opaqueUrls: false,
    lateFirstObservation: true,
  },
  {
    hasInit: true,
    manifestCodecs: true,
    opaqueUrls: false,
    lateFirstObservation: true,
    startCase: "fromEnd",
  },
  {
    hasInit: true,
    manifestCodecs: false,
    opaqueUrls: false,
    switchBeforeBuffer: true,
  },
  ...[true, false].flatMap((hasInit) =>
    ["absolute", "fromEnd", "liveTimeline"].map((startCase) => ({
      hasInit,
      manifestCodecs: false,
      opaqueUrls: false,
      startCase,
    })),
  ),
])(
  "waits for buffers and reuses probes: $hasInit init, $manifestCodecs codecs, $opaqueUrls opaque URLs, start $startCase",
  async ({
    hasInit,
    manifestCodecs,
    opaqueUrls,
    switchBeforeBuffer,
    lateFirstObservation,
    startCase,
  }) => {
    const success = (value) => ({ value });
    const requests = [];
    const resources = new Map();
    const appended = [];
    const inspected = [];
    const createdBuffers = [];
    const freed = [];
    const bufferOperations = [];
    const rebufferingEvents = [];
    let streamEnded = false;
    let variants;
    let nextResourceId = 1;
    const bindings = new Proxy(
      {
        fetch(url) {
          const id = requests.length + 1;
          requests.push({ id, url, finished: false });
          return id;
        },
        getResourceData: (id) => resources.get(id),
        freeResource(id) {
          freed.push(id);
          return resources.delete(id);
        },
        announceFetchedContent(_type, info) {
          // Each variant has seven fields following the variant count.
          variants = [info[1], info[8]];
        },
        addSourceBuffer(_mediaType, contentType) {
          createdBuffers.push(contentType);
          return success(1);
        },
        inspectSegment(resourceId) {
          inspected.push(resourceId);
          return success({
            codec: "avc1.42E01E",
            mimeType: "video/mp4",
            mediaType: MediaType.Video,
          });
        },
        isTypeSupported: () => true,
        appendBuffer(_bufferId, resourceId) {
          appended.push(resourceId);
          bufferOperations.push("append");
          return success({
            start: { hi: 0, lo: 0 },
            end: { hi: 0, lo: 4 },
            timescale: 1,
          });
        },
        removeBuffer() {
          bufferOperations.push("remove");
          return success(true);
        },
        monotonicNow: () => 1000,
        getRandom: () => 0.5,
        timer: () => 1,
        startRebuffering: () => rebufferingEvents.push("start"),
        stopRebuffering: () => rebufferingEvents.push("stop"),
        endOfStream() {
          streamEnded = true;
          return success(true);
        },
      },
      {
        get(target, name) {
          if (name in target) {
            return target[name];
          }
          if (String(name).startsWith("send")) {
            return (...args) => {
              throw new Error(`${String(name)}: ${JSON.stringify(args)}`);
            };
          }
          return () => success(true);
        },
      },
    );
    const wasm = await (await fetch(EmbeddedWasm)).arrayBuffer();
    await initializeWasm(wasm, bindings);
    const dispatcher = new Dispatcher(1_000_000);

    function finishBufferOperations() {
      while (bufferOperations.length > 0) {
        bufferOperations.shift();
        dispatcher.on_source_buffer_update(1, new JsTimeRanges([]));
      }
    }

    function finish(suffix, body) {
      const request = requests.find(
        (entry) => !entry.finished && entry.url.endsWith(suffix),
      );
      expect(request, `Expected request for ${suffix}`).toBeDefined();
      request.finished = true;
      const resourceId = nextResourceId++;
      const bytes = new TextEncoder().encode(body);
      resources.set(resourceId, bytes);
      dispatcher.on_request_finished(
        request.id,
        resourceId,
        bytes.length,
        request.url,
        100,
      );
      return resourceId;
    }

    function playlist(prefix) {
      const init = hasInit ? `#EXT-X-MAP:URI="${initUrl(prefix)}"\n` : "";
      const live = startCase === "liveTimeline";
      const timing = live
        ? "#EXT-X-PROGRAM-DATE-TIME:2024-01-02T03:04:05.000Z\n"
        : "";
      const segments = Array.from(
        { length: live ? 6 : 2 },
        (_, index) => `#EXTINF:4,\n${mediaUrl(prefix, index)}\n`,
      ).join("");
      return `#EXTM3U\n#EXT-X-TARGETDURATION:4\n${init}${timing}${segments}${live ? "" : "#EXT-X-ENDLIST\n"}`;
    }

    const initUrl = (prefix) => `${prefix}-init${opaqueUrls ? "" : ".mp4"}`;
    const mediaUrl = (prefix, index) =>
      `${prefix}-${index}${opaqueUrls ? "" : ".m4s"}`;
    const startingIndex = startCase === "liveTimeline" ? 3 : startCase ? 1 : 0;
    const probeSuffix = (prefix) =>
      hasInit ? initUrl(prefix) : mediaUrl(prefix, startingIndex);
    const requestCount = (suffix) =>
      requests.filter((request) => request.url.endsWith(suffix)).length;

    try {
      const startingPosition =
        startCase === "absolute"
          ? new StartingPosition(StartingPositionType.Absolute, 4)
          : startCase === "fromEnd"
            ? new StartingPosition(
                StartingPositionType.FromEnd,
                lateFirstObservation ? 2 : 4,
              )
            : undefined;
      dispatcher.load_content(
        "https://example.com/master.m3u8",
        startingPosition,
      );
      const codecs = manifestCodecs ? ',CODECS="avc1.42E01E"' : "";
      finish(
        "master.m3u8",
        `#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000${codecs}\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000${codecs}\nhigh.m3u8\n`,
      );
      finish("low.m3u8", playlist("low"));
      const needsProbe = !manifestCodecs || opaqueUrls;
      let initialProbe;
      if (needsProbe) {
        initialProbe = finish(probeSuffix("low"), "initial probe bytes");
        expect(inspected).toContain(initialProbe);
        expect(freed).not.toContain(initialProbe);
        expect(appended).not.toContain(initialProbe);
      }

      // Reconsider scheduling while the initial probe is waiting for its buffer.
      dispatcher.set_buffer_goal(20);
      expect(freed).not.toContain(initialProbe);
      expect(requestCount(probeSuffix("low"))).toBe(needsProbe ? 1 : 0);

      if (switchBeforeBuffer) {
        dispatcher.lock_variant(variants[1]);
        finish("high.m3u8", playlist("high"));
        dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
        // The new selection still lacks metadata, so buffer creation must wait.
        expect(createdBuffers).toEqual([]);
        const replacementProbe = finish(
          probeSuffix("high"),
          "replacement probe bytes",
        );
        expect(inspected).toContain(replacementProbe);
        expect(createdBuffers).toEqual(['video/mp4;codecs="avc1.42E01E"']);
        expect(appended).not.toContain(initialProbe);
        expect(appended).toContain(replacementProbe);
        finishBufferOperations();
        expect(freed).toContain(initialProbe);
        expect(freed).toContain(replacementProbe);
        expect(requestCount(probeSuffix("high"))).toBe(1);
        return;
      }

      dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
      if (!needsProbe) {
        initialProbe = finish(probeSuffix("low"), "initial segment bytes");
        expect(inspected).not.toContain(initialProbe);
      }
      expect(appended).toContain(initialProbe);
      finishBufferOperations();
      expect(freed).toContain(initialProbe);
      expect(requestCount(probeSuffix("low"))).toBe(1);

      if (lateFirstObservation) {
        finish(mediaUrl("low", startingIndex), "media bytes");
        finishBufferOperations();
        const nearEnd = startCase === "fromEnd";
        if (nearEnd) {
          expect(streamEnded).toBe(true);
        }
        // A slow main thread may first report playback after data is already buffered.
        // Startup must still announce buffering completion so the API can become Loaded.
        // At EOF, decoded buffer ends can differ slightly from the media duration.
        const buffered = new Float64Array(nearEnd ? [4, 7.98] : [0, 4]);
        for (const readyState of nearEnd ? [2, 2, 4, 4] : [4]) {
          dispatcher.on_playback_tick(
            new MediaObservation(
              PlaybackTickReason.RegularInterval,
              nearEnd ? 6 : 0,
              readyState,
              new JsTimeRanges(buffered),
              true,
              false,
              false,
              nearEnd ? 8.03 : 8,
              new JsTimeRanges([]),
              new JsTimeRanges(buffered),
            ),
          );
          expect(rebufferingEvents).toEqual(
            readyState < 3 ? ["start"] : ["start", "stop"],
          );
        }
        return;
      }

      if (startCase) {
        // No observation has advanced the selectors yet. Startup itself must select
        // the requested position, including when it falls exactly on a boundary.
        for (let index = 0; index < startingIndex; index++) {
          expect(requestCount(mediaUrl("low", index))).toBe(0);
        }
        if (hasInit) {
          expect(requestCount(mediaUrl("low", startingIndex))).toBe(1);
        }
        return;
      }

      dispatcher.lock_variant(variants[1]);
      finishBufferOperations();
      finish("high.m3u8", playlist("high"));
      const laterProbe = finish(probeSuffix("high"), "later probe bytes");
      expect(inspected.includes(laterProbe)).toBe(needsProbe);
      expect(appended).toContain(laterProbe);
      finishBufferOperations();
      expect(freed).toContain(laterProbe);
      expect(requestCount(probeSuffix("high"))).toBe(1);
    } finally {
      dispatcher.stop();
      dispatcher.free();
    }
  },
);
