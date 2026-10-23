//! Prepare ordinary IMSC1 text subtitles for the shared rendering tree.
mod render;
mod structure;
mod style_values;
mod styles;
mod time;
mod tree;
mod xml;
use super::{style::Style, Diagnostic, Error, Limits};
use crate::text::budget::Budget;
use std::collections::BTreeMap;
use std::sync::Arc;
use styles::{specified_style, Resolver};
pub use time::{parse_time, Rates};
use tree::{space, timing, Context};
use xml::Element;
const TT: &str = "http://www.w3.org/ns/ttml";
const TTS: &str = "http://www.w3.org/ns/ttml#styling";
const TTP: &str = "http://www.w3.org/ns/ttml#parameter";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Space {
    /// Collapse XML whitespace and let CSS handle spaces across inline elements.
    Default,
    /// Keep spaces and line breaks when processing the text.
    Preserve,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Interval {
    begin: f64,
    end: Option<f64>,
}
struct Region {
    interval: Interval,
    style: Arc<Style>,
}

pub fn parse(bytes: &[u8]) -> Result<crate::text::Document, Error> {
    parse_with_limits(bytes, Limits::default())
}
pub fn parse_with_limits(bytes: &[u8], limits: Limits) -> Result<crate::text::Document, Error> {
    parse_with_timeline(bytes, crate::text::Timeline::default(), limits)
}
pub fn parse_with_timeline(
    bytes: &[u8],
    timeline: crate::text::Timeline,
    limits: Limits,
) -> Result<crate::text::Document, Error> {
    let mut budget = Budget::new(limits);
    let mut result = read(bytes, limits, &mut budget)?;
    result.offset(timeline)?;
    Ok(result)
}

/// Extracted sample coverage and playback mapping are applied here, not by a renderer.
pub fn parse_sample(
    sample: &crate::text::mp4::Sample<'_>,
    timeline: crate::text::Timeline,
    limits: Limits,
) -> Result<crate::text::Document, Error> {
    if sample.timescale == 0 {
        return Err(Error::Invalid("zero subtitle timescale".into()));
    }
    let mut result = parse_with_timeline(sample.document, timeline, limits)?;
    let start = sample.presentation_time as f64 / sample.timescale as f64 + timeline.playback_time
        - timeline.media_time;
    let end = start + sample.duration as f64 / sample.timescale as f64;
    for n in &mut result.nodes {
        n.start = n.start.max(start);
        n.end = n.end.min(end);
    }
    Ok(result)
}

fn read(bytes: &[u8], limits: Limits, budget: &mut Budget) -> Result<crate::text::Document, Error> {
    let mut arena = xml::parse(bytes, limits, budget)?;
    let root = &arena[0];
    if root.ns != TT || root.name != "tt" {
        return Err(Error::Header("expected TTML tt root and namespace".into()));
    }
    if root.attr(TTP, "timeBase").is_some_and(|v| v != "media") {
        return Err(Error::Unsupported("IMSC timeBase other than media".into()));
    }
    if root.attr(TTP, "subFrameRate").is_some() {
        return Err(Error::Unsupported("IMSC subFrameRate".into()));
    }
    if let Some(profile) = root.attr(TTP, "profile") {
        if profile != "http://www.w3.org/ns/ttml/profile/imsc1/text" {
            return Err(Error::Unsupported(format!("IMSC profile {profile}")));
        }
    }
    let rates = parse_rates(root)?;
    budget.bytes(std::mem::size_of::<Style>() + 32)?;
    let empty_style = Arc::new(Style::default());
    let extent = root
        .attr(TTS, "extent")
        .and_then(|s| style_values::parse("extent", s))
        .and_then(|(_, v)| match v {
            super::style::Value::Lengths(v) => Some([v[0], v[1]]),
            _ => None,
        });
    let mut cell_resolution = (32, 15);
    let mut out = crate::text::Document::default();
    if let Some(v) = root.attr(TTP, "cellResolution") {
        let p: Vec<_> = v.split_ascii_whitespace().take(3).collect();
        if p.len() != 2 {
            return Err(Error::Invalid("cellResolution".into()));
        }
        cell_resolution = (positive_int(p[0])?, positive_int(p[1])?);
    }
    let structure = structure::collect(&arena, budget)?;
    let definitions = &structure.definitions;
    for &index in &structure.elements {
        let e = &arena[index];
        for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
            if !definitions.contains_key(id) {
                budget.bytes(64 + id.len())?;
                out.diagnostics.push(Diagnostic {
                    line: None,
                    message: format!("ignored unknown style reference {id}"),
                });
            }
        }
    }
    let mut resolver = Resolver::new(&arena, definitions, limits.depth, &empty_style, budget);
    for id in definitions.keys() {
        resolver.resolve(id)?;
    }
    let styles = resolver.cache;
    let mut regions = BTreeMap::new();
    for &index in &structure.regions {
        let e = &arena[index];
        let id = e
            .attr(XML, "id")
            .ok_or_else(|| Error::Invalid("region without xml:id".into()))?;
        let style = styles::region_style(e, &arena, &styles, &empty_style, budget)?;
        let interval = timing(e, rates, 0.0, None)?.0;
        budget.bytes(256 + id.len())?;
        regions.insert(id.into(), Region { interval, style });
    }
    let space = space(root, Space::Default)?;
    budget.bytes(root.attr(XML, "lang").map_or(0, |s| s.len() + 32))?;
    let language = root.attr(XML, "lang").map(Arc::<str>::from);
    let children = std::mem::take(&mut arena[0].children);
    let layout = render::Layout::new(cell_resolution.1, extent);
    let style = Arc::new(layout.base.clone());
    let mut builder = tree::Builder {
        arena: &mut arena,
        styles: &styles,
        regions: &regions,
        empty: &empty_style,
        rates,
        layout,
        groups: Vec::new(),
        out,
        budget,
    };
    for i in children {
        if builder.arena[i].ns == TT && builder.arena[i].name == "body" {
            builder.build(
                i,
                Context {
                    parent: None,
                    base: 0.0,
                    parent_end: None,
                    space,
                    language: language.clone(),
                    region: None,
                    in_sequence: false,
                    style: style.clone(),
                },
            )?;
        }
    }
    Ok(builder.out)
}

fn positive_int(s: &str) -> Result<u32, Error> {
    s.parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| Error::Invalid(format!("expected positive integer: {s}")))
}
fn parse_rates(root: &Element) -> Result<Rates, Error> {
    let mut r = Rates::default();
    if let Some(v) = root.attr(TTP, "frameRate") {
        r.frame_rate = f64::from(positive_int(v)?);
        r.explicit_frame_rate = true;
    }
    if let Some(v) = root.attr(TTP, "frameRateMultiplier") {
        let p: Vec<_> = v.split_ascii_whitespace().take(3).collect();
        if p.len() != 2 {
            return Err(Error::Invalid("frameRateMultiplier".into()));
        }
        r.frame_rate *= f64::from(positive_int(p[0])?) / f64::from(positive_int(p[1])?);
    }
    r.tick_rate = if r.explicit_frame_rate {
        r.frame_rate
    } else {
        1.0
    };
    if let Some(v) = root.attr(TTP, "tickRate") {
        r.tick_rate = f64::from(positive_int(v)?);
        r.explicit_tick_rate = true;
    }
    Ok(r)
}

#[cfg(test)]
mod tests;
