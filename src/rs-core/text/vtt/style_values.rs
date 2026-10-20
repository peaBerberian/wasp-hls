//! Check supported CSS property values without using TTML's grammar. Valid CSS
//! outside this subset stays in the raw declaration; it is not guessed at here.
use crate::text::style::{color, syntax, Color, Length, Property, Unit, Value};

pub(super) fn parse(name: &str, source: &str) -> Option<(Property, Value)> {
    use Property::*;
    let s = source.trim_matches(|c: char| c.is_ascii_whitespace());
    if name == "font-family" {
        return syntax::family_list(s, true).then(|| (FontFamily, Value::Text(s.into())));
    }
    // CSS keywords are case-insensitive. Font-family names above keep their case.
    let lower = s.to_ascii_lowercase();
    let s = lower.as_str();
    let result = match name {
        "color" | "background-color" => (
            if name == "color" {
                Color
            } else {
                BackgroundColor
            },
            Value::Color(css_color(s)?),
        ),
        "font-size" => {
            let keywords = [
                "xx-small",
                "x-small",
                "small",
                "medium",
                "large",
                "x-large",
                "xx-large",
                "xxx-large",
                "larger",
                "smaller",
            ];
            let value = if keywords.contains(&s) {
                Value::Keyword(s.into())
            } else {
                Value::Lengths(vec![css_length(s)?])
            };
            (FontSize, value)
        }
        "font-weight" => {
            let value = if ["normal", "bold", "bolder", "lighter"].contains(&s) {
                Value::Keyword(s.into())
            } else {
                let n = syntax::number(s, true)?;
                if !(1.0..=1000.0).contains(&n) {
                    return None;
                }
                Value::Number(n)
            };
            (FontWeight, value)
        }
        "font-style" if ["normal", "italic", "oblique"].contains(&s) => {
            (FontStyle, Value::Keyword(s.into()))
        }
        "visibility" if ["visible", "hidden", "collapse"].contains(&s) => {
            (Visibility, Value::Keyword(s.into()))
        }
        "line-height" => {
            let value = if s == "normal" {
                Value::Keyword(s.into())
            } else if let Some(n) = syntax::number(s, true) {
                if n < 0.0 {
                    return None;
                }
                Value::Number(n)
            } else {
                Value::Lengths(vec![css_length(s)?])
            };
            (LineHeight, value)
        }
        "opacity" => {
            let n = if let Some(s) = s.strip_suffix('%') {
                syntax::number(s, true)? / 100.0
            } else {
                syntax::number(s, true)?
            };
            (Opacity, Value::Number(n.clamp(0.0, 1.0)))
        }
        // This subset handles only decoration lines. Shorthand colors, line
        // styles and thicknesses remain raw until the model can represent them.
        "text-decoration" | "text-decoration-line" => {
            if !decoration(s) {
                return None;
            }
            (TextDecoration, Value::Keyword(s.into()))
        }
        _ => return None,
    };
    Some(result)
}
fn css_length(s: &str) -> Option<Length> {
    for (suffix, unit) in [("px", Unit::Pixel), ("em", Unit::Em), ("%", Unit::Percent)] {
        if let Some(number) = s.strip_suffix(suffix) {
            let value = syntax::number(number, true)?;
            return (value >= 0.0).then_some(Length { value, unit });
        }
    }
    // CSS allows a unitless zero for a length. Nonzero unitless values are
    // valid for line-height numbers, but not for font-size lengths.
    (syntax::number(s, true)? == 0.0).then_some(Length {
        value: 0.0,
        unit: Unit::Pixel,
    })
}
fn decoration(s: &str) -> bool {
    if s == "none" {
        return true;
    }
    let mut seen = 0u8;
    for word in s.split_ascii_whitespace() {
        let bit = match word {
            "underline" => 1,
            "overline" => 2,
            "line-through" => 4,
            _ => return false,
        };
        if seen & bit != 0 {
            return false;
        }
        seen |= bit;
    }
    seen != 0
}
fn css_color(s: &str) -> Option<Color> {
    let rgba = s.starts_with("rgba(");
    if rgba || s.starts_with("rgb(") {
        let open = if rgba { 5 } else { 4 };
        let body = s[open..].strip_suffix(')')?;
        let mut parts = body.split(',').map(str::trim);
        let channels = [parts.next()?, parts.next()?, parts.next()?];
        // The comma syntax requires either three numbers or three percentages.
        if channels.iter().any(|c| c.ends_with('%')) && !channels.iter().all(|c| c.ends_with('%')) {
            return None;
        }
        let mut rgb = [0; 3];
        for (out, input) in rgb.iter_mut().zip(channels) {
            let n = if let Some(percent) = input.strip_suffix('%') {
                syntax::number(percent, true)?.clamp(0.0, 100.0) * 255.0 / 100.0
            } else {
                syntax::number(input, true)?.clamp(0.0, 255.0)
            };
            *out = n.round() as u8;
        }
        let alpha = if rgba {
            let input = parts.next()?;
            let n = if let Some(percent) = input.strip_suffix('%') {
                syntax::number(percent, true)? / 100.0
            } else {
                syntax::number(input, true)?
            };
            (n.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            255
        };
        if parts.next().is_some() {
            return None;
        }
        return Some(Color(rgb[0], rgb[1], rgb[2], alpha));
    }
    if let Some(hex) = s.strip_prefix('#') {
        if (hex.len() == 3 || hex.len() == 4) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            let expanded = hex.chars().flat_map(|c| [c, c]).collect::<String>();
            return color(&format!("#{expanded}"));
        }
    }
    // The shared reader handles the supported named and hexadecimal colors.
    // Function syntax was handled above, where CSS alpha differs from TTML.
    color(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn css_size_takes_one_value_and_never_ttml_cells() {
        for value in ["12px", "1.5em", "120%", "1e2px", "0", "MEDIUM", "larger"] {
            assert!(parse("font-size", value).is_some(), "{value}");
        }
        for value in [
            "10px 20px",
            "1c",
            "-1px",
            "2",
            "1.px",
            "bogus",
            "NaNpx",
            "12px junk",
        ] {
            assert!(parse("font-size", value).is_none(), "{value}");
        }
    }
    #[test]
    fn font_keywords_and_weights_have_known_values() {
        for value in [
            "normal", "BOLD", "bolder", "lighter", "100", "450.5", "1000",
        ] {
            assert!(parse("font-weight", value).is_some(), "{value}");
        }
        for value in ["bogus", "0", "1001", "-1", "NaN", "700 junk"] {
            assert!(parse("font-weight", value).is_none(), "{value}");
        }
        for value in ["normal", "ITALIC", "oblique"] {
            assert!(parse("font-style", value).is_some());
        }
        assert!(parse("font-style", "bold").is_none());
        assert!(parse("visibility", "collapse").is_some());
        assert!(parse("visibility", "none").is_none());
    }
    #[test]
    fn line_height_numbers_are_not_lengths() {
        assert_eq!(
            parse("line-height", "1.5"),
            Some((Property::LineHeight, Value::Number(1.5)))
        );
        for value in ["normal", "120%", "20px", "0"] {
            assert!(parse("line-height", value).is_some());
        }
        for value in ["-1", "1c", "10px 20px", "NaN"] {
            assert!(parse("line-height", value).is_none());
        }
    }
    #[test]
    fn css_decoration_rejects_ttml_words_and_duplicates() {
        for value in ["none", "underline overline", "LINE-THROUGH"] {
            assert!(parse("text-decoration", value).is_some());
        }
        for value in [
            "noUnderline",
            "lineThrough",
            "none underline",
            "underline underline",
            "underline red",
        ] {
            assert!(parse("text-decoration", value).is_none(), "{value}");
        }
    }
    #[test]
    fn color_uses_css_alpha_and_clamps_components() {
        assert_eq!(
            parse("color", "rgba(1,2,3,.5)"),
            Some((Property::Color, Value::Color(Color(1, 2, 3, 128))))
        );
        assert_eq!(
            parse("color", "rgb(100%,0%,50%)"),
            Some((Property::Color, Value::Color(Color(255, 0, 128, 255))))
        );
        assert_eq!(
            parse("color", "rgba(999,-1,3,128)"),
            Some((Property::Color, Value::Color(Color(255, 0, 3, 255))))
        );
        for value in [
            "rgb(1%,2,3)",
            "rgb(1,2,3,4)",
            "rgba(1,2,3)",
            "#+12345",
            "#xyf",
            "rgb(NaN,0,0)",
        ] {
            assert!(parse("color", value).is_none(), "{value}");
        }
    }
    #[test]
    fn css_family_list_and_opacity_rules() {
        for value in ["Arial, sans-serif", "'A,B', serif", "Noto Sans"] {
            assert!(parse("font-family", value).is_some());
        }
        for value in ["Arial,", "inherit", "initial", "font()"] {
            assert!(parse("font-family", value).is_none());
        }
        assert_eq!(
            parse("opacity", "50%"),
            Some((Property::Opacity, Value::Number(0.5)))
        );
        assert_eq!(
            parse("opacity", "2"),
            Some((Property::Opacity, Value::Number(1.0)))
        );
        assert!(parse("opacity", "NaN").is_none());
    }
    #[test]
    fn unsupported_valid_syntax_stays_raw() {
        for (name, value) in [
            ("color", "currentColor"),
            ("font-size", "calc(1em + 1px)"),
            ("font-size", "1rem"),
            ("font-weight", "inherit"),
            ("font-family", r#""escaped\ name""#),
        ] {
            assert!(parse(name, value).is_none(), "{name}={value}");
        }
    }
}
