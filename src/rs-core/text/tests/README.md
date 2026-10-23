# Subtitle parser tests

- Run all subtitle tests: `cargo test text::`.
- Run snapshots: `cargo test text::tests::fixture_tests`.
- Update snapshots explicitly: `UPDATE_SUBTITLE_FIXTURES=1 cargo test text::tests::fixture_tests`.
- Keep the existing fixture inputs. Expected files show the complete output at
  each stage. Normal test runs do not update these files.
- Timing matrices, malformed input, every truncation, many XML attributes,
  namespaces, whitespace, references and allocation limits are checked separately.
- MP4 tests cover timing defaults, sample boundaries, corrupt boxes, extraction
  restrictions and truncation. Extracted sample data borrows the input.
- Language/ruby tests distinguish base text from pronunciation guides and check
  inherited language on subtitle nodes, without altering application elements.
- Property-value fixture tables keep CSS and TTML grammars separate. Tests cover
  invalid overrides, style cycles, hidden definitions, font-list sharing, CSS
  specificity/importance and a scanner work counter independent of machine speed.
- Placement tests cover TTML document coordinates, region alignment, WebVTT cue
  anchors, settings order, region dimensions and both vertical writing modes.

Reference material:

- Local `hls2026.txt`, especially WebVTT and IMSC segment boundaries.
- [WebVTT parsing](https://www.w3.org/TR/2026/CRD-webvtt1-20260520/#parsing).
- [IMSC1 text profile](https://www.w3.org/TR/2016/REC-ttml-imsc1-20160421/).
- [TTML1 timing and styles](https://www.w3.org/TR/ttml1/).
- Local Shaka/hls.js parsers and dash.js/RxPlayer subtitle fixtures are useful
  comparison targets. Normal test runs need none of these checkouts or a browser.

The named WebVTT entity table is WHATWG HTML data from Python's standard library;
the reader itself is Rust-only. No serializer dependency or FFI is introduced.
