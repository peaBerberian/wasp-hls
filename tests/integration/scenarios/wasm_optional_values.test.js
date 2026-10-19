import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createWasmImports } from "../../../build/es6/wasm/js/imports.js";
import { rawOptionalId } from "../../../build/es6/wasm/js/helpers.js";
import { RequestErrorReason } from "../../../build/es6/wasm/js/enums.js";
import WaspSegmentRequestError from "../../../build/es6/ts-main/errors/WaspSegmentRequestError.js";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";

describe("WASM optional unsigned values", () => {
  let ctx;
  beforeAll(async () => {
    // Initialize real WASM memory for the imports that read strings and timing.
    ctx = await createDispatcher();
  });
  afterAll(() => ctx?.dispatcher.free());

  it.each([
    [0, 0],
    [1, 1],
    [0x7fffffff, 0x7fffffff],
    [-0x80000000, 0x80000000],
    [-2, 0xfffffffe],
    [-1, undefined],
    [0xffffffff, undefined],
  ])("decodes raw %s as %s", (raw, expected) => {
    expect(rawOptionalId(raw)).toBe(expected);
  });

  it.each([0, 1, 0x7fffffff, 0x80000000, 0xfffffffe])(
    "preserves unsigned track selection IDs (%s)",
    (trackId) => {
      const calls = [];
      const imports = createWasmImports({
        sendTrackSelectionError: (...args) => calls.push(args),
      }).wasp;
      imports.__js_func__send_track_selection_error(0, 1, trackId | 0, 0, 0);
      expect(calls.at(-1)[2]).toBe(trackId);
    },
  );

  // Each optional argument is paired with its position in the host callback.
  it.each([
    ["announce_variant_update", "announceVariantUpdate", 1, [[0, 0]]],
    ["announce_track_update", "announceTrackUpdate", 2, [[1, 1]]],
    [
      "announce_variant_lock_status_change",
      "announceVariantLockStatusChange",
      1,
      [[0, 0]],
    ],
    [
      "send_segment_request_error",
      "sendSegmentRequestError",
      9,
      [
        [4, 4],
        [8, 6],
      ],
    ],
    [
      "send_multivariant_playlist_request_error",
      "sendMultivariantPlaylistRequestError",
      5,
      [[4, 3]],
    ],
    [
      "send_media_playlist_request_error",
      "sendMediaPlaylistRequestError",
      6,
      [
        [4, 3],
        [5, 4],
      ],
    ],
    [
      "send_media_playlist_parsing_error",
      "sendMediaPlaylistParsingError",
      5,
      [[2, 2]],
    ],
    ["send_segment_parsing_error", "sendSegmentParsingError", 5, [[2, 2]]],
  ])(
    "decodes all optional arguments of %s",
    (rawName, hostName, count, positions) => {
      const calls = [];
      const imports = createWasmImports({
        [hostName]: (...args) => calls.push(args),
      }).wasp;
      for (const [rawIndex, hostIndex] of positions) {
        for (const value of [
          0,
          1,
          200,
          404,
          503,
          0x7fffffff,
          0x80000000,
          0xfffffffe,
          undefined,
        ]) {
          const args = Array(count).fill(0);
          // Reproduce the signed i32 supplied by WASM to an imported JS function.
          args[rawIndex] = value === undefined ? -1 : value | 0;
          imports[`__js_func__${rawName}`](...args);
          expect(calls.at(-1)[hostIndex]).toBe(value);
        }
      }
    },
  );

  it("can construct a probe request error when the media type and status are unknown", () => {
    let error;
    const imports = createWasmImports({
      sendSegmentRequestError: (
        _fatal,
        url,
        isInit,
        timeInfo,
        mediaType,
        reason,
        status,
      ) => {
        error = new WaspSegmentRequestError({
          url,
          isInit,
          mediaType,
          reason,
          status,
          start: timeInfo?.[0],
          duration: timeInfo?.[1],
        });
      },
    }).wasp;
    expect(() =>
      imports.__js_func__send_segment_request_error(
        1,
        0,
        0,
        0,
        -1,
        0,
        0,
        RequestErrorReason.Error,
        -1,
      ),
    ).not.toThrow();
    expect(error.code).toBe("SegmentRequestError");
    expect(error.message).toContain("A probe segment");
  });
});
