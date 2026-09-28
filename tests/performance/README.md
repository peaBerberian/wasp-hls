# Performance tests

These tests compare selected Wasp HLS operations between the current working
tree and another Git branch, `main` by default.

```sh
npm run test -- performance
npm run test -- performance --branch my-base-branch --browser firefox
npm run test -- performance --filter hls_playlists
```

The runner lives in `tests/performance/run.mjs`.

Run performance tests with `npm run test -- performance`. They are not
included in the regular `npm run test` command.

The runner builds both revisions in release mode. It then runs an A/A control
and an A/B treatment in fresh browser processes, alternates the order of the
two slots, and compares the per-process mean and median differences with a
Mann–Whitney U test. A median regression has to be reproduced by a second run
before the command fails. Use `--report report.md` to write a report suitable
for a pull request comment.

The tests need the repository's JavaScript and Rust build dependencies, Git,
Chrome or Firefox, and the codecs used by the generated test content. By
default, ports 3000, 8080, and 6789 serve content, test pages, and results.

The runner recreates `tests/performance/node_modules` and generates JavaScript
bundles in this directory. Those files are ignored by Git.

Repeatable `--filter` options match suite path substrings. All suites run by
default.

Suite files in `src/suites` describe the operations they measure.

Groups can set `runEvery` through `declareTestGroup`. For example,
`runEvery: 3` runs on the 1st, 4th, 7th... visits to each version within a
browser process. Omitting it runs the group on every visit.

The runner accepts these environment variables:

- `WASP_HLS_PERF_CONTROL_ITERATIONS`: number of fresh browser processes for
  the A/A comparison of identical builds.
- `WASP_HLS_PERF_TREATMENT_ITERATIONS`: number of fresh browser processes for
  the A/B comparison of the baseline and current builds.
- `WASP_HLS_PERF_INNER_ITERATIONS`: page visits per browser process,
  alternating between comparison slots.

For a quick check of the setup, use one process per comparison and one visit
per slot:

```sh
WASP_HLS_PERF_CONTROL_ITERATIONS=1 \
WASP_HLS_PERF_TREATMENT_ITERATIONS=1 \
WASP_HLS_PERF_INNER_ITERATIONS=2 \
npm run test -- performance
```
