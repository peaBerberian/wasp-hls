//! Style values that a browser or another renderer can read.
//! These are the values written by the author. Relative units are kept because
//! the parser does not know the viewport size, font metrics or parent layout.
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
    /// TTML cells, based on the document's column and row counts.
    Cell,
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
    /// Optional blur radius. None means no blur was written.
    pub blur: Option<Length>,
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
    ShowBackground,
    TextOutline,
    WrapOption,
    ZIndex,
}

/// An invalid or unsupported value kept as source text. It is not an active
/// shared style value. The namespace avoids confusing a
/// future extension with a TTML property that happens to have the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub namespace: Arc<str>,
    pub name: Arc<str>,
    pub value: Arc<str>,
}
#[derive(Debug, Clone, Default, PartialEq)]
/// Values read from one style or a combination of referenced styles. This does
/// not contain inherited values or layout results.
pub struct Style {
    pub properties: BTreeMap<Property, Value>,
    pub unsupported: Vec<Declaration>,
}
impl Style {
    pub fn overlay(&mut self, other: &Self) {
        self.properties.extend(other.properties.clone());
        for declaration in &other.unsupported {
            if let Some(existing) = self
                .unsupported
                .iter_mut()
                .find(|d| d.namespace == declaration.namespace && d.name == declaration.name)
            {
                *existing = declaration.clone();
            } else {
                self.unsupported.push(declaration.clone());
            }
        }
    }
}

pub fn length(s: &str) -> Option<Length> {
    [
        ("px", Unit::Pixel),
        ("%", Unit::Percent),
        ("em", Unit::Em),
        ("c", Unit::Cell),
    ]
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
            ("2c", Unit::Cell),
        ] {
            assert_eq!(length(s).unwrap().unit, u);
        }
        for s in ["NaNpx", "inf%", "1e3px", "12", "px", "--1px"] {
            assert!(length(s).is_none(), "{s}");
        }
    }
    #[test]
    fn extension_overlay_remains_bounded() {
        let mut s = Style::default();
        s.unsupported.push(Declaration {
            namespace: "urn:test".into(),
            name: "effect".into(),
            value: "a".into(),
        });
        for _ in 0..100 {
            let other = s.clone();
            s.overlay(&other);
        }
        assert_eq!(s.unsupported.len(), 1);
    }
}
