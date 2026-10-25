# Subtitle parsers

This directory reads WebVTT and IMSC1 text subtitles into the same small tree.
Use `vtt::parse`, `imsc::parse`, or `imsc::parse_sample` for extracted MP4 samples.

- `Document::nodes` contains paragraphs, inline text and line breaks. The parent
  has a lower index than its children. Times use seconds during playback.
  A node is active at its start time, but not at its end time.
  It can be displayed only while its parent is active too.
- Whitespace and character references are processed here. Display the returned
  text without changing its spaces or newlines. Cues keep their full time ranges.
- `Timeline` supplies matching media and playback times. Give a fresh reference
  after a discontinuity. Only the parser uses WebVTT initialization headers.
- `mp4::parse_init` and `mp4::extract` return track information and borrowed XML
  samples. `imsc::parse_sample` applies sample coverage before returning text.
- UTF-8 XML, namespaces, CDATA, media timing and bounded parsing are supported.
  DTDs, image profiles and encrypted/subsampled MP4 documents are rejected.
- Language labels are omitted. Ruby keeps its base text and drops inline reading
  annotations. Both can be added without changing the parser functions.
- TODO: Add colors, fonts and positions from the subtitle file. Apply styles
  from parent nodes too. Styles and cue settings are skipped for now.

`model.rs` defines the output. `imsc/` and `vtt/` read each format; `mp4.rs` reads
its container. Native and browser backends call the same parser functions.

## Where it stops

There is no renderer, FFI, CSS stylesheet or application CSS variable here.
TODO: Transfer whole documents through flat numeric tables and UTF-8 strings,
with JS copying the WASM data before Rust frees it. See `model.rs`.

Timed `<set>` changes and external resources are unsupported. Foreign XML and
metadata cannot introduce visible content or global styles. Source preservation
is a non-goal. These readers do not check every authoring rule.

XXX TODO: Is it overkill?
`Limits` bounds input size, nesting, entries and estimated allocations, including
temporary copies. Freeing temporary data does not increase the allowance.
This is not a hard limit on total process or WebAssembly memory.

Tests and source references are in [tests](tests/README.md).
