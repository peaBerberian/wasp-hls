//! WebVTT documents and HLS subtitle segments, independent of playback and DOM.
pub mod css;
mod entities;
mod model;
mod parser;
mod settings;
mod style_values;
mod text;
mod timestamp;
pub use model::*;
pub use parser::{parse, parse_with_limits};
pub use timestamp::timestamp;

#[cfg(test)]
mod tests;
