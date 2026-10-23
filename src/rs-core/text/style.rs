//! Checked style values used while preparing the shared CSS tree.
use std::{collections::BTreeMap, sync::Arc};
pub(crate) mod syntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Red, green, blue and alpha, each from 0 to 255. Alpha 0 is transparent.
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Pixels in the document's coordinate system.
    Pixel,
    /// A percentage; the property decides what 100% refers to.
    Percent,
    /// A multiple of the applicable font size.
    Em,
    /// A fraction of the video image height, resolved before rendering.
    ViewportHeight,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    pub value: f64,
    pub unit: Unit,
}

#[derive(Debug, Clone, PartialEq)]
/// A border around text glyphs. A missing color uses the text color.
pub struct Outline {
    pub color: Option<Color>,
    pub thickness: Length,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Color(Color),
    /// One or more lengths. Their order and allowed count depend on the property.
    Lengths(Vec<Length>),
    Number(f64),
    Outline(Outline),
    /// A named value checked by the format parser. Its meaning depends on the property.
    Keyword(Arc<str>),
    /// A text value, such as a font-family list. Text is shared when styles are
    /// combined, so copying a map does not copy a large font-family string.
    Text(Arc<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    Color,
    BackgroundColor,
    FontFamily,
    FontSize,
    FontStyle,
    FontWeight,
    TextDecoration,
    TextAlign,
    DisplayAlign,
    WritingMode,
    Direction,
    UnicodeBidi,
    LineHeight,
    Opacity,
    Origin,
    Extent,
    Padding,
    Display,
    Visibility,
    Overflow,
    TextOutline,
    WrapOption,
    ZIndex,
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Specified or computed properties used internally by both parsers.
pub struct Style {
    pub properties: BTreeMap<Property, Value>,
}
impl Style {
    pub fn overlay(&mut self, other: &Self) {
        self.properties.extend(other.properties.clone());
    }
}

pub fn length(s: &str) -> Option<Length> {
    [("px", Unit::Pixel), ("%", Unit::Percent), ("em", Unit::Em)]
        .into_iter()
        .find_map(|(suffix, unit)| {
            let value = s.strip_suffix(suffix)?;
            let n = syntax::number(value, false)?;
            n.is_finite().then_some(Length { value: n, unit })
        })
}
pub fn color(s: &str) -> Option<Color> {
    if let Some(body) = s.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let channels: Option<Vec<u8>> = body
            .split(',')
            .take(5)
            .map(|v| {
                let s = v.trim_matches(|c: char| c.is_ascii_whitespace());
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    None
                } else {
                    s.parse().ok()
                }
            })
            .collect();
        let c = channels?;
        if c.len() != 3 {
            return None;
        }
        return Some(Color(c[0], c[1], c[2], 255));
    }
    if let Some(body) = s.strip_prefix("rgba(").and_then(|v| v.strip_suffix(')')) {
        let channels: Option<Vec<u8>> = body
            .split(',')
            .take(5)
            .map(|v| {
                let s = v.trim_matches(|c: char| c.is_ascii_whitespace());
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    None
                } else {
                    s.parse().ok()
                }
            })
            .collect();
        let c = channels?;
        if c.len() != 4 {
            return None;
        }
        return Some(Color(c[0], c[1], c[2], c[3]));
    }
    if let Some(hex) = s.strip_prefix('#') {
        if (hex.len() != 6 && hex.len() != 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let n = u32::from_str_radix(hex, 16).ok()?;
        return Some(if hex.len() == 6 {
            Color((n >> 16) as u8, (n >> 8) as u8, n as u8, 255)
        } else {
            Color((n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8)
        });
    }
    if s.len() > 16 {
        return None;
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "transparent" => Color(0, 0, 0, 0),
        "black" => Color(0, 0, 0, 255),
        "silver" => Color(192, 192, 192, 255),
        "gray" => Color(128, 128, 128, 255),
        "white" => Color(255, 255, 255, 255),
        "maroon" => Color(128, 0, 0, 255),
        "red" => Color(255, 0, 0, 255),
        "purple" => Color(128, 0, 128, 255),
        "fuchsia" | "magenta" => Color(255, 0, 255, 255),
        "green" => Color(0, 128, 0, 255),
        "lime" => Color(0, 255, 0, 255),
        "olive" => Color(128, 128, 0, 255),
        "yellow" => Color(255, 255, 0, 255),
        "navy" => Color(0, 0, 128, 255),
        "blue" => Color(0, 0, 255, 255),
        "teal" => Color(0, 128, 128, 255),
        "aqua" | "cyan" => Color(0, 255, 255, 255),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_forms_and_invalid_values() {
        for (s, c) in [
            ("red", Color(255, 0, 0, 255)),
            ("#123456", Color(18, 52, 86, 255)),
            ("#12345678", Color(18, 52, 86, 120)),
            ("rgb(1, 2, 3)", Color(1, 2, 3, 255)),
            ("rgba(1,2,3,128)", Color(1, 2, 3, 128)),
            ("transparent", Color(0, 0, 0, 0)),
        ] {
            assert_eq!(color(s), Some(c), "{s}");
        }
        for s in [
            "#12345",
            "#123456789",
            "#zzzzzz",
            "rgb(256,0,0)",
            "rgba(1,2,3,0.5)",
            "url(x)",
        ] {
            assert_eq!(color(s), None, "{s}");
        }
    }
    #[test]
    fn length_units_and_finite_values() {
        for (s, u) in [
            ("12px", Unit::Pixel),
            ("1.5em", Unit::Em),
            ("50%", Unit::Percent),
        ] {
            assert_eq!(length(s).unwrap().unit, u);
        }
        for s in ["NaNpx", "inf%", "1e3px", "12", "px", "--1px"] {
            assert!(length(s).is_none(), "{s}");
        }
    }
}
