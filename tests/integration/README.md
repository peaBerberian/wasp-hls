# tests/integration

This directory contains browser-based integration tests.

Those tests are run through `scripts/run_integration_tests.mjs`, which starts
Vitest in browser mode and executes the scenarios against the built Wasp HLS
artifacts.

The code is split into the following directories:

- `scenarios`: actual test files covering end-to-end playback situations.

- `utils`: helpers specific to integration tests.
