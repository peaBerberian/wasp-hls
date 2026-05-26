# scripts

This directory contains helper scripts used to build, test, generate and serve
the project.

The code is split into several directories:

- `tasks`: higher-level task entry points, generally called through npm scripts.

- `utils`: helper functions shared by multiple scripts.

- `packager`: scripts allowing to generate test HLS contents locally.

It also contains standalone scripts for more targeted operations, such as wasm
ABI checks, embedded asset generation and local server utilities.
