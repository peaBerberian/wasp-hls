# tests/contents

This directory contains the content server and static fixtures used by the test
suite.

Its `server.mjs` file creates a small HTTP server exposing the test contents and
some helper endpoints to start, stop and inspect the live packaging process used
by integration tests.

The `static` directory contains static route definitions and metadata served by
that server.
