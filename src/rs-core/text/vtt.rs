//! WebVTT documents and HLS subtitle segments, independent of playback and DOM.
mod css;
mod entities;
mod model;
mod parser;
mod render;
mod settings;
mod style_values;
mod text;
mod timestamp;
use super::{budget::Budget, Document, Error, Limits, Timeline};
pub use model::Header;
pub(crate) use model::*;
pub(crate) use timestamp::timestamp;

/// An initialization header is parser state, never renderer input.
pub fn parse_init(bytes: &[u8]) -> Result<Header, Error> {
    Ok(parser::read(
        bytes,
        None,
        Limits::default(),
        &mut Budget::new(Limits::default()),
    )?
    .header)
}
pub fn parse(bytes: &[u8], initialization: Option<&Header>) -> Result<Document, Error> {
    parse_with_limits(bytes, initialization, Limits::default())
}
pub fn parse_with_limits(
    bytes: &[u8],
    initialization: Option<&Header>,
    limits: Limits,
) -> Result<Document, Error> {
    parse_with_timeline(bytes, initialization, Timeline::default(), limits)
}
pub fn parse_with_timeline(
    bytes: &[u8],
    initialization: Option<&Header>,
    timeline: Timeline,
    limits: Limits,
) -> Result<Document, Error> {
    let mut budget = Budget::new(limits);
    let source = parser::read(bytes, initialization, limits, &mut budget)?;
    render::prepare(source, timeline, &mut budget)
}

#[cfg(test)]
mod tests;
