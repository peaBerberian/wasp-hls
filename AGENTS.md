# AGENTS.md

## Scope

These instructions apply to the whole repository.

## Project Summary

`wasp-hls` is a web HLS player library, not an end-user app. The public API lives in TypeScript, most playback logic lives in Rust compiled to WebAssembly, and a large part of runtime execution happens in a Web Worker.

## Source Layout

- `src/rs-core`: Rust core logic compiled to the crate's wasm artifact
- `src/ts-main`: main-thread TypeScript API and browser-facing glue
- `src/ts-worker`: worker-side TypeScript runtime and wasm bindings
- `src/ts-transmux`: MPEG-TS to fMP4 transmuxing logic
- `src/ts-common`: shared utilities
- `src/wasm`: handwritten JS/TS glue plus shared wasm ABI files
- `demo`: React demo app for manual testing and showcasing the API
- `tests`: transmux and browser integration tests
- `scripts/tasks`: canonical task entry points used by `npm run ...`
- `build`: generated outputs and runtime assets, prefer editing source files elsewhere

## Working Rules

- Start from the existing `README.md` files in the relevant directory before changing architecture-sensitive code
- Keep caller-specific assumptions in the caller; a callee should only rely on context made explicit in its inputs or contract
- Refactor when it makes the design clearer, including across modules; keep each change focused and leave the code working
- Don't forget to typecheck and format the code when doing modifications
- If the user uses `jj`, use it for history changes, preserve unrelated work and check for stale workspaces after moving their commits
- Use simple English in comments and docs
- If Chrome fails to start or connect, retry with Firefox; investigate test assertion failures normally

## Common Commands

- `npm run build`: build library artifacts
- `npm run demo`: build the demo and its dependencies
- `npm run start`: watch/rebuild during development and serve the demo
- `npm run check`: run standard TS and Rust checks
- `npm run test`: run all test suites
- `npm run test -- integration`: run browser integration tests against built artifacts
- `npm run test -- transmux`: run Node-based transmux tests
- `npm run test -- rust`: run Rust unit tests
- `npm run fmt`: format Rust, TS/JS, and Markdown
- `npm run generate`: regenerate wasm ABI enums

## Validation

- Run the smallest relevant check for the area you changed
- Run `npm run check` for mixed TS/Rust changes
- Run `npm run generate` after ABI edits, then rerun the relevant build/test command
- Run `npm run fmt -- --check` after the final generation, check or build command; those commands can regenerate ABI files

If a generated file fails formatting, fix the generator, then regenerate and check again. Do not just format the generated file. In particular, `scripts/generate_wasm_abi_bindings.mjs` must produce Rust declarations that match rustfmt in `src/wasm/rust/raw_host.rs`.
