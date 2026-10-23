//! Style declarations used while parsing. Each property accepts one value type.
use super::{budget::Budget, *};
use std::{mem::discriminant, sync::Arc};
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Property {
    Color(Color),
    Background(Color),
    Family(Arc<[FontFamily]>),
    Size(Length),
    FontStyle(FontStyle),
    Weight(f64),
    RelativeWeight(bool),
    Decoration(u8, u8),
    LineHeight(LineHeight),
    Wrap(bool),
    Ruby(Kind),
    Origin([Length; 2]),
    Extent([Length; 2]),
    Padding([Length; 4]),
    TextAlign(TextAlign),
    BlockAlign(Align),
    Direction(Direction),
    WritingMode(WritingMode, bool),
}
impl Property {
    pub fn ruby(&self) -> Option<Kind> {
        if let Self::Ruby(n) = self {
            Some(*n)
        } else {
            None
        }
    }
}
pub(crate) fn same(a: &Property, b: &Property) -> bool {
    matches!(
        (a, b),
        (
            Property::Weight(_) | Property::RelativeWeight(_),
            Property::Weight(_) | Property::RelativeWeight(_)
        )
    ) || discriminant(a) == discriminant(b)
}
pub(crate) fn put(out: &mut Vec<Property>, p: Property) {
    // Keep one value for each property. A later declaration replaces the old
    // value. The property list is small, so a linear search is enough.
    if let Some(old) = out.iter_mut().find(|old| same(old, &p)) {
        *old = p;
    } else {
        out.push(p);
    }
}
#[derive(Clone, Default)]
pub(crate) struct Computed {
    pub appearance: Appearance,
    pub placement: Placement,
}
impl Computed {
    pub fn vtt() -> Self {
        let mut out = Self::default();
        out.appearance.font_size = Length::VideoHeight(0.05);
        out.appearance.line_height = LineHeight::Multiple(1.2);
        out.appearance.text_align = TextAlign::Center;
        out
    }
    pub fn child(&self) -> Self {
        let mut out = self.clone();
        out.appearance.background = [0; 4];
        out.placement = Placement::default();
        out
    }
    pub fn apply(&self, local: &[Property], pixels: Option<f64>) -> Self {
        let mut out = self.child();
        // Line height may use the font size. Read the font size first, even if
        // line height appeared first in the subtitle file.
        if let Some(Property::Size(size)) = local.iter().find(|p| matches!(p, Property::Size(_))) {
            out.appearance.font_size = size.relative(self.appearance.font_size, pixels);
        }
        if local.iter().any(|p| p.ruby() == Some(Kind::RubyText))
            && !local.iter().any(|p| matches!(p, Property::Size(_)))
        {
            out.appearance.font_size = self.appearance.font_size.scale(0.5);
        }
        for p in local {
            use Property::*;
            let a = &mut out.appearance;
            match p {
                Color(c) => a.color = *c,
                Background(c) => a.background = *c,
                Family(f) => a.font_family = Some(f.clone()),
                Size(_) | Ruby(_) => {}
                FontStyle(s) => a.font_style = *s,
                Weight(n) => a.font_weight = *n,
                RelativeWeight(bolder) => {
                    // CSS Fonts 4, relative weights: keep very light/heavy values
                    // when a lighter/bolder value does not exist.
                    // https://www.w3.org/TR/css-fonts-4/#relative-weights
                    let weight = a.font_weight;
                    a.font_weight = if *bolder {
                        if weight < 350.0 {
                            400.0
                        } else if weight < 550.0 {
                            700.0
                        } else if weight < 900.0 {
                            900.0
                        } else {
                            weight
                        }
                    } else if weight < 100.0 {
                        weight
                    } else if weight < 550.0 {
                        100.0
                    } else if weight < 750.0 {
                        400.0
                    } else {
                        700.0
                    };
                }
                Decoration(add, remove) => {
                    let old = u8::from(a.decoration.underline)
                        | (u8::from(a.decoration.overline) * 2)
                        | (u8::from(a.decoration.line_through) * 4);
                    let bits = (old & !remove) | add;
                    a.decoration = super::Decoration {
                        underline: bits & 1 != 0,
                        overline: bits & 2 != 0,
                        line_through: bits & 4 != 0,
                    };
                }
                LineHeight(super::LineHeight::Length(l)) => {
                    a.line_height = super::LineHeight::Length(l.relative(a.font_size, pixels))
                }
                LineHeight(v) => a.line_height = *v,
                Wrap(v) => a.wrap = *v,
                Origin(v) => {
                    out.placement.x.offset = v[0];
                    out.placement.y.offset = v[1];
                }
                Extent(v) => {
                    out.placement.width = Some(v[0]);
                    out.placement.height = Some(v[1]);
                }
                Padding(v) => {
                    out.placement.padding = v.map(|l| {
                        if matches!(l, Length::Percent(_)) {
                            l
                        } else {
                            l.relative(a.font_size, pixels)
                        }
                    })
                }
                BlockAlign(v) => out.placement.content_align = *v,
                TextAlign(v) => a.text_align = *v,
                Direction(v) => a.direction = *v,
                WritingMode(v, rtl) => {
                    a.writing_mode = *v;
                    if *rtl && !local.iter().any(|p| matches!(p, Property::Direction(_))) {
                        a.direction = super::Direction::Rtl;
                    }
                }
            }
        }
        out
    }
}

