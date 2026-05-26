# scripts/tasks

This directory contains the higher-level tasks used by the repository's npm
scripts.

Its `index.mjs` file is the main entry point. It receives a task name and
dispatches to the corresponding implementation.

Those tasks cover build, check, test, report and development-server related
operations.
