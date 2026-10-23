//! Each text property has a fixed type. There are no CSS expressions or variables.
use std::sync::Arc;
/// Red, green, blue and alpha. Each value is between 0 and 255.
pub type Color = [u8; 4];
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Pixels(f64),
    /// A fraction of the video image height. It does not use the browser window.
    VideoHeight(f64),
    /// Em uses the font size. Percent uses the parent box size.
    Em(f64),
    Percent(f64),
}
impl Length {
    /// Calculate the size using the video, font and parent box sizes.
    pub fn resolve(self, video_height: f64, font_size: f64, box_size: f64) -> f64 {
        match self {
            Self::Pixels(n) => n,
            Self::VideoHeight(n) => n * video_height,
            Self::Em(n) => n * font_size,
            Self::Percent(n) => n * box_size / 100.0,
        }
    }
    pub(crate) fn relative(self, font: Self, pixels: Option<f64>) -> Self {
        match self {
            Self::Em(n) => font.scale(n),
            Self::Percent(n) => font.scale(n / 100.0),
            Self::Pixels(n) if pixels.is_some_and(|p| (n / p).is_finite()) => {
                Self::VideoHeight(n / pixels.unwrap())
            }
            _ => self,
        }
    }
    pub(crate) fn scale(self, factor: f64) -> Self {
        match self {
            Self::Pixels(n) if (n * factor).is_finite() => Self::Pixels(n * factor),
            Self::VideoHeight(n) if (n * factor).is_finite() => Self::VideoHeight(n * factor),
            Self::Em(n) if (n * factor).is_finite() => Self::Em(n * factor),
            Self::Percent(n) if (n * factor).is_finite() => Self::Percent(n * factor),
            _ => self,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Decoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum LineHeight {
    #[default]
    Normal,
    Multiple(f64),
    Length(Length),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily {
    pub name: String,
    /// CSS must not quote generic names such as sans-serif.
    pub generic: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub color: Color,
    pub background: Color,
    /// Many nodes use the same font names. Share the list to avoid copying
    /// long names. None means the renderer chooses its default font.
    pub font_family: Option<Arc<[FontFamily]>>,
    pub font_size: Length,
    pub font_style: FontStyle,
    pub font_weight: f64,
    pub decoration: Decoration,
    pub line_height: LineHeight,
    pub wrap: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            color: [255; 4],
            background: [0; 4],
            font_family: None,
            font_size: Length::VideoHeight(1.0 / 15.0),
            font_style: FontStyle::Normal,
            font_weight: 400.0,
            decoration: Decoration::default(),
            line_height: LineHeight::Normal,
            wrap: true,
        }
    }
}
// For CSS, calculate pixel sizes again when the video size changes.
// Apply decorations only to nodes with text, so a child can remove an underline.
// TODO: Send these values to JS as arrays, without CSS or a call for each field.