pub(crate) fn parse(
    name: &str,
    source: &str,
    css: bool,
    budget: &mut Budget,
) -> Result<Option<Property>, Error> {
    // CSS and TTML use different property names. Do not accept a CSS name
    // in a TTML attribute.
    let name = if css {
        match name {
            "color" => "color",
            "background-color" => "backgroundColor",
            "font-family" => "fontFamily",
            "font-size" => "fontSize",
            "font-style" => "fontStyle",
            "font-weight" => "fontWeight",
            "line-height" => "lineHeight",
            "text-decoration" | "text-decoration-line" => "textDecoration",
            _ => return Ok(None),
        }
    } else {
        name
    };
    budget.bytes(256 + source.len())?;
    if matches!(name, "fontFamily") {
        budget.bytes(source.bytes().filter(|&b| b == b',').count() * 64)?;
    }
    Ok(read(name, source, css))
}
fn read(name: &str, source: &str, css: bool) -> Option<Property> {
    use Property::*;
    let s = source.trim();
    if name == "fontFamily" {
        return Some(Family(families(s, css)?));
    }
    // Only CSS keywords are case-insensitive. Font names retain their spelling.
    let lower = if css {
        s.to_ascii_lowercase()
    } else {
        String::new()
    };
    let s = if css { lower.as_str() } else { s };
    Some(match name {
        "color" => Color(color(s, css)?),
        "backgroundColor" => Background(color(s, css)?),
        "fontSize" => {
            let values = lengths(s, 1, if css { 1 } else { 2 }, css);
            if let Some(v) = values.filter(|v| v.len() == 1 || v[0] == v[1]) {
                Size(v[0])
            } else if css {
                let scale = match s {
                    "xx-small" => 0.6,
                    "x-small" => 0.75,
                    "small" => 0.89,
                    "medium" => 1.0,
                    "large" => 1.2,
                    "x-large" => 1.5,
                    "xx-large" => 2.0,
                    "xxx-large" => 3.0,
                    "larger" => 1.2,
                    "smaller" => 1.0 / 1.2,
                    _ => return None,
                };
                Size(if matches!(s, "larger" | "smaller") {
                    Length::Em(scale)
                } else {
                    Length::VideoHeight(0.05 * scale)
                })
            } else {
                return None;
            }
        }
        "fontStyle" => FontStyle(match s {
            "normal" => super::FontStyle::Normal,
            "italic" => super::FontStyle::Italic,
            "oblique" => super::FontStyle::Oblique,
            _ => return None,
        }),
        "fontWeight" => match s {
            "normal" => Weight(400.0),
            "bold" => Weight(700.0),
            "bolder" if css => RelativeWeight(true),
            "lighter" if css => RelativeWeight(false),
            _ if css => {
                let n = super::syntax::number(s, true)?;
                if !(1.0..=1000.0).contains(&n) {
                    return None;
                }
                Weight(n)
            }
            _ => return None,
        },
        "lineHeight" => LineHeight(if s == "normal" {
            super::LineHeight::Normal
        } else if css && super::syntax::number(s, true).is_some() {
            let n = super::syntax::number(s, true)?;
            if n < 0.0 {
                return None;
            }
            super::LineHeight::Multiple(n)
        } else {
            super::LineHeight::Length(length(s, css)?)
        }),
        "textDecoration" => {
            let (a, r) = decoration(s, css)?;
            Decoration(a, r)
        }
        "wrapOption" => Wrap(match s {
            "wrap" => true,
            "noWrap" => false,
            _ => return None,
        }),
        "ruby" => Ruby(match s {
            "container" => Kind::Ruby,
            "text" => Kind::RubyText,
            "base" => Kind::Span,
            _ => return None,
        }),
        "origin" | "extent" => {
            let v = lengths(s, 2, 2, false)?;
            if v.iter().any(|l| matches!(l, Length::Em(_))) {
                return None;
            }
            if name == "origin" {
                Origin([v[0], v[1]])
            } else {
                Extent([v[0], v[1]])
            }
        }
        "padding" => {
            let v = lengths(s, 1, 4, false)?;
            Padding(match v.as_slice() {
                [a] => [*a; 4],
                [a, b] => [*a, *b, *a, *b],
                [a, b, c] => [*a, *b, *c, *b],
                [a, b, c, d] => [*a, *b, *c, *d],
                _ => return None,
            })
        }
        "textAlign" => TextAlign(match s {
            "start" => super::TextAlign::Start,
            "center" => super::TextAlign::Center,
            "end" => super::TextAlign::End,
            "left" => super::TextAlign::Left,
            "right" => super::TextAlign::Right,
            _ => return None,
        }),
        "displayAlign" => BlockAlign(match s {
            "before" => Align::Start,
            "center" => Align::Center,
            "after" => Align::End,
            _ => return None,
        }),
        "direction" => Direction(match s {
            "ltr" => super::Direction::Ltr,
            "rtl" => super::Direction::Rtl,
            _ => return None,
        }),
        "writingMode" => WritingMode(
            match s {
                "lrtb" | "rltb" | "lr" | "rl" => super::WritingMode::Horizontal,
                "tbrl" | "tb" => super::WritingMode::VerticalRl,
                "tblr" => super::WritingMode::VerticalLr,
                _ => return None,
            },
            matches!(s, "rltb" | "rl"),
        ),
        // TODO: EBU linePadding/multiRowAlign, escaped font names and complex CSS.
        _ => return None,
    })
}
pub(crate) fn length(s: &str, css: bool) -> Option<Length> {
    for (unit, kind) in [("px", 0), ("em", 1), ("%", 2)] {
        if let Some(v) = s.strip_suffix(unit) {
            let n = super::syntax::number(v, css)?;
            if n < 0.0 {
                return None;
            }
            return Some(match kind {
                0 => Length::Pixels(n),
                1 => Length::Em(n),
                _ => Length::Percent(n),
            });
        }
    }
    (css && super::syntax::number(s, true)? == 0.0).then_some(Length::Pixels(0.0))
}
fn lengths(s: &str, min: usize, max: usize, css: bool) -> Option<Vec<Length>> {
    let out: Option<Vec<_>> = s
        .split_ascii_whitespace()
        .take(max + 1)
        .map(|s| length(s, css))
        .collect();
    out.filter(|v| v.len() >= min && v.len() <= max)
}
fn families(s: &str, css: bool) -> Option<Arc<[FontFamily]>> {
    let mut out = Vec::new();
    let mut tail = s.trim();
    if tail.is_empty() {
        return None;
    }
    loop {
        let quoted = tail.starts_with(['\'', '"']);
        let (name, rest) = if quoted {
            let quote = tail.as_bytes()[0] as char;
            let end = tail[1..].find(quote)? + 1;
            let name = &tail[1..end];
            if name.is_empty() || name.chars().any(|c| c == '\\' || c.is_control()) {
                return None;
            }
            (name, &tail[end + 1..])
        } else {
            let end = tail.find(',').unwrap_or(tail.len());
            let name = tail[..end].trim();
            if name.is_empty() || !name.split_ascii_whitespace().all(super::syntax::identifier) {
                return None;
            }
            if css
                && [
                    "inherit",
                    "initial",
                    "unset",
                    "revert",
                    "revert-layer",
                    "default",
                ]
                .iter()
                .any(|v| name.eq_ignore_ascii_case(v))
            {
                return None;
            }
            (name, &tail[end..])
        };
        let name = if !css && !quoted {
            match name {
                "default" | "sansSerif" | "proportionalSansSerif" => "sans-serif",
                "monospaceSerif" | "monospaceSansSerif" => "monospace",
                "proportionalSerif" => "serif",
                _ => name,
            }
        } else {
            name
        };
        let generic = !quoted
            && [
                "serif",
                "sans-serif",
                "monospace",
                "cursive",
                "fantasy",
                "system-ui",
            ]
            .iter()
            .any(|v| name.eq_ignore_ascii_case(v));
        out.push(FontFamily {
            name: if generic {
                name.to_ascii_lowercase()
            } else {
                name.into()
            },
            generic,
        });
        tail = rest.trim_start();
        if tail.is_empty() {
            break;
        }
        tail = tail.strip_prefix(',')?.trim_start();
        if tail.is_empty() {
            return None;
        }
    }
    Some(out.into())
}
fn decoration(s: &str, css: bool) -> Option<(u8, u8)> {
    if s == "none" {
        return Some((0, 7));
    }
    let (mut add, mut remove) = (0, 0);
    for token in s.split_ascii_whitespace() {
        let (bit, neg) = match token {
            "underline" => (1, false),
            "overline" => (2, false),
            "lineThrough" if !css => (4, false),
            "line-through" if css => (4, false),
            "noUnderline" if !css => (1, true),
            "noOverline" if !css => (2, true),
            "noLineThrough" if !css => (4, true),
            _ => return None,
        };
        if (add | remove) & bit != 0 {
            return None;
        }
        if neg {
            remove |= bit;
        } else {
            add |= bit;
        }
    }
    if add | remove == 0 {
        None
    } else {
        Some((add, if css { 7 } else { remove }))
    }
}

