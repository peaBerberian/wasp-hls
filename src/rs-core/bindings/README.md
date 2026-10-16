# Calling JavaScript from Rust

This directory regroups the Rust-side interface for calling JavaScript: requesting
resources, acting on media buffers and playback, starting timers, reporting
metadata and errors etc. Its functions and shared types are exported through
`mod.rs`.

`js_functions/mod.rs` provides the wrappers used by Rust code. It translates
Rust values into the raw ABI representation and turns returned values back into
usable metadata or errors. This includes strings, optional values and precise
segment timestamps.

The raw calls are declared in `js_functions/raw_wasm.rs`. On non-WebAssembly
targets, `js_functions/raw_host.rs` provides replacements so the code can compile
for native checks and tests. Both files are automatically generated and should
not be modified by hand. The native replacements panic when called; they don't
simulate the browser.

`abi_types.rs` defines the shared enums and numeric codes. It is also
automatically generated and should not be modified by hand. Use `npm run generate`
to regenerate those three files after changing the ABI definitions.
`formatters.rs` handles additional packing of content metadata and errors for
JavaScript.

## Where it stops

This is the call boundary, with the memory conversions needed to cross it. It
doesn't own the browser objects or decide when requests, buffer operations or
playback actions should happen. A successful call doesn't always mean the work
has finished: some operations report completion asynchronously.

Memory ownership matters here. Packed metadata can contain pointers into
existing Rust data, which has to stay alive while JavaScript reads it. Returned
owned strings are taken back into Rust, while downloaded resources generally
stay in JavaScript memory behind resource IDs. Those resources need to be freed
when no longer wanted; copying their bytes into Rust doesn't free them.
