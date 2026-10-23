# Subtitle parsers

This directory turns WebVTT and IMSC1 text subtitles into the same small tree.
The main entry points are `vtt::parse`, `imsc::parse`, and `imsc::parse_sample`.

Its job is to:

- Return text, HTML element names, CSS styles, parent indices and times in seconds.
- Resolve ordinary TTML parent and region styles, relative font sizes and whitespace.
- Apply supported WebVTT CSS rules, cue settings and markup.
- Map times to playback when given a `Timeline`. WebVTT cues keep their full range.
- Extract borrowed XML samples from MP4 with `mp4::parse_init` / `mp4::extract`.
- Skip unsupported styles and timed `<set>` elements. Source preservation is a non-goal.
- Return diagnostics for recoverable problems and errors when parsing cannot continue.

The renderer sets `viewport_style(width, height)` on a container covering the video
image, then builds elements from `Document::nodes` in order. A node is visible when
`start <= playback_time < end` and its parent is visible. `end` can be infinity.
Set the supplied styles, language and text directly; no format-specific data is needed.
Update the viewport styles when the video image size changes.

`parse_with_timeline` takes matching media and playback times. Use a new reference
after a discontinuity. `vtt::parse_init` reads an external WebVTT header for later
segments; it is parser state. `imsc::parse_sample` also applies MP4 sample coverage.

`render.rs` defines the shared output. `style.rs` contains internal checked values.
`imsc/` and `vtt/` handle each format; `mp4.rs` handles the container.
Trees use vectors and parent indices. Shared strings avoid copying large font names.
Tests and specification references are in [tests](tests/README.md).

## Where it stops

- The renderer handles CSS layout and font selection. Collision avoidance, scrolling
  WebVTT regions and inline timestamp highlighting remain TODOs.
- TTML timed styles, paragraphs split across regions and persistent region
  backgrounds remain TODOs. Basic region placement is supported.
- Extension styles such as EBU `linePadding` and `multiRowAlign` are ignored.
  Invalid values, unequal TTML font-size axes and IMSC-prohibited blur are discarded.
- IMSC support covers the text profile and media time. XML must be UTF-8 without DTDs.
- MP4 extraction rejects encrypted/subsampled documents, external resources and
  sample-description switches. These parsers do not check every authoring rule.

XXX TODO: Is it overkill?
`Limits` bounds input size, tree size, nesting and estimated allocations. The
`output_bytes` allowance counts temporary copies too, without refunding freed space.
It is not a cap on total process or WebAssembly memory.
