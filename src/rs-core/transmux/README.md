# Rust transmuxer

- Converts complete H.264/AAC MPEG-TS and packed ADTS segments to fMP4.
- Playback owns its parser in Rust's `SourceBuffer`; probes create a temporary
  Rust parser. JS receives completed bytes, never a playback session address.
- `transport` borrows input ranges; `aac` and `h264` describe samples. `mp4`
  assembles one output buffer. No encoded GOP payload is retained between calls.
- Continuous video keeps predictive pictures. Fresh starts drop pictures before
  the first IDR, preserving the remaining pictures' real timestamps.
- Without AUD, picture boundaries follow PES boundaries, like the reference
  players. Multiple pictures inside one PES without AUD remain ambiguous.
- Input/output coexist during assembly. The 64 MiB input, 128 MiB output and
  131,072 samples-per-track caps are limits, not reservations or a total budget.
- WASM memory grows permanently; freed allocations can be reused. JS input and
  output buffers also contribute to process memory.
- Playback copies input into WASM and output once into JS-owned storage safe
  for asynchronous MSE or transfer. MP4 passthrough avoids the WASM input copy.
- TS requires PAT/PMT in each input. Separate TS `EXT-X-MAP` initialization is
  deliberately unsupported, matching the old player path.
- Encryption is outside the current scope. AAC PCE/multiple raw blocks are
  explicitly rejected instead of generating incomplete decoder settings/timing.
- Packed ID3 accepts plain v2.3/v2.4. Extended headers and unsynchronization
  still need compatibility work. Packed AAC also accepts raw ADTS without the
  timestamp ID3 tag required by HLS.
- Ported mux.js-derived logic retains its attribution in LICENSE.

TODOs:

- Support separate TS `EXT-X-MAP` initialization and retain PAT/PMT mappings
  until the initialization changes or the parser is reset.
- Incremental input/output and LL-HLS part support, including split PES/frames.
- Additional codecs and metadata/caption delivery beyond the old media API.
- Faster H.264 start-code scanning; profile before changing other hot paths.

Checks: `npm run test -- rust`, `npm run test -- transmux`, `npm run check`.

Standalone handles live only in the `transmux-test-api` feature and the separate
`tmp/transmux-test.wasm` artifact. They are absent from production builds.
After the transmux tests build that artifact, use
`node tests/transmux/benchmark-rust.mjs segment.ts [segment.ts ...]`.
