//! Owned subtitle documents, independent of playback, host bindings and rendering.
//!
//! Parsers retain source timing. The caller supplies the A/V anchor when mapping
//! WebVTT MPEG timestamps, and owns discontinuity and selection state.
//! XXX TODO: Validate identified subtitle data against the supported HLS formats: WebVTT and
//! fMP4 IMSC1 Text Profile. Probe unknown declarations before rejecting the format.
//! Report confirmed unsupported formats through the public API: warning or dedicated error?
//! Audio/video use NoSupportedVariant or CantPlayType; define the subtitle equivalent without
//! rejecting otherwise playable audio/video variants.
//!
//! TODO(subtitles): parse WebVTT directly from UTF-8 bytes, including EXT-X-MAP
//! headers, cue settings, regions, styles and cue-text markup. Preserve full cue
//! intervals and distinguish empty segments from malformed input.
//!
//! TODO(subtitles): normalize X-TIMESTAMP-MAP (default LOCAL=0/MPEGTS=0), 90 kHz
//! media time, 33-bit wraparound and discontinuities using the actual A/V timeline.
//! Playlist estimates are not sufficient to establish cue timestamps.
//!
//! TODO(subtitles): define owned, backend-independent cue batches with segment
//! coverage, content/selection generation and structured formatting. Preserve
//! WebVTT markup; support an IMSC tree/styles/regions model rather than flattening
//! all formats to plain text. Add fMP4 IMSC initialization/sample parsing separately.
//!
//! TODO(subtitles): batch the browser ABI transfer, release downloaded bytes once
//! parsed and bound pending/installed cue retention. The native backend should
//! consume the same model without browser or WASM dependencies.
mod budget;
pub mod imsc;
pub mod mp4;
pub mod style;
pub mod vtt;

/// Formats recognized by the Rust text backend.
/// TODO: Keep this whitelist aligned with the implemented parsers and supported profiles.
/// XXX TODO: High risk of false negatives?
pub(crate) fn supports_codec(codecs: &str) -> bool {
    !codecs.trim().is_empty()
        && codecs
            .split(',')
            .all(|codec| matches!(codec.trim().split('.').next(), Some("wvtt" | "stpp")))
}

/// Fatal errors mean no document can be used. Recoverable WebVTT cue errors are
/// returned alongside the successfully parsed cues instead.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid subtitle header: {0}")]
    Header(String),
    #[error("invalid subtitle data: {0}")]
    Invalid(String),
    #[error("unsupported subtitle feature: {0}")]
    Unsupported(String),
    #[error("subtitle resource limit exceeded: {0}")]
    Limit(&'static str),
}

/// Explicit bounds apply before allocating parser trees or descending recursively.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest input document, in bytes.
    pub bytes: usize,
    /// Estimated allocation allowance for output and temporary copies, in bytes.
    /// This is a counter, not a reserved arena or a hard heap limit. Checks happen
    /// before allocation; reservations are not refunded during a parse.
    pub output_bytes: usize,
    /// Maximum tree entries, style values, CSS rules, selectors and declarations.
    pub nodes: usize,
    /// Maximum XML, markup, CSS block and style-reference depth.
    pub depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 16 * 1024 * 1024,
            output_bytes: 32 * 1024 * 1024,
            nodes: 100_000,
            depth: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// One-based source line when available.
    pub line: Option<usize>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn codec_whitelist_handles_profiles_and_rejects_unknown_formats() {
        for codec in [
            "wvtt",
            "stpp",
            "stpp.ttml.im1t",
            "stpp.ttml.im1i",
            "wvtt, stpp",
        ] {
            assert!(super::supports_codec(codec), "{codec}");
        }
        for codec in ["", " ", "wvtt,", "wvtt,unknown", "avc1.42E01E", "unknown"] {
            assert!(!super::supports_codec(codec), "{codec}");
        }
    }
}
