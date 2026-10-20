//! Read IMSC1 text documents without a browser or a rendering backend.
//! Keep the timed text tree and the styles written in the document. Applying
//! inherited styles, choosing fonts and placing text are later steps.
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
use styles::{inline_style, specified_style, Resolver};
pub use time::{parse_time, Rates};
use tree::{build, space, timing, Context};
use xml::Element;
const TT: &str = "http://www.w3.org/ns/ttml";
const TTS: &str = "http://www.w3.org/ns/ttml#styling";
const TTP: &str = "http://www.w3.org/ns/ttml#parameter";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    /// Apply TTML whitespace rules later. The parser still keeps the source text.
    Default,
    /// Keep spaces and line breaks when processing the text.
    Preserve,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The role of a node in the timed text tree.
pub enum Kind {
    Body,
    /// A group of paragraphs or smaller groups.
    Division,
    Paragraph,
    /// Text with its own style, time range or language.
    Span,
    Break,
    /// A style change that applies during this node's time range.
    Set,
    Text,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    /// Start in document media time, in seconds.
    pub begin: f64,
    /// End in seconds, or no known end. Parent time ranges have been applied.
    pub end: Option<f64>,
}
#[derive(Debug, Clone, PartialEq)]
/// One entry in Document::nodes. Indices keep the tree flat, so dropping a
/// deeply nested document does not recursively drop child objects.
pub struct Node {
    /// Index of the parent, or None for a body node.
    pub parent: Option<usize>,
    /// Child indices, in source order.
    pub children: Vec<usize>,
    pub kind: Kind,
    /// The optional xml:id from the source.
    pub id: Option<String>,
    pub interval: Interval,
    /// Region ID from this node or its closest ancestor. None leaves the
    /// default-region rules to the later document processing step.
    pub region: Option<Arc<str>>,
    /// xml:lang from this node or its closest ancestor.
    pub language: Option<Arc<str>>,
    pub space: Space,
    /// Referenced styles, then inline values. Nodes that use the same named
    /// style share this allocation. Parent and region styles are not applied
    /// here; their links are kept so a later step can apply inheritance.
    pub style: Arc<Style>,
    /// Source text after XML decoding. Only Text nodes have a value. Spaces
    /// are kept; the space field tells a later step how to process them.
    pub text: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
/// A timed <set> on a region. Content-node sets appear in Document::nodes.
pub struct Animation {
    pub interval: Interval,
    pub style: Arc<Style>,
}
#[derive(Debug, Clone, PartialEq)]
/// A named text area and any timed changes to its style.
pub struct Region {
    pub id: String,
    pub interval: Interval,
    pub style: Arc<Style>,
    pub sets: Vec<Animation>,
}
#[derive(Debug, Clone, PartialEq)]
/// Parsed text and shared styles. This contains no browser or layout objects.
pub struct Document {
    /// Rates used to read frame and tick time expressions.
    pub rates: Rates,
    /// Style values on the tt element, before inheritance.
    pub root_style: Arc<Style>,
    /// Number of columns and rows used by TTML cell units.
    pub cell_resolution: (u32, u32),
    /// Named styles. Each is resolved once; node references share the result.
    pub styles: BTreeMap<String, Arc<Style>>,
    pub regions: BTreeMap<String, Region>,
    pub nodes: Vec<Node>,
    /// Problems that did not prevent reading the document.
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(bytes: &[u8]) -> Result<Document, Error> {
    parse_with_limits(bytes, Limits::default())
}
pub fn parse_with_limits(bytes: &[u8], limits: Limits) -> Result<Document, Error> {
    let mut budget = Budget::new(limits);
    let arena = xml::parse(bytes, limits, &mut budget)?;
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
    let mut doc = Document {
        rates,
        root_style: inline_style(root, &empty_style, &mut budget)?,
        cell_resolution: (32, 15),
        styles: BTreeMap::new(),
        regions: BTreeMap::new(),
        nodes: Vec::new(),
        diagnostics: Vec::new(),
    };
    if let Some(v) = root.attr(TTP, "cellResolution") {
        let p: Vec<_> = v.split_ascii_whitespace().take(3).collect();
        if p.len() != 2 {
            return Err(Error::Invalid("cellResolution".into()));
        }
        doc.cell_resolution = (positive_int(p[0])?, positive_int(p[1])?);
    }
    let structure = structure::collect(&arena, &mut budget)?;
    let definitions = &structure.definitions;
    for &index in &structure.elements {
        let e = &arena[index];
        for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
            if !definitions.contains_key(id) {
                budget.bytes(64 + id.len())?;
                doc.diagnostics.push(Diagnostic {
                    line: None,
                    message: format!("ignored unknown style reference {id}"),
                });
            }
        }
    }
    let mut resolver = Resolver::new(&arena, definitions, limits.depth, &empty_style, &mut budget);
    for id in definitions.keys() {
        resolver.resolve(id)?;
    }
    doc.styles = resolver.cache;
    for &index in &structure.regions {
        let e = &arena[index];
        let id = e
            .attr(XML, "id")
            .ok_or_else(|| Error::Invalid("region without xml:id".into()))?;
        let style = styles::region_style(e, &arena, &doc.styles, &empty_style, &mut budget)?;
        let interval = timing(e, rates, 0.0, None)?.0;
        let mut sets = Vec::new();
        for child in &e.children {
            let child = &arena[*child];
            if child.ns == TT && child.name == "set" {
                budget.nodes(1)?;
                budget.bytes(std::mem::size_of::<Animation>() * 2 + 32)?;
                sets.push(Animation {
                    interval: timing(child, rates, interval.begin, interval.end)?.0,
                    style: specified_style(child, &doc.styles, &empty_style, &mut budget)?,
                });
            }
        }
        budget.bytes(256 + id.len())?;
        doc.regions.insert(
            id.into(),
            Region {
                id: id.into(),
                interval,
                style,
                sets,
            },
        );
    }
    let space = space(root, Space::Default)?;
    let language = if let Some(value) = root.attr(XML, "lang") {
        budget.bytes(value.len() + 32)?;
        Some(Arc::<str>::from(value))
    } else {
        None
    };
    for i in &root.children {
        if arena[*i].ns == TT && arena[*i].name == "body" {
            build(
                *i,
                &arena,
                &mut doc,
                Context {
                    parent: None,
                    base: 0.0,
                    parent_end: None,
                    parent_space: space,
                    parent_language: language.clone(),
                    parent_region: None,
                    in_sequence: false,
                },
                &empty_style,
                &mut budget,
            )?;
        }
    }
    Ok(doc)
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
