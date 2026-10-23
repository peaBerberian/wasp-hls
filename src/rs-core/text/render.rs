//! Shared rendering output. Both parsers return the same small DOM-like tree.
use super::{budget::Budget, style::*, Diagnostic, Error};
use std::{collections::BTreeMap, sync::Arc};

pub type CssStyle = BTreeMap<&'static str, Arc<str>>;

/// A matching media timestamp and playback position, both in seconds.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Div,
    P,
    Span,
    Br,
    Ruby,
    Rt,
}
impl Tag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::P => "p",
            Self::Span => "span",
            Self::Br => "br",
            Self::Ruby => "ruby",
            Self::Rt => "rt",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// A preceding node, or None to attach directly to the subtitle viewport.
    pub parent: Option<usize>,
    pub tag: Tag,
    /// Active playback interval [start, end), in seconds. End may be infinity.
    pub start: f64,
    pub end: f64,
    pub style: CssStyle,
    pub language: Option<Arc<str>>,
    pub text: Option<String>,
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
    pub(super) fn offset(&mut self, timeline: Timeline) -> Result<(), Error> {
        let offset = timeline.offset()?;
        for n in &mut self.nodes {
            n.start += offset;
            n.end += offset;
        }
        Ok(())
    }
}

/// Set these on a positioned container covering the video image, then attach
/// root nodes to it. Styles use only CSS and the video height supplied here.
pub fn viewport_style(width: f64, height: f64) -> CssStyle {
    BTreeMap::from([
        ("position", "relative".into()),
        ("width", format!("{width}px").into()),
        ("height", format!("{height}px").into()),
        ("--subtitle-height", format!("{height}px").into()),
    ])
}

pub(super) fn length_css(l: Length) -> String {
    match l.unit {
        Unit::Pixel => format!("{}px", l.value),
        Unit::Em => format!("{}em", l.value),
        Unit::Percent => format!("{}%", l.value),
        Unit::ViewportHeight => format!("calc(var(--subtitle-height) * {})", l.value),
    }
}
fn color_css(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.0, c.1, c.2, c.3)
}

pub(super) fn css(style: &Style, budget: &mut Budget) -> Result<CssStyle, Error> {
    budget.bytes(
        style
            .properties
            .values()
            .map(|v| {
                256 + match v {
                    Value::Lengths(v) => v.len() * 384,
                    Value::Outline(_) => 4096,
                    Value::Number(_) => 384,
                    _ => 0,
                }
            })
            .sum(),
    )?;
    use Property::*;
    let mut out: CssStyle = style
        .properties
        .iter()
        .filter_map(|(p, v)| css_value(p, v))
        .collect();
    if !out.contains_key("direction")
        && matches!(style.properties.get(&WritingMode), Some(Value::Keyword(s)) if matches!(s.as_ref(),"rltb"|"rl"))
    {
        out.insert("direction", "rtl".into());
    }
    Ok(out)
}

pub(super) fn css_value(property: &Property, value: &Value) -> Option<(&'static str, Arc<str>)> {
    use Property::*;
    if matches!((property,value),(Display,Value::Keyword(s)) if s.as_ref()=="auto") {
        return None;
    }
    let name = match property {
        Color => "color",
        BackgroundColor => "background-color",
        FontFamily => "font-family",
        FontSize => "font-size",
        FontStyle => "font-style",
        FontWeight => "font-weight",
        TextDecoration => "text-decoration",
        TextAlign => "text-align",
        WritingMode => "writing-mode",
        Direction => "direction",
        UnicodeBidi => "unicode-bidi",
        LineHeight => "line-height",
        Opacity => "opacity",
        Padding => "padding",
        Display => "display",
        Visibility => "visibility",
        Overflow => "overflow",
        ZIndex => "z-index",
        WrapOption => "white-space",
        TextOutline => "text-shadow",
        DisplayAlign => "justify-content",
        Origin | Extent => return None,
    };
    let v = match value {
        Value::Text(s) | Value::Keyword(s) => {
            let mapped = match (property, s.as_ref()) {
                (WritingMode, "lrtb" | "rltb" | "lr" | "rl") => Some("horizontal-tb"),
                (WritingMode, "tbrl" | "tb") => Some("vertical-rl"),
                (WritingMode, "tblr") => Some("vertical-lr"),
                (UnicodeBidi, "bidiOverride") => Some("bidi-override"),
                (DisplayAlign, "before") => Some("flex-start"),
                (DisplayAlign, "center") => Some("center"),
                (DisplayAlign, "after") => Some("flex-end"),
                (WrapOption, "noWrap") => Some("pre"),
                (WrapOption, _) => Some("pre-wrap"),
                _ => None,
            };
            mapped.map_or_else(|| s.clone(), Arc::from)
        }
        Value::Color(c) => color_css(*c).into(),
        Value::Number(n) => n.to_string().into(),
        Value::Lengths(v) => v
            .iter()
            .map(|l| length_css(*l))
            .collect::<Vec<_>>()
            .join(" ")
            .into(),
        Value::Outline(o) => {
            let w = length_css(o.thickness);
            let c = o.color.map(color_css).unwrap_or("currentColor".into());
            format!("{w} 0 {c}, calc(-1 * {w}) 0 {c}, 0 {w} {c}, 0 calc(-1 * {w}) {c}").into()
        }
    };
    Some((name, v))
}

#[cfg(test)]
mod tests;
