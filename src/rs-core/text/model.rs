//! Both parsers return the same text structure.
use super::{budget::Budget, Diagnostic, Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    Paragraph,
    Span,
    Break,
}
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Node {
    /// The parent has a lower index than its children. None means no parent.
    pub parent: Option<usize>,
    pub kind: Kind,
    /// Times in seconds during playback. The node is active at start, not at end.
    /// Infinity means that the end time is unknown.
    pub start: f64,
    pub end: f64,
    /// Text after reading character references. A Break node starts a new line.
    pub text: Option<String>,
}
impl Node {
    pub(super) fn new(parent: Option<usize>, kind: Kind, start: f64, end: f64) -> Self {
        Self {
            parent,
            kind,
            start,
            end,
            text: None,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Document {
    pub(super) fn push(&mut self, node: Node, budget: &mut Budget) -> Result<usize, Error> {
        budget.nodes(1)?;
        budget.bytes(std::mem::size_of::<Node>() * 2)?;
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }
}

/// A matching media timestamp and playback position, in seconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timeline {
    pub media_time: f64,
    pub playback_time: f64,
}
impl Timeline {
    pub(super) fn offset(self) -> Result<f64, Error> {
        let offset = self.playback_time - self.media_time;
        if self.media_time.is_finite() && self.playback_time.is_finite() && offset.is_finite() {
            Ok(offset)
        } else {
            Err(Error::Invalid("non-finite subtitle timeline".into()))
        }
    }
}
// TODO: Send each document to JS as arrays of nodes, times and UTF-8 text.
// JS must copy the data before Rust frees it. Native code can use Document directly.
// TODO: Add styles from the subtitle file. Keep the same parser functions.