fn color(s: &str, css: bool) -> Option<Color> {
    let function = s
        .strip_prefix("rgba(")
        .map(|v| (v, true))
        .or_else(|| s.strip_prefix("rgb(").map(|v| (v, false)));
    if let Some((body, alpha)) = function {
        // CSS alpha uses 0..1. TTML alpha uses whole bytes, from 0 to 255.
        // CSS also accepts percentages, but the RGB channels must agree.
        let parts: Vec<_> = body
            .strip_suffix(')')?
            .split(',')
            .take(5)
            .map(str::trim)
            .collect();
        if parts.len() != if alpha { 4 } else { 3 } {
            return None;
        }
        let percentages = parts[..3].iter().all(|v| v.ends_with('%'));
        if css && !percentages && parts[..3].iter().any(|v| v.ends_with('%')) {
            return None;
        }
        let mut out = [0, 0, 0, 255];
        for (i, value) in parts.iter().enumerate() {
            out[i] = if css {
                let n = if let Some(v) = value.strip_suffix('%') {
                    super::syntax::number(v, true)? / 100.0
                } else {
                    super::syntax::number(value, true)? / if i == 3 { 1.0 } else { 255.0 }
                };
                (n.clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                if !value.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                value.parse().ok()?
            };
        }
        return Some(out);
    }
    if let Some(hex) = s.strip_prefix('#') {
        let short = css && matches!(hex.len(), 3 | 4);
        if !short && !matches!(hex.len(), 6 | 8) {
            return None;
        }
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut out = [0, 0, 0, 255];
        let step = if short { 1 } else { 2 };
        for (i, value) in out.iter_mut().take(hex.len() / step).enumerate() {
            *value = u8::from_str_radix(&hex[i * step..(i + 1) * step], 16).ok()?;
            if short {
                *value *= 17;
            }
        }
        return Some(out);
    }
    if s.len() > 16 {
        return None;
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "transparent" => [0, 0, 0, 0],
        "black" => [0, 0, 0, 255],
        "silver" => [192, 192, 192, 255],
        "gray" => [128, 128, 128, 255],
        "white" => [255, 255, 255, 255],
        "maroon" => [128, 0, 0, 255],
        "red" => [255, 0, 0, 255],
        "purple" => [128, 0, 128, 255],
        "fuchsia" | "magenta" => [255, 0, 255, 255],
        "green" => [0, 128, 0, 255],
        "lime" => [0, 255, 0, 255],
        "olive" => [128, 128, 0, 255],
        "yellow" => [255, 255, 0, 255],
        "navy" => [0, 0, 128, 255],
        "blue" => [0, 0, 255, 255],
        "teal" => [0, 128, 128, 255],
        "aqua" | "cyan" => [0, 255, 255, 255],
        _ => return None,
    })
}
