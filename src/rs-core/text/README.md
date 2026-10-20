# Subtitle parsers

This directory is where we read subtitle segments and turn them into text,
timing and style data. The main entry points are `vtt::parse`, `imsc::parse`,
and `mp4::parse_init` / `mp4::extract`.

Its job is to:

- Read WebVTT cues, text markup, regions, CSS rules and timestamp maps. Cues keep
  their full time range, even when they extend outside the segment.

- Read IMSC1 text documents, keeping their timed text tree, styles, regions and
  timed style changes. Spaces, language and parent links are kept for later
  processing.

- Extract XML samples from MP4 subtitle segments, with their track information
  and sample timestamps. The samples borrow the downloaded bytes to avoid
  copying each XML document.

- Check supported style values using each format's rules. Unsupported or invalid
  values stay as source text and must not be applied as checked styles. CSS rule
  order and `!important` are kept too.

- Report problems that still leave a usable document through diagnostics. Fatal
  errors return no document.

Parsed documents own their data. Named IMSC styles and text values are shared
to avoid repeated copies. Text trees use vectors and indices, so dropping a deep
tree does not recursively drop child objects.

`vtt.rs` and `vtt/` contain the WebVTT parser and timestamp helpers. `imsc/`
contains the XML, timing and style handling for IMSC. `mp4.rs` extracts subtitle
samples without parsing their XML.

`style.rs` defines the shared style values; CSS and IMSC each have their own
readers for those values. `mod.rs` defines common errors, diagnostics and limits.
`budget.rs` tracks memory estimates while parsing.

Tests and specification references are described in [tests](tests/README.md).

## Where it stops

This code reads subtitles without a browser or renderer. The caller applies
whitespace rules, inherited styles and layout, then displays the text. XML from
other namespaces and metadata cannot define styles or regions for visible text.

The caller also maps subtitle times to playback. WebVTT timestamp mapping needs
an audio/video timestamp, with a new reference after a discontinuity. IMSC
document times and MP4 sample times stay separate; the caller applies sample
coverage and playback offsets.

IMSC support is limited to the text profile and media time. XML must be UTF-8
and cannot contain DTDs. MP4 extraction rejects encrypted or subsampled
documents, external or auxiliary resources, and sample-description switches.
The parsers do not check every authoring rule. Complex CSS and extension styles
may stay as source text, and checks that need final font sizes or layout are
left to later processing.

XXX TODO: Is it overkill?
`Limits` bounds input size, tree size and nesting depth. Its `output_bytes`
budget estimates memory for parser output and temporary copies, with checks
before allocation. Space is not returned to that budget during a parse, even
when a temporary value is freed. This is not a cap on total process or
WebAssembly memory; downloaded input and allocator overhead also affect usage.
