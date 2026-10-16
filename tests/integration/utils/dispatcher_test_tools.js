import { expect } from "vitest";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import initializeWasm, {
  Dispatcher,
  JsTimeRanges,
  MediaObservation,
  MediaType,
  PlaybackTickReason,
} from "../../../build/es6/wasm/js/index.js";

// Exercise the real dispatcher with simulated downloads and browser observations.
// This setup covers scheduling and startup decisions, not media decoding.
export async function createDispatcher({
  segmentDuration = 4,
  unsupportedCodec,
} = {}) {
  const requests = [];
  const aborted = [];
  const variants = [];
  const announcedVariants = [];
  const rebufferingEvents = [];
  const errors = [];
  const resources = new Map();
  let nextResource = 1;
  let nextBuffer = 1;
  const bindings = new Proxy(
    {
      fetch(url) {
        const id = requests.length + 1;
        requests.push({ id, url, finished: false });
        return id;
      },
      abortRequest(id) {
        aborted.push(id);
        return true;
      },
      getResourceData: (id) => resources.get(id),
      freeResource: (id) => resources.delete(id),
      announceFetchedContent(_type, info) {
        // Seven fields per variant, following the count.
        for (let i = 0; i < info[0]; i++) {
          variants.push(info[1 + i * 7]);
        }
      },
      announceVariantUpdate: (id) => announcedVariants.push(id),
      addSourceBuffer: () => ({ value: nextBuffer++ }),
      appendBuffer: () => ({
        value: {
          start: { hi: 0, lo: 0 },
          end: { hi: 0, lo: segmentDuration },
          timescale: 1,
        },
      }),
      isTypeSupported: (_type, codec) => codec !== unsupportedCodec,
      inspectSegment: () => ({
        value: {
          codec: unsupportedCodec ?? "avc1.42E01E",
          mimeType: "video/mp4",
          mediaType: MediaType.Video,
        },
      }),
      monotonicNow: () => 1000,
      getRandom: () => 0.5,
      timer: () => 1,
      startRebuffering: () => rebufferingEvents.push("start"),
      stopRebuffering: () => rebufferingEvents.push("stop"),
    },
    {
      get(target, name) {
        if (name in target) {
          return target[name];
        }
        if (String(name).startsWith("send")) {
          return (...args) => errors.push({ name, args });
        }
        return () => ({ value: true });
      },
    },
  );
  // A fresh instance also isolates the other tests from a trapping dispatcher.
  await initializeWasm(
    await (await fetch(EmbeddedWasm)).arrayBuffer(),
    bindings,
  );
  const dispatcher = new Dispatcher(1_875_000);

  function finish(suffix, body = "media") {
    const request = requests.find(
      (request) => !request.finished && request.url.endsWith(suffix),
    );
    expect(request, `Expected a pending request for ${suffix}`).toBeDefined();
    request.finished = true;
    const resourceId = nextResource++;
    resources.set(resourceId, new TextEncoder().encode(body));
    dispatcher.on_request_finished(
      request.id,
      resourceId,
      1000,
      request.url,
      100,
    );
  }

  function observe(bufferLevel, position = 0) {
    const ranges = () =>
      new JsTimeRanges(new Float64Array([0, position + bufferLevel]));
    dispatcher.on_playback_tick(
      new MediaObservation(
        PlaybackTickReason.RegularInterval,
        position,
        4,
        ranges(),
        true,
        false,
        false,
        segmentDuration * 8,
        ranges(),
        ranges(),
      ),
    );
  }

  function playlist(prefix) {
    return (
      `#EXTM3U\n#EXT-X-TARGETDURATION:${segmentDuration}\n` +
      Array.from(
        { length: 8 },
        (_, i) => `#EXTINF:${segmentDuration},\n${prefix}-${i}.ts\n`,
      ).join("") +
      "#EXT-X-ENDLIST\n"
    );
  }

  return {
    dispatcher,
    finish,
    observe,
    playlist,
    requests,
    aborted,
    variants,
    announcedVariants,
    rebufferingEvents,
    errors,
  };
}
