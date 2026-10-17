# Rust utilities

This directory regroups small helpers used by the Rust code. There's no central
object coordinating them; `mod.rs` just exposes the individual modules.

- `codecs.rs` infers audio or video media types from recognized codec base names.

- `url.rs` provides an owned URL string with helpers to combine a base and a
  relative URL, recognize absolute URLs and extract parts such as the filename
  or extension. This is lightweight string handling, not a full URL parser or
  validator.

- `time_ranges.rs` provides an owned collection of chronological time ranges,
  with operations to add and merge ranges, remove them and find the range
  containing a position. It only knows about the numbers given to it, not what
  those ranges represent.

- `logger.rs` provides logging macros and a shared maximum log level. Formatting
  work is skipped when the corresponding level is disabled. Logs are forwarded
  to JavaScript on WebAssembly targets; native builds don't emit them.

## Where it stops

These helpers own their local data, plus the shared log level for logging. They
don't keep playback state, fetch resources or observe browser buffers. The
caller decides what the values mean and when to update them.
