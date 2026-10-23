# tests/transmux

This directory contains the Node-based test suite for the transmuxer.

Those tests focus on isolated transmuxing behaviors and can be run without a
browser through `npm run test -- transmux`. Rust cases build a separate release WASM test artifact first
and require FFmpeg and FFprobe.

The `helpers` directory contains small dedicated entry points used by some of
those tests.

After `npm run test -- transmux --filter rust-transmux`, run the opt-in microbenchmark with
`node tests/transmux/benchmark-rust.mjs segment.ts [segment.ts ...]`.
It includes JS/WASM copies and output allocation. Standalone handles exist only
in `tmp/transmux-test.wasm`; the production WASM ABI exposes none.

Optional competitors:

```sh
node tests/transmux/benchmark-rust.mjs segment-0.ts segment-1.ts \
  --hlsjs /path/to/hls.js \
  --shaka /path/to/shaka-player/dist/shaka-player.compiled.js \
  --iterations 100 --warmup 20 --report report.json
```

- Use consecutive, complete segments containing H.264 and AAC-LC.
- Install the hls.js checkout's dependencies first. `--hlsjs-deps node_modules`
  can supply a separate dependency directory without modifying that checkout.
- hls.js is bundled from source with esbuild and minified. Shaka uses the supplied
  compiled bundle. Reports identify both builds and hash the inputs and WASM.
- Each pass creates a fresh session outside timing, then feeds segments in order.
  Library order rotates between passes. Initialization work inside transmux calls
  is timed; session construction and destruction are not.
- Rust includes input/output copies. hls.js keeps its separate init/moof/mdat
  buffers. Shaka runs both its audio and video pipelines, as its player does.
- FFmpeg checks decoded video and encoded AAC before measurement. Extra leading
  AAC is accepted only when it decodes to silence; the report records it.
- The comparison measures calls returning native JS buffers, including any
  synchronous allocation or GC they trigger. It excludes decoding, network,
  worker transport and MSE. It does not compare whole-player performance or
  retained memory between libraries.
