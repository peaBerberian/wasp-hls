use super::*;
use crate::text::{
    render::{css, length_css},
    style::{Length, Property::*, Unit, Value},
    CssStyle,
};

fn font(style: &Style) -> Length {
    match &style.properties[&FontSize] {
        Value::Lengths(v) => v[0],
        _ => unreachable!(),
    }
}
fn relative(value: Length, parent: Length, pixels: Option<f64>) -> Length {
    match value.unit {
        Unit::Percent | Unit::Em => Length {
            value: parent.value * value.value
                / if value.unit == Unit::Percent {
                    100.0
                } else {
                    1.0
                },
            unit: parent.unit,
        },
        Unit::Pixel if pixels.is_some() => Length {
            value: value.value / pixels.unwrap(),
            unit: Unit::ViewportHeight,
        },
        _ => value,
    }
}
fn inherited(p: &crate::text::style::Property) -> bool {
    matches!(
        p,
        Color
            | FontFamily
            | FontSize
            | FontStyle
            | FontWeight
            | TextDecoration
            | TextAlign
            | WritingMode
            | Direction
            | LineHeight
            | Visibility
            | TextOutline
            | WrapOption
    )
}
pub(super) fn cascade(
    parent: &Style,
    local: &Style,
    pixels: Option<f64>,
    groups: bool,
    budget: &mut Budget,
) -> Result<Style, Error> {
    budget.bytes((parent.properties.len() + local.properties.len()) * 256)?;
    let mut out = Style {
        properties: parent
            .properties
            .iter()
            .filter(|(p, _)| inherited(p))
            .map(|(p, v)| (p.clone(), v.clone()))
            .collect(),
    };
    out.overlay(local);
    if let Some(Value::Lengths(v)) = local.properties.get(&FontSize) {
        out.properties.insert(
            FontSize,
            Value::Lengths(vec![relative(v[0], font(parent), pixels)]),
        );
    }
    if let Some(Value::Lengths(v)) = local.properties.get(&LineHeight) {
        out.properties.insert(
            LineHeight,
            Value::Lengths(vec![relative(v[0], font(&out), pixels)]),
        );
    }
    // TODO: Handle percentage padding and backgrounds on flattened groups.
    if let Some(Value::Lengths(v)) = local.properties.get(&Padding) {
        out.properties.insert(
            Padding,
            Value::Lengths(
                v.iter()
                    .map(|l| {
                        if l.unit == Unit::Percent {
                            *l
                        } else {
                            relative(*l, font(&out), pixels)
                        }
                    })
                    .collect(),
            ),
        );
    }
    if let Some(Value::Outline(o)) = local.properties.get(&TextOutline) {
        let mut o = o.clone();
        o.thickness = relative(o.thickness, font(&out), pixels);
        out.properties.insert(TextOutline, Value::Outline(o));
    }
    if let Some(Value::Keyword(value)) = local.properties.get(&TextDecoration) {
        let old = match parent.properties.get(&TextDecoration) {
            Some(Value::Keyword(s)) => s.as_ref(),
            _ => "",
        };
        let mut lines = [
            old.contains("underline"),
            old.contains("line-through"),
            old.contains("overline"),
        ];
        for word in value.split_ascii_whitespace() {
            match word {
                "none" => lines = [false; 3],
                "underline" => lines[0] = true,
                "noUnderline" => lines[0] = false,
                "lineThrough" => lines[1] = true,
                "noLineThrough" => lines[1] = false,
                "overline" => lines[2] = true,
                "noOverline" => lines[2] = false,
                _ => {}
            }
        }
        out.properties.insert(
            TextDecoration,
            Value::Keyword(
                ["underline", "line-through", "overline"]
                    .into_iter()
                    .zip(lines)
                    .filter_map(|(s, yes)| yes.then_some(s))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .into(),
            ),
        );
    }
    if groups {
        let opacity = |s: &Style| match s.properties.get(&Opacity) {
            Some(Value::Number(n)) => *n,
            _ => 1.0,
        };
        if parent.properties.contains_key(&Opacity) {
            out.properties
                .insert(Opacity, Value::Number(opacity(parent) * opacity(local)));
        }
    }
    if matches!(parent.properties.get(&Display),Some(Value::Keyword(s)) if s.as_ref()=="none") {
        out.properties
            .insert(Display, Value::Keyword("none".into()));
    }
    Ok(out)
}
pub(super) fn normalized(
    source: String,
    preserve: bool,
    budget: &mut Budget,
) -> Result<String, Error> {
    if preserve {
        return Ok(source);
    }
    budget.bytes(source.len())?;
    let mut out = String::with_capacity(source.len());
    for c in source.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

pub(super) struct Layout {
    pub base: Style,
    pub pixels: Option<f64>,
    extent: Option<[Length; 2]>,
}
impl Layout {
    pub fn new(rows: u32, extent: Option<[Length; 2]>) -> Self {
        Self {
            pixels: extent
                .filter(|v| v[1].unit == Unit::Pixel && v[1].value > 0.0)
                .map(|v| v[1].value),
            extent,
            base: Style {
                properties: BTreeMap::from([
                    (
                        FontSize,
                        Value::Lengths(vec![Length {
                            value: 1.0 / rows as f64,
                            unit: Unit::ViewportHeight,
                        }]),
                    ),
                    (
                        Color,
                        Value::Color(crate::text::style::Color(255, 255, 255, 255)),
                    ),
                    (FontFamily, Value::Text("sans-serif".into())),
                ]),
            },
        }
    }
    pub fn region(
        &self,
        region: Option<&Region>,
        computed: &Style,
        budget: &mut Budget,
    ) -> Result<CssStyle, Error> {
        // TODO: Share region boxes for overlapping paragraphs and backgrounds
        // that must remain visible while the region has no active text.
        budget.bytes(4096)?;
        let mut style = css(computed, budget)?;
        style.extend([
            ("position", "absolute".into()),
            ("flex-direction", "column".into()),
        ]);
        style.entry("display").or_insert("flex".into());
        style.entry("justify-content").or_insert("flex-end".into());
        for (property, names, defaults) in [
            (Origin, ["left", "top"], ["0%", "0%"]),
            (Extent, ["width", "height"], ["100%", "100%"]),
        ] {
            let values = region.and_then(|r| r.style.properties.get(&property));
            for axis in 0..2 {
                let value = match values {
                    Some(Value::Lengths(v)) => {
                        let mut l = v[axis];
                        if l.unit == Unit::Pixel {
                            if let Some(e) = self
                                .extent
                                .filter(|v| v[axis].unit == Unit::Pixel && v[axis].value > 0.0)
                            {
                                l = Length {
                                    value: l.value / e[axis].value * 100.0,
                                    unit: Unit::Percent,
                                };
                            }
                        }
                        length_css(l).into()
                    }
                    _ => defaults[axis].into(),
                };
                style.insert(names[axis], value);
            }
        }
        Ok(style)
    }
}
