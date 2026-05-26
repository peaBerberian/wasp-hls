# tests

This directory contains the different test suites and test helpers for
Wasp HLS.

The code is split into several directories:

- `contents`: local HTTP server and static fixtures used to expose test
  contents.

- `integration`: browser-based integration tests running against built player
  artifacts.

- `transmux`: Node-based tests for the transmuxing logic.

- `utils`: test helpers reused by multiple suites.

The `globalSetup.mjs` file starts shared infrastructure before tests, currently
the local content server used by integration tests.
