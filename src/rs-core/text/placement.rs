//! Positions and sizes use numbers and units, without CSS layout rules.
use super::Length;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
    Left,
    Right,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WritingMode {
    #[default]
    Horizontal,
    VerticalRl,
    VerticalLr,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub offset: Length,
    /// Measure from the bottom or right edge of the parent box.
    pub from_end: bool,
    /// Subtract this fraction of the measured text size from the position.
    pub fraction: f64,
}
impl Default for Anchor {
    fn default() -> Self {
        Self {
            offset: Length::Pixels(0.0),
            from_end: false,
            fraction: 0.0,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub x: Anchor,
    pub y: Anchor,
    /// None means use the text size. Percentages use the parent region size.
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub content_align: Align,
    /// Top, right, bottom, left. The renderer applies these inside the region.
    pub padding: [Length; 4],
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            x: Anchor::default(),
            y: Anchor::default(),
            width: Some(Length::Percent(100.0)),
            height: Some(Length::Percent(100.0)),
            content_align: Align::End,
            padding: [Length::Pixels(0.0); 4],
        }
    }
}
impl Anchor {
    pub fn resolve(self, video_height: f64, font_size: f64, container: f64, measured: f64) -> f64 {
        let offset = self.offset.resolve(video_height, font_size, container);
        (if self.from_end {
            container - offset
        } else {
            offset
        }) - measured * self.fraction
    }
}
// A browser backend can measure the text and calculate its position in pixels.
// It does not need flexbox or CSS transforms to use these values.
// TODO: Add these positions and sizes to the arrays sent to JS.
