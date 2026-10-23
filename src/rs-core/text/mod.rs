//! Text subtitles with one shared output and no host bindings.
mod budget;
pub mod imsc;
mod model;
pub mod mp4;
pub mod vtt;
pub use model::{Document, Kind, Node, Timeline};
mod appearance;
mod properties;
mod syntax;
pub use appearance::*;
mod placement;
pub use placement::*;
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
mod tests;
