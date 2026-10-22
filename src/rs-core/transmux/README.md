# Rust transmuxer

- Converts complete H.264/AAC MPEG-TS and packed ADTS segments to fMP4.
- `Transmuxer` owns timeline, rollover, decoder settings and bounded GOP history.
- Playback state belongs to Rust's `SourceBuffer`; JS borrows it synchronously.
- `transport` extracts borrowed payload ranges; `aac` and `h264` describe samples.
- `mp4` sizes output before assembly. `ffi` documents ownership and ABI layout.
- Implementation budgets, not HLS limits: 64 MiB input, 128 MiB output and
  131,072 samples per track.
- Cached GOP: at most 2 MiB and 4,096 samples. When history cannot be saved, discard leading pictures up to the next IDR.
- No input segments or media-sized scratch buffers survive a call.
- WASM memory grows permanently; released allocations are reusable, not returned.
- Cross-boundary copies: input into WASM, output into a separate JS ArrayBuffer
  that the worker can transfer to the main thread.
- Payload assembly copies once; bounded GOP history adds a copy when saved.
- Rejects encrypted TS, truncated input, AAC PCE/multiple raw blocks, and
  interlaced AVC without AUD. Other codecs and metadata delivery are unsupported.
- Timeline and GOP repair follow the existing mux.js-derived TS code; see LICENSE.
- Packed ID3 accepts plain v2.3/v2.4 tags; extended headers and unsynchronization
  require further work.

Compatibility work still needed:

- These unsupported inputs are implementation gaps, not HLS prohibitions.
- Complete input is required; streaming chunks and LL-HLS `EXT-X-PART` have not
  been validated. A truncated buffer and a valid partial segment are different.
- HLS permits TS initialization through `EXT-X-MAP`; this parser currently needs
  PAT and PMT in each input.
- HLS recommends an IDR in each video segment but does not require independent
  decoding unless signaled. The bounded GOP fallback needs further review for
  continuous segments whose pictures depend on previously appended data.
- Packed AAC currently also accepts raw ADTS without Apple's timestamp ID3 tag;
  HLS requires that tag for Packed Audio.

Checks: `npm run test -- rust`, `npm run test -- transmux`, `npm run check`.

Opt-in measurement after a release build:
`node tests/transmux/benchmark-rust.mjs segment.ts [segment.ts ...]`.
It includes WASM copies and cache updates, and does not measure browser playback.
