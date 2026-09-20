# `wasm` directory

This directory contains the WebAssembly target file when built as well as the
handwritten code, in `./js`, allowing to link it to JavaScript.

Its `./abi` directory contains file useful both for file generation on both
sides (Rust and JavaScript) - such as the creation of synchronized enums and
bindings.

## Why not just `wasm-bindgen`?

This project initially relied on the Rust "crate" `wasm-bindgen` which takes
care of most of the glue code written manually here.

However, maintainance difficulty arised after updates where it became unclear if
they targeted a stable EcmaScript/DOM version. For this project this is very
important as streaming apps are in a great part specific environments with
sometimes old browser software (smart TVs, game consoles, set-top boxes etc.).

Moreover the idea of ensuring we control the glue code in potentially hot paths
is also a clear advantage.

## Supported WebAssembly features

Both debug and release builds target the WebAssembly MVP. The build uses a
nightly Rust toolchain with `rust-src` to rebuild the standard library for that
target, then validates the final artifact with Binaryen's `wasm-opt`. Install
nightly, `rust-src`, the `wasm32-unknown-unknown` target for nightly, and
Binaryen before building. `npm run install:binaryen` installs Binaryen locally
if it is unavailable on your path.
