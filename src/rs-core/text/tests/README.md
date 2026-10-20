# Subtitle parser tests

- Run fixture tests: `cargo test text::tests::fixture_tests` (native Rust only).
- Run all subtitle tests: `cargo test text::`.
- The runner is `fixture_tests.rs`; its inputs and expected results are in `fixtures/`.
- 28 authored fixtures have complete parser-output `.expected` snapshots.
- Unit tests cover times, cue settings, error recovery, styles, XML namespaces, limits, truncated input, timestamp wraparound and MP4 sample boundaries.
- Memory tests check that repeated references share styles and that combined styles share large text values.
- Budget tests cover XML/style output, external headers, CSS rules, selectors, property values and nested blocks.
- A scanner-work counter checks that CSS work grows with input size. It does not depend on machine speed.
- Hidden-style tests cover foreign XML, metadata, ignored duplicate IDs and ignored style cycles.
- No JavaScript reference parser, player checkout, browser or snapshot-update command is needed by these tests.

Development comparison, 2026-10-20:

| Reference checkout  | Values compared                                                    | Inputs | Matches |
| ------------------- | ------------------------------------------------------------------ | -----: | ------: |
| `vtt.js` `42ca104`  | Cue IDs, millisecond intervals, raw text, alignment, size and line |    340 |     333 |
| `imscJS` `6aef62f6` | Content-node kinds, active time ranges, color and font weight      |    141 |     140 |

- Inputs included authored fixtures, generated timing/style matrices and the available reference test files. External inputs were not copied into this suite.
- Normalization maps legacy `middle` to `center`, equates negative zero with zero, clips reference IMSC intervals to ancestor activity and treats fatal-header errors as equivalent.
- WebVTT differences: recovery without a separator/after a malformed arrow; cues immediately after the header; invalid two-component hour timestamps; obsolete `middle` and invalid line alignment. Regression tests follow current WebVTT parsing rules.
- IMSC difference: one SMPTE image metadata fixture. Foreign image content is diagnosed/ignored by this text-profile parser.
- Additional unit tests cover differences outside that projection: HTML character references, repeated settings, fractional lines, markup, CSS syntax and region animation.

Specification baseline:

- HLS 2026 §§3.1.2, 3.1.4, 3.1.5 and referenced editions.
- [WebVTT, 20 May 2026](https://www.w3.org/TR/2026/CRD-webvtt1-20260520/), especially §6.
- [WebVTT metadata headers, December 2015](https://www.w3.org/TR/2015/WD-webvtt1-20151208/#webvtt-metadata-header).
- [IMSC1 text profile, April 2016](https://www.w3.org/TR/2016/REC-ttml-imsc1-20160421/), with TTML timing/styling semantics.
- HLS names ISO/IEC 14496-12:2015 and 14496-30:2014 for MP4 carriage. Their full normative texts were not available during this task; extraction was cross-checked against local player code and fixtures.

The WebVTT named-reference table is WHATWG HTML data from Python's standard-library `html.entities.html5`; lookup/tokenization is Rust-only.

Memory check after the fixes (native debug build, temporary allocator probe):

- 1,000 references to one 10 KB font-family style: about 1.0 MB peak heap use during parsing.
- 1,000 references to a two-style combination: about 1.6 MB peak heap use during parsing.
- These are measurements of those inputs, not guaranteed WASM memory limits.

Style-value checks:

- CSS and TTML tests use separate tables of accepted and rejected values.
- Tests cover font weight, size counts, length units, decoration words, family lists, color alpha, opacity and line height.
- Full parser tests check that invalid values remain raw and do not remove readable cue text or replace valid referenced values.
- The region-set snapshot keeps its blurred outline as raw text: IMSC1 prohibits blurred outlines.
- Property rules were checked against [TTML1 style rules](https://www.w3.org/TR/ttml1/#styling-attribute-vocabulary), [IMSC1 text-profile limits](https://www.w3.org/TR/2016/REC-ttml-imsc1-20160421/#text-profile-constraints), [CSS Fonts](https://www.w3.org/TR/css-fonts-4/) and [CSS text decoration](https://www.w3.org/TR/css-text-decor-3/).
