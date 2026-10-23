use super::{css::Selector, model::*};
use crate::text::{budget::Budget, CssStyle, Error, Timeline};
use std::collections::BTreeMap;

fn tag_name(tag: &Tag) -> &'static str {
    match tag {
        Tag::Class => "c",
        Tag::Italic => "i",
        Tag::Bold => "b",
        Tag::Underline => "u",
        Tag::Ruby => "ruby",
        Tag::RubyText => "rt",
        Tag::Voice => "v",
        Tag::Language => "lang",
    }
}
fn matched(selector: &Selector, node: &NodeKind, id: &str) -> Option<u8> {
    match selector {
        Selector::Cue if matches!(node, NodeKind::Root) => Some(0),
        Selector::CueId(s) if s == id && matches!(node, NodeKind::Root) => Some(100),
        Selector::Class(s) if matches!(node,NodeKind::Element {classes,..} if classes.contains(s)) => {
            Some(10)
        }
        Selector::Tag(s) if matches!(node,NodeKind::Element {tag,..} if tag_name(tag)==s) => {
            Some(1)
        }
        Selector::Voice(s) if matches!(node,NodeKind::Element {tag:Tag::Voice,annotation,..} if annotation==s) => {
            Some(11)
        }
        Selector::Language(s) if matches!(node,NodeKind::Element {tag:Tag::Language,annotation,..} if annotation==s) => {
            Some(11)
        }
        _ => None,
    }
}
fn styles(
    header: &Header,
    node: &NodeKind,
    id: &str,
    region: bool,
    budget: &mut Budget,
) -> Result<CssStyle, Error> {
    budget.bytes(1024)?;
    let mut style = CssStyle::new();
    if let NodeKind::Element { tag, classes, .. } = node {
        let builtin = match tag {
            Tag::Bold => Some(("font-weight", "bold")),
            Tag::Italic => Some(("font-style", "italic")),
            Tag::Underline => Some(("text-decoration", "underline")),
            _ => None,
        };
        if let Some((property, value)) = builtin {
            style.insert(property, value.into());
        }
        for class in classes {
            let (property, name) = class
                .strip_prefix("bg_")
                .map_or(("color", class.as_str()), |s| ("background-color", s));
            if [
                "white", "lime", "cyan", "red", "yellow", "magenta", "blue", "black",
            ]
            .contains(&name)
            {
                style.insert(property, name.into());
            }
        }
    }
    let mut priorities = BTreeMap::new();
    for rule in header.style_sheets.iter().flat_map(|s| &s.rules) {
        let specificity = rule
            .selectors
            .iter()
            .filter_map(|s| {
                if region {
                    matches!(s, Selector::Region).then_some(0)
                } else {
                    matched(s, node, id)
                }
            })
            .max();
        if let Some(specificity) = specificity {
            for d in &rule.declarations {
                let priority = (d.important, specificity);
                if priorities
                    .get(&d.property)
                    .is_none_or(|old| *old <= priority)
                {
                    budget.bytes(512)?;
                    priorities.insert(d.property, priority);
                    style.insert(d.property, d.value.clone());
                }
            }
        }
    }
    Ok(style)
}
fn placement(settings: &Settings, header: &Header) -> CssStyle {
    let mut s = CssStyle::from([
        ("position", "absolute".into()),
        ("white-space", "pre-wrap".into()),
        ("margin", "0".into()),
        ("font-size", "calc(var(--subtitle-height) * 0.05)".into()),
        ("line-height", "1.2".into()),
        ("color", "white".into()),
    ]);
    let align = match settings.align {
        Align::Start => "start",
        Align::Center => "center",
        Align::End => "end",
        Align::Left => "left",
        Align::Right => "right",
    };
    s.insert("text-align", align.into());
    if let Some(r) = settings
        .region
        .as_ref()
        .and_then(|id| header.regions.iter().rev().find(|r| &r.id == id))
    {
        // TODO: Support scrolling regions and collision avoidance between cues.
        s.extend([
            ("left", format!("{}%", r.viewport_anchor.0).into()),
            ("top", format!("{}%", r.viewport_anchor.1).into()),
            ("width", format!("{}%", r.width).into()),
            ("height", format!("{}em", r.lines as f64 * 1.2).into()),
            (
                "transform",
                format!(
                    "translate(-{}%, -{}%)",
                    r.region_anchor.0, r.region_anchor.1
                )
                .into(),
            ),
            ("overflow", "hidden".into()),
        ]);
        return s;
    }
    let position = settings.position.unwrap_or(match settings.align {
        Align::Start | Align::Left => 0.0,
        Align::End | Align::Right => 100.0,
        _ => 50.0,
    });
    let anchor = match settings.position_align {
        PositionAlign::LineLeft => 0.0,
        PositionAlign::Center => 0.5,
        PositionAlign::LineRight => 1.0,
        PositionAlign::Auto => match settings.align {
            Align::Start | Align::Left => 0.0,
            Align::End | Align::Right => 1.0,
            _ => 0.5,
        },
    };
    let max_size = if anchor == 0.0 {
        100.0 - position
    } else if anchor == 1.0 {
        position
    } else {
        position.min(100.0 - position) * 2.0
    };
    let vertical = settings.vertical != Vertical::Horizontal;
    s.insert(
        if vertical { "top" } else { "left" },
        format!("{position}%").into(),
    );
    s.insert(
        if vertical { "height" } else { "width" },
        format!("{}%", settings.size.min(max_size)).into(),
    );
    let line_anchor = match settings.line_align {
        LineAlign::Start => 0.0,
        LineAlign::Center => 50.0,
        LineAlign::End => 100.0,
    };
    match settings.line {
        Line::Auto => {
            s.insert(if vertical { "right" } else { "bottom" }, "0".into());
        }
        Line::Percent(n) => {
            s.insert(
                if vertical { "left" } else { "top" },
                format!("{n}%").into(),
            );
        }
        Line::Number(n) => {
            s.insert(
                if vertical {
                    if n < 0.0 {
                        "left"
                    } else {
                        "right"
                    }
                } else if n < 0.0 {
                    "bottom"
                } else {
                    "top"
                },
                format!("{}em", if n < 0.0 { (-n - 1.0) * 1.2 } else { n * 1.2 }).into(),
            );
        }
    }
    let along = anchor * 100.0;
    let across = if matches!(settings.line, Line::Percent(_)) {
        line_anchor
    } else {
        0.0
    };
    s.insert(
        "transform",
        if vertical {
            format!("translate(-{across}%, -{along}%)")
        } else {
            format!("translate(-{along}%, -{across}%)")
        }
        .into(),
    );
    s.insert(
        "writing-mode",
        match settings.vertical {
            Vertical::Horizontal => "horizontal-tb",
            Vertical::Rl => "vertical-rl",
            Vertical::Lr => "vertical-lr",
        }
        .into(),
    );
    s
}
pub(super) fn prepare(
    source: Document,
    timeline: Timeline,
    budget: &mut Budget,
) -> Result<crate::text::Document, Error> {
    let mut out = crate::text::Document {
        nodes: Vec::new(),
        diagnostics: source.diagnostics,
    };
    let offset = timeline.offset()?;
    for cue in source.cues {
        budget.bytes(4096 + cue.text.len() * std::mem::size_of::<usize>())?;
        let start = source
            .header
            .timestamp_map
            .map_seconds(cue.start_ms, (timeline.media_time * 90_000.0) as i64)
            + offset;
        let end = start + (cue.end_ms as f64 - cue.start_ms as f64) / 1000.0;
        let mut root = placement(&cue.settings, &source.header);
        if cue.settings.region.is_some() {
            root.extend(styles(
                &source.header,
                &NodeKind::Root,
                &cue.id,
                true,
                budget,
            )?);
        }
        let root = out.push(
            crate::text::Node {
                parent: None,
                tag: crate::text::Tag::Div,
                start,
                end,
                style: root,
                language: None,
                text: None,
            },
            budget,
        )?;
        let mut style = styles(&source.header, &NodeKind::Root, &cue.id, false, budget)?;
        style
            .entry("background-color")
            .or_insert("rgba(0, 0, 0, 0.8)".into());
        style.insert("box-decoration-break", "clone".into());
        let root = out.push(
            crate::text::Node {
                parent: Some(root),
                tag: crate::text::Tag::Span,
                start,
                end,
                style,
                language: None,
                text: None,
            },
            budget,
        )?;
        let mut parents = vec![root; cue.text.len()];
        for (i, n) in cue.text.into_iter().enumerate().skip(1) {
            let parent = parents[n.parent.unwrap_or(0)];
            let style = styles(&source.header, &n.kind, &cue.id, false, budget)?;
            let (tag, text, language) = match n.kind {
                NodeKind::Text(s) => (crate::text::Tag::Span, Some(s), None),
                NodeKind::Element {
                    tag, annotation, ..
                } => (
                    match tag {
                        Tag::Ruby => crate::text::Tag::Ruby,
                        Tag::RubyText => crate::text::Tag::Rt,
                        _ => crate::text::Tag::Span,
                    },
                    None,
                    (tag == Tag::Language).then(|| annotation.into()),
                ),
                NodeKind::Root => continue,
            };
            parents[i] = out.push(
                crate::text::Node {
                    parent: Some(parent),
                    tag,
                    start,
                    end,
                    style,
                    language,
                    text,
                },
                budget,
            )?;
        }
    }
    Ok(out)
}
