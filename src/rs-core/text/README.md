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
- Languages are inherited and attached to subtitle nodes. Ruby base and reading
  text have distinct kinds; a renderer can build ruby elements from them.
- `Appearance` contains computed text properties. Font lists are shared; colors,
  sizes and decorations are typed. The output does not include style definitions or CSS selectors.
- Named TTML styles, ordinary inheritance and supported WebVTT CSS are resolved
  while parsing. Invalid declarations cannot replace a valid referenced value.
- Apply decorations only to nodes that contain text. Ancestor CSS underlines cannot be
  cancelled by a descendant, whereas TTML permits that cancellation.
- Resolve `Length::VideoHeight` against the displayed video image height, not
  the browser viewport. A browser backend can then set ordinary pixel values.
- `Placement` holds bounds and anchors. Measure the text, then resolve anchors
  using the containing box dimensions. Alignment is an enum, not flexbox syntax.
- Percent padding uses the containing region's height for top/bottom and width
  for left/right. Pixel TTML geometry scales when a root pixel extent is given.
- TODO: Scrolling/collisions, paragraphs split across regions, and shared region
  backgrounds when several paragraphs are active together.
- Effects use numeric opacity, visibility flags and a single outline width/color.
  Parent opacity also applies to children. Hidden nodes may have visible children;
  undisplayed nodes exclude their whole subtree.
- Region clipping and stacking are explicit. Outlines have no blur grammar.

`model.rs` defines the output. `imsc/` and `vtt/` read each format; `mp4.rs` reads
its container. Native and browser backends call the same parser functions.

## Where it stops

There is no renderer, FFI, CSS stylesheet or application CSS variable here.
TODO: Transfer whole documents through flat numeric tables and UTF-8 strings,
with JS copying the WASM data before Rust frees it. See `model.rs`.

Timed `<set>` changes and external resources are unsupported. Foreign XML and
metadata cannot introduce visible content or global styles. Source preservation
is a non-goal. These readers do not check every authoring rule.
EBU linePadding/multiRowAlign, escaped font names and complex CSS remain TODOs.

XXX TODO: Is it overkill?
`Limits` bounds input size, nesting, entries and estimated allocations, including
temporary copies. Freeing temporary data does not increase the allowance.
This is not a hard limit on total process or WebAssembly memory.

Tests and source references are in [tests](tests/README.md).
