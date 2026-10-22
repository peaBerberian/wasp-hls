# Player benchmarks

Independent, opt-in comparisons. Competitor packages and TypeScript are local
to this private package; normal tests and the published library are unaffected.
Requires Chrome, FFmpeg, GPAC, root build dependencies, and Node 22.18+ for tests.
The hls.js ES-module build uses its shipped external worker; worker errors fail
the run instead of silently falling back to main-thread transmuxing.

```sh
npm ci --prefix tests/player-benchmarks
npm run build -- --release
npm run test -- player-benchmarks --iterations 7 --report /tmp/players.json
npm run check --prefix tests/player-benchmarks
npm test --prefix tests/player-benchmarks
```

Use `--help` for selections. Flags can be repeated. Defaults: all three players,
both formats, declared/missing-codec masters, startup/reload, five samples per cell.
Scenario names are local conveniences,
not a versioned API.

Measurement boundaries:

- Startup reports initialization separately from load-to-metadata, canplay,
  playing and first-frame callback. Script evaluation is excluded.
- All players receive HTML media-element play at canplay; player-specific
  load promises do not define readiness. A paused preview frame can precede playing.
- Reload uses the existing player; new metadata gates readiness and frame
  observations. Seek checks that the same one-second target is buffered.
- Playback/pressure run 16 seconds with a four-second forward-buffer target.
  Pressure updates 400 DOM nodes, forces layout and performs 48 JSON round
  trips per batch, then waits 16 ms. This is main-thread work, not memory stress.
  Select both scenarios for interleaved controls; actual work and stalls are recorded.
- `memory` compares all players after 16 seconds of playback, paused with the same
  first 20 seconds buffered. Later media requests remain pending; their timeout is
  extended for these cases only. Back buffers are retained. Range mismatches fail.
- `memory-large-playlist` keeps the full two-hour playlist but the same media window.
  Compare it with `memory` to investigate playlist overhead.
- `memory-churn` adds three buffered seeks and reloads with post-GC checkpoints.
  These measure retained application memory, including workers, not decoder/GPU totals.
- Memory runs use isolated pages and forced GC; their timing is not reported as a
  performance result. Linux also samples Chrome process-tree PSS every 250 ms,
  including native allocations. Its sampled high-water mark can miss short peaks.

Content:

- GPAC packages one shared 60-second H.264/AAC encode as separate-track fMP4
  or muxed TS: 960×540, 24 fps, ~1600/128 kbps, two-second segments.
- Defaults cover declared CODECS and the explicit `no-codecs` probe case.
  Repeat `--manifest master|no-codecs|direct` to select cases. Direct separate-track fMP4 is rejected because
  it would omit audio; optional `--format fmp4-muxed` keeps both tracks.
- Large playlists have real two-hour timelines and unique segment URLs, not
  repeated discontinuities. FFmpeg loops encoded samples with new timestamps;
  images repeat every minute. Generation is opt-in, caching ~3 GB for both formats.
- Generation precedes timing. Recipe/tool versions key the cache; source and
  manifest hashes are recorded. Incomplete generation is not reused.

Each sample uses a fresh Chrome process/profile and no-store content. Player
and condition orders rotate. Raw samples are retained; no ranking blocks CI.
The local server uses an ephemeral port, with a per-sample watchdog and cleanup.

Scenario implementations live in `src/scenarios/`. Pressure reuses playback;
large playlists reuse startup. Formats and manifest metadata select fixtures,
not duplicate scenario implementations. `src/runner.ts` owns setup and cleanup.
Node helpers handle configuration, fixtures and browser lifetime.

Use this opt-in suite when changing startup, scheduling or buffering; run a
short selection while developing, then repeated comparisons before drawing
conclusions. Pressure and large-playlist runs remain explicit, not routine CI gates.

- Use the Wasp-versus-baseline suite for regressions; competitors provide context.
- Run cross-player cases for relevant scheduling, parsing or buffering changes and
  before releases, not on every PR. Keep JSON reports; small timing gaps do not gate CI.
- Memory example: `npm run test -- player-benchmarks --scenario memory --manifest master --iterations 3`.

`--diagnostic` is session-only tracing that rewrites built workers and changes
costs: never mix it with baseline results. Document Resource Timing omits
worker requests and is not a cross-player request total.
