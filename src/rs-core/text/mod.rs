//! Backend-independent subtitle segment parsing (implementation pending).
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

/// Formats recognized by the Rust text backend.
/// TODO: Keep this whitelist aligned with the implemented parsers and supported profiles.
/// XXX TODO: High risk of false negatives?
pub(crate) fn supports_codec(codecs: &str) -> bool {
    !codecs.trim().is_empty()
        && codecs
            .split(',')
            .all(|codec| matches!(codec.trim().split('.').next(), Some("wvtt" | "stpp")))
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
