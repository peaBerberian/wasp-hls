use super::{model::*, timestamp};
use crate::text::{budget::Budget, Error};
use std::collections::BTreeMap;

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
        .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
}
pub(super) fn timings(
    s: &str,
    regions: &[Region],
    region_index: &BTreeMap<String, usize>,
    budget: &mut Budget,
) -> Result<Option<(u64, u64, Settings)>, Error> {
    let Some((start, end, tokens)) = timing_fields(s) else {
        return Ok(None);
    };
    let mut settings = Settings::default();
    for token in tokens {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let mut valid = true;
        match key {
            "vertical" => match value {
                "rl" => settings.vertical = Vertical::Rl,
                "lr" => settings.vertical = Vertical::Lr,
                _ => valid = false,
            },
            "align" => {
                if let Some(a) = alignment(value) {
                    settings.align = a;
                } else {
                    valid = false;
                }
            }
            "size" => {
                if let Some(v) = percent(value) {
                    settings.size = v;
                } else {
                    valid = false;
                }
            }
            "region" => {
                settings.region = if let Some(&index) = region_index.get(value) {
                    let region = &regions[index];
                    budget.bytes(region.id.len() + 32)?;
                    Some(region.id.clone())
                } else {
                    None
                };
            }
            "line" => {
                let (v, a) = value
                    .split_once(',')
                    .map_or((value, None), |(v, a)| (v, Some(a)));
                let line = if let Some(p) = percent(v) {
                    Some(Line::Percent(p))
                } else {
                    line_number(v).map(Line::Number)
                };
                let a = match a {
                    None => Some(settings.line_align),
                    Some("start") => Some(LineAlign::Start),
                    Some("center") => Some(LineAlign::Center),
                    Some("end") => Some(LineAlign::End),
                    _ => None,
                };
                if let (Some(l), Some(a)) = (line, a) {
                    settings.line = l;
                    settings.line_align = a;
                } else {
                    valid = false;
                }
            }
            "position" => {
                let (v, a) = value
                    .split_once(',')
                    .map_or((value, None), |(v, a)| (v, Some(a)));
                let a = match a {
                    None => Some(settings.position_align),
                    Some("line-left") => Some(PositionAlign::LineLeft),
                    Some("center") => Some(PositionAlign::Center),
                    Some("line-right") => Some(PositionAlign::LineRight),
                    _ => None,
                };
                if let (Some(v), Some(a)) = (percent(v), a) {
                    settings.position = Some(v);
                    settings.position_align = a;
                } else {
                    valid = false;
                }
            }
            _ => valid = false,
        }
        let drops_region = match key {
            "line" => settings.line != Line::Auto,
            "vertical" => settings.vertical != Vertical::Horizontal,
            "size" => settings.size != 100.0,
            _ => false,
        };
        if valid && drops_region {
            settings.region = None;
        }
    }
    Ok(Some((start, end, settings)))
}

fn timing_fields(s: &str) -> Option<(u64, u64, std::str::SplitAsciiWhitespace<'_>)> {
    let (start, rest) = s.split_once("-->")?;
    let start = timestamp(start.trim_matches([' ', '\t']))?;
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let timestamp_end = rest
        .find(|c: char| !c.is_ascii_digit() && c != ':' && c != '.')
        .unwrap_or(rest.len());
    let end = timestamp(&rest[..timestamp_end])?;
    // Keep non-positive intervals. The consumer decides whether a cue is active.
    Some((start, end, rest[timestamp_end..].split_ascii_whitespace()))
}
fn alignment(s: &str) -> Option<Align> {
    Some(match s {
        "start" => Align::Start,
        "center" => Align::Center,
        "end" => Align::End,
        "left" => Align::Left,
        "right" => Align::Right,
        _ => return None,
    })
}
pub(super) fn parse_region<'a>(
    values: impl Iterator<Item = (&'a str, &'a str)>,
    budget: &mut Budget,
) -> Result<Region, Error> {
    budget.nodes(1)?;
    budget.bytes(std::mem::size_of::<Region>() * 2 + 32)?;
    let mut r = Region::default();
    for (k, v) in values {
        match k {
            "id" => {
                // Repeated IDs allocate too, even when the next one replaces them.
                budget.bytes(v.len() + 32)?;
                r.id = v.into();
            }
            "width" => {
                if let Some(p) = percent(v) {
                    r.width = p;
                }
            }
            "lines" => {
                if v.bytes().all(|b| b.is_ascii_digit()) {
                    if let Ok(n) = v.parse() {
                        r.lines = n;
                    }
                }
            }
            "scroll" => {
                if v == "up" {
                    r.scroll_up = true;
                }
            }
            "regionanchor" | "viewportanchor" => {
                if let Some((x, y)) = v.split_once(',') {
                    if let (Some(x), Some(y)) = (percent(x), percent(y)) {
                        if k == "regionanchor" {
                            r.region_anchor = (x, y);
                        } else {
                            r.viewport_anchor = (x, y);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(r)
}

fn line_number(s: &str) -> Option<f64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || digits.bytes().any(|b| !b.is_ascii_digit() && b != b'.') {
        return None;
    }
    if let Some((a, b)) = digits.split_once('.') {
        if a.is_empty() || b.is_empty() || b.contains('.') {
            return None;
        }
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}
