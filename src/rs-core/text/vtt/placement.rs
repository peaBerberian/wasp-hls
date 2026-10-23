use crate::text::{budget::Budget, *};
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub(super) struct Region {
    pub width: f64,
    pub lines: u32,
    pub anchor: [f64; 2],
    pub viewport: [f64; 2],
}
pub(super) fn region<'a>(
    fields: impl Iterator<Item = (&'a str, &'a str)>,
    budget: &mut Budget,
) -> Result<(String, Region), Error> {
    let mut id = String::new();
    let mut r = Region {
        width: 100.0,
        lines: 3,
        anchor: [0.0, 100.0],
        viewport: [0.0, 100.0],
    };
    for (key, value) in fields {
        match key {
            "id" => {
                budget.bytes(value.len() + 128)?;
                id = value.into();
            }
            "width" => {
                if let Some(n) = percent(value) {
                    r.width = n;
                }
            }
            "lines" => {
                if value.bytes().all(|b| b.is_ascii_digit()) {
                    if let Ok(n) = value.parse() {
                        r.lines = n;
                    }
                }
            }
            "regionanchor" | "viewportanchor" => {
                if let Some((a, b)) = value.split_once(',') {
                    if let (Some(a), Some(b)) = (percent(a), percent(b)) {
                        if key == "regionanchor" {
                            r.anchor = [a, b];
                        } else {
                            r.viewport = [a, b];
                        }
                    }
                }
            }
            _ => {}
        }
    }
    budget.nodes(1)?;
    budget.bytes(128)?;
    Ok((id, r))
}
pub(super) fn region_box(r: &Region) -> Placement {
    Placement {
        x: Anchor {
            offset: Length::Percent(r.viewport[0]),
            fraction: r.anchor[0] / 100.0,
            ..Anchor::default()
        },
        y: Anchor {
            offset: Length::Percent(r.viewport[1]),
            fraction: r.anchor[1] / 100.0,
            ..Anchor::default()
        },
        width: Some(Length::Percent(r.width)),
        height: Some(Length::Em(r.lines as f64 * 1.2)),
        ..Placement::default()
    }
}
fn percent(s: &str) -> Option<f64> {
    let s = s.strip_suffix('%')?;
    if s.is_empty()
        || s.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
        || s.split('.').any(str::is_empty)
    {
        return None;
    }
    s.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
}
fn number(s: &str) -> Option<f64> {
    let s0 = s.strip_prefix('-').unwrap_or(s);
    if s0.is_empty()
        || s0.split('.').any(str::is_empty)
        || s0.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
    {
        return None;
    }
    s.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (n.abs() * 1.2).is_finite())
}
fn align(s: &str) -> Option<TextAlign> {
    Some(match s {
        "start" => TextAlign::Start,
        "end" => TextAlign::End,
        "center" => TextAlign::Center,
        "left" => TextAlign::Left,
        "right" => TextAlign::Right,
        _ => return None,
    })
}
fn fraction(s: &str, position: bool) -> Option<f64> {
    // Position uses line-left/line-right. Line uses start/end instead.
    Some(match (position, s) {
        (true, "line-left") | (false, "start") => 0.0,
        (_, "center") => 0.5,
        (true, "line-right") | (false, "end") => 1.0,
        _ => return None,
    })
}
pub(super) struct Settings<'a> {
    pub region: Option<&'a Region>,
    pub placement: Placement,
    pub align: TextAlign,
    pub writing: WritingMode,
}
pub(super) fn cue<'a>(s: &str, regions: &'a BTreeMap<String, Region>) -> Settings<'a> {
    let (mut region, mut size, mut alignment, mut writing) =
        (None, 100.0, TextAlign::Center, WritingMode::Horizontal);
    let (mut position, mut position_anchor) = (None, None);
    // None is automatic; true is a percentage and false is a snapped line number.
    let (mut line, mut line_anchor) = (None, 0.0);
    for token in s.split_ascii_whitespace() {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        match key {
            "region" => region = regions.get(value),
            "align" => {
                if let Some(v) = align(value) {
                    alignment = v;
                }
            }
            "vertical" => {
                if matches!(value, "rl" | "lr") {
                    writing = if value == "rl" {
                        WritingMode::VerticalRl
                    } else {
                        WritingMode::VerticalLr
                    };
                    region = None;
                }
            }
            "size" => {
                if let Some(v) = percent(value) {
                    size = v;
                    if v != 100.0 {
                        region = None;
                    }
                }
            }
            "position" => {
                let (v, a) = value
                    .split_once(',')
                    .map_or((value, None), |(v, a)| (v, Some(a)));
                let anchor = a
                    .map(|a| fraction(a, true))
                    .unwrap_or(Some(position_anchor.unwrap_or(0.5)));
                if let (Some(v), Some(f)) = (percent(v), anchor) {
                    position = Some(v);
                    if a.is_some() {
                        position_anchor = Some(f);
                    }
                }
            }
            "line" => {
                let (v, a) = value
                    .split_once(',')
                    .map_or((value, None), |(v, a)| (v, Some(a)));
                let anchor = a.map(|a| fraction(a, false)).unwrap_or(Some(line_anchor));
                let n = percent(v)
                    .map(|v| (true, v))
                    .or_else(|| number(v).map(|v| (false, v)));
                if let (Some(v), Some(a)) = (n, anchor) {
                    line = Some(v);
                    line_anchor = a;
                    region = None;
                }
            }
            _ => {}
        }
    }
    let default = match alignment {
        TextAlign::Start | TextAlign::Left => 0.0,
        TextAlign::End | TextAlign::Right => 1.0,
        _ => 0.5,
    };
    let position = position.unwrap_or(default * 100.0);
    let anchor = position_anchor.unwrap_or(default);
    let maximum = if anchor == 0.0 {
        100.0 - position
    } else if anchor == 1.0 {
        position
    } else {
        position.min(100.0 - position) * 2.0
    };
    let along = Anchor {
        offset: Length::Percent(position),
        fraction: anchor,
        ..Anchor::default()
    };
    let vertical = writing != WritingMode::Horizontal;
    let across = match line {
        Some((true, n)) => Anchor {
            offset: Length::Percent(n),
            fraction: line_anchor,
            ..Anchor::default()
        },
        Some((false, n)) => {
            let from_end = if writing == WritingMode::VerticalRl {
                n >= 0.0
            } else {
                n < 0.0
            };
            Anchor {
                offset: Length::Em(if n < 0.0 { (-n - 1.0) * 1.2 } else { n * 1.2 }),
                from_end,
                fraction: if from_end { 1.0 } else { 0.0 },
            }
        }
        None => {
            let far = writing != WritingMode::VerticalLr;
            Anchor {
                from_end: far,
                fraction: if far { 1.0 } else { 0.0 },
                ..Anchor::default()
            }
        }
    };
    let mut placement = Placement {
        x: if vertical { across } else { along },
        y: if vertical { along } else { across },
        width: None,
        height: None,
        content_align: Align::Start,
        ..Placement::default()
    };
    if vertical {
        placement.height = Some(Length::Percent(size.min(maximum)));
    } else {
        placement.width = Some(Length::Percent(size.min(maximum)));
    }
    // TODO: Shared scrolling regions and collision avoidance between active cues.
    Settings {
        region,
        placement,
        align: alignment,
        writing,
    }
}
