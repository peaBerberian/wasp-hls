//! Check TTML values before putting them in the shared model. IMSC1 text-profile
//! limits are applied where they can be checked without fonts or screen sizes.
use crate::text::style::{color, length, syntax, Length, Outline, Property, Unit, Value};

pub(super) fn parse(name: &str, source: &str) -> Option<(Property, Value)> {
    use Property::*;
    let s = source.trim_matches(|c: char| c.is_ascii_whitespace());
    let (property, value) = match name {
        "color" => (Color, Value::Color(color(s)?)),
        "backgroundColor" => (BackgroundColor, Value::Color(color(s)?)),
        "fontFamily" => {
            if !syntax::family_list(s, false) {
                return None;
            }
            (
                FontFamily,
                Value::Text(
                    match s {
                        "default" | "sansSerif" | "proportionalSansSerif" => "sans-serif",
                        "monospaceSansSerif" | "monospaceSerif" => "monospace",
                        "proportionalSerif" => "serif",
                        _ => s,
                    }
                    .into(),
                ),
            )
        }
        "fontSize" => {
            let values = lengths(s, 1, 2)?;
            // IMSC1 allows equal horizontal and vertical sizes, but not text
            // stretched by different sizes on the two axes.
            if values.len() == 2 && values[0] != values[1] {
                return None;
            }
            (FontSize, Value::Lengths(values))
        }
        "origin" | "extent" => {
            let values = lengths(s, 2, 2)?;
            if values.iter().any(|v| v.unit == Unit::Em) {
                return None;
            }
            (
                if name == "origin" { Origin } else { Extent },
                Value::Lengths(values),
            )
        }
        "padding" => (Padding, Value::Lengths(lengths(s, 1, 4)?)),
        "lineHeight" if s != "normal" => (LineHeight, Value::Lengths(lengths(s, 1, 1)?)),
        "textOutline" if s != "none" => (TextOutline, Value::Outline(outline(s)?)),
        "opacity" => {
            // TTML explicitly maps NaN to zero and clamps alpha to [0,1].
            let n = match s {
                "NaN" | "-INF" => 0.0,
                "INF" => 1.0,
                _ => syntax::number(s, true)?.clamp(0.0, 1.0),
            };
            (Opacity, Value::Number(n))
        }
        "zIndex" if s != "auto" => {
            if !s
                .strip_prefix(['+', '-'])
                .unwrap_or(s)
                .bytes()
                .all(|b| b.is_ascii_digit())
            {
                return None;
            }
            (ZIndex, Value::Number(f64::from(s.parse::<i32>().ok()?)))
        }
        "textDecoration" => (TextDecoration, Value::Keyword(decoration(s)?.into())),
        _ => {
            let (p, allowed): (Property, &[&str]) = match name {
                "fontStyle" => (FontStyle, &["normal", "italic", "oblique"]),
                "fontWeight" => (FontWeight, &["normal", "bold"]),
                "textAlign" => (TextAlign, &["left", "center", "right", "start", "end"]),
                "displayAlign" => (DisplayAlign, &["before", "center", "after"]),
                "writingMode" => (
                    WritingMode,
                    &["lrtb", "rltb", "tbrl", "tblr", "lr", "rl", "tb"],
                ),
                "direction" => (Direction, &["ltr", "rtl"]),
                "unicodeBidi" => (UnicodeBidi, &["normal", "embed", "bidiOverride"]),
                "lineHeight" => (LineHeight, &["normal"]),
                "display" => (Display, &["auto", "none"]),
                "visibility" => (Visibility, &["visible", "hidden"]),
                "overflow" => (Overflow, &["visible", "hidden"]),
                "wrapOption" => (WrapOption, &["wrap", "noWrap"]),
                "textOutline" => (TextOutline, &["none"]),
                "zIndex" => (ZIndex, &["auto"]),
                _ => return None,
            };
            if !allowed.contains(&s) {
                return None;
            }
            (p, Value::Keyword(s.into()))
        }
    };
    Some((property, value))
}
fn lengths(s: &str, min: usize, max: usize) -> Option<Vec<Length>> {
    let values: Option<Vec<_>> = s
        .split_ascii_whitespace()
        .take(max + 1)
        .map(length)
        .collect();
    let values = values?;
    // IMSC1 forbids negative lengths. Cell units are reserved for the separate
    // ebutts:linePadding extension, which is ignored by this parser.
    if values.len() < min || values.len() > max || values.iter().any(|v| v.value < 0.0) {
        return None;
    }
    Some(values)
}
fn outline(s: &str) -> Option<Outline> {
    // A color function can contain spaces, so do not split inside its brackets.
    let (color, rest) = if s.starts_with("rgb(") || s.starts_with("rgba(") {
        let end = s.find(')')? + 1;
        if !s[end..].starts_with(|c: char| c.is_ascii_whitespace()) {
            return None;
        }
        (
            Some(color(&s[..end])?),
            s[end..].trim_matches(|c: char| c.is_ascii_whitespace()),
        )
    } else {
        let end = s.find(|c: char| c.is_ascii_whitespace()).unwrap_or(s.len());
        if let Some(c) = color(&s[..end]) {
            (
                Some(c),
                s[end..].trim_matches(|c: char| c.is_ascii_whitespace()),
            )
        } else {
            (None, s)
        }
    };
    let values = lengths(rest, 1, 2)?;
    // A blur radius is prohibited by IMSC1.
    if values.get(1).is_some_and(|v| v.value != 0.0) {
        return None;
    }
    Some(Outline {
        color,
        thickness: values[0],
    })
}
fn decoration(s: &str) -> Option<&str> {
    if s == "none" {
        return Some(s);
    }
    let mut seen = 0u8;
    for word in s.split_ascii_whitespace() {
        let bit = match word {
            "underline" | "noUnderline" => 1,
            "lineThrough" | "noLineThrough" => 2,
            "overline" | "noOverline" => 4,
            _ => return None,
        };
        if seen & bit != 0 {
            return None;
        }
        seen |= bit;
    }
    (seen != 0).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keywords_are_checked_per_property() {
        for (name, value) in [
            ("fontWeight", "bold"),
            ("fontStyle", "italic"),
            ("textAlign", "end"),
            ("displayAlign", "after"),
            ("writingMode", "tbrl"),
            ("direction", "rtl"),
            ("unicodeBidi", "bidiOverride"),
            ("display", "none"),
            ("visibility", "hidden"),
            ("overflow", "visible"),
            ("wrapOption", "noWrap"),
            ("lineHeight", "normal"),
            ("zIndex", "auto"),
        ] {
            assert!(parse(name, value).is_some(), "{name}={value}");
            assert!(parse(name, "bogus").is_none(), "{name}");
        }
        for (name, value) in [
            ("fontWeight", "700"),
            ("fontWeight", "BOLD"),
            ("unicodeBidi", "bidi-override"),
            ("display", "block"),
            ("textAlign", "justify"),
        ] {
            assert!(parse(name, value).is_none(), "{name}={value}");
        }
    }
    #[test]
    fn lengths_check_count_sign_units_and_profile_rules() {
        for (name, value) in [
            ("fontSize", "12px"),
            ("fontSize", "12px 12px"),
            ("fontSize", "1.5em"),
            ("origin", "10% 20%"),
            ("extent", "640px 360px"),
            ("padding", "1px 2px 3px 4px"),
            ("lineHeight", "120%"),
        ] {
            assert!(parse(name, value).is_some(), "{name}={value}");
        }
        for (name, value) in [
            ("fontSize", "10px 20px"),
            ("fontSize", "1c"),
            ("fontSize", "-1px"),
            ("fontSize", "1e2px"),
            ("fontSize", "1.px"),
            ("origin", "1px"),
            ("origin", "1em 2em"),
            ("extent", "1px 2px 3px"),
            ("padding", "0"),
            ("padding", "1px 2px 3px 4px 5px"),
            ("lineHeight", "1.5"),
        ] {
            assert!(parse(name, value).is_none(), "{name}={value}");
        }
    }
    #[test]
    fn ttml_decoration_has_its_own_words_and_exclusions() {
        for value in [
            "none",
            "noUnderline lineThrough",
            "underline overline noLineThrough",
        ] {
            assert!(parse("textDecoration", value).is_some(), "{value}");
        }
        for value in [
            "",
            "line-through",
            "none underline",
            "underline underline",
            "underline noUnderline",
            "noLineThrough lineThrough",
        ] {
            assert!(parse("textDecoration", value).is_none(), "{value}");
        }
    }
    #[test]
    fn outline_checks_blur_and_reads_color_functions() {
        assert!(matches!(
            parse("textOutline", "red 2px").unwrap().1,
            Value::Outline(_)
        ));
        assert!(parse("textOutline", "rgb(1, 2, 3) 2px").is_some());
        assert!(parse("textOutline", "2px 0px").is_some());
        for value in ["red -2px", "red 2px 1px", "2px 1px 0px", "bogus 2px"] {
            assert!(parse("textOutline", value).is_none(), "{value}");
        }
    }
    #[test]
    fn alpha_uses_ttml_rules() {
        for (value, expected) in [
            ("NaN", 0.0),
            ("INF", 1.0),
            ("-INF", 0.0),
            ("2", 1.0),
            ("-1", 0.0),
            ("5e-1", 0.5),
        ] {
            assert_eq!(
                parse("opacity", value),
                Some((Property::Opacity, Value::Number(expected)))
            );
        }
        for value in ["inf", "infinity", "50%", "bogus"] {
            assert!(parse("opacity", value).is_none());
        }
    }
    #[test]
    fn font_names_need_a_complete_valid_list() {
        for value in [
            "default",
            "Arial, proportionalSansSerif",
            "'A, B', serif",
            "Noto Sans",
        ] {
            assert!(parse("fontFamily", value).is_some(), "{value}");
        }
        for value in [
            "",
            "Arial,",
            "Arial,,serif",
            "'unfinished",
            "'A' junk",
            "font()",
        ] {
            assert!(parse("fontFamily", value).is_none(), "{value}");
        }
    }
}
