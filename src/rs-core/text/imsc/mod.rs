//! Read IMSC XML and return text with its start and end times.
mod styles;
mod time;
mod tree;
mod xml;
use super::{budget::Budget, Document, Error, Kind, Limits, Node, Timeline};
pub use time::{parse_time, Rates};
use xml::Element;
const TT: &str = "http://www.w3.org/ns/ttml";
const TTS: &str = "http://www.w3.org/ns/ttml#styling";
const TTP: &str = "http://www.w3.org/ns/ttml#parameter";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

pub fn parse(bytes: &[u8]) -> Result<Document, Error> {
    parse_with_limits(bytes, Limits::default())
}
pub fn parse_with_limits(bytes: &[u8], limits: Limits) -> Result<Document, Error> {
    parse_with_timeline(bytes, Timeline::default(), limits)
}
pub fn parse_with_timeline(
    bytes: &[u8],
    timeline: Timeline,
    limits: Limits,
) -> Result<Document, Error> {
    let offset = timeline.offset()?;
    let mut budget = Budget::new(limits);
    let mut xml = xml::parse(bytes, limits, &mut budget)?;
    let root = &xml[0];
    if root.ns != TT || root.name != "tt" {
        return Err(Error::Header("expected TTML tt root and namespace".into()));
    }
    if root.attr(TTP, "timeBase").is_some_and(|v| v != "media")
        || root.attr(TTP, "subFrameRate").is_some()
        || root
            .attr(TTP, "profile")
            .is_some_and(|v| v != "http://www.w3.org/ns/ttml/profile/imsc1/text")
    {
        return Err(Error::Unsupported("IMSC profile or time base".into()));
    }
    let rates = rates(root)?;
    let preserve = tree::space(root, false)?;
    let language = tree::language(root, None, &mut budget)?;
    let mut styles = styles::Styles::read(&xml, limits, &mut budget)?;
    let out = Document::default();
    let out = Document {
        diagnostics: std::mem::take(&mut styles.diagnostics),
        ..out
    };
    let mut reader = tree::Reader {
        xml: &mut xml,
        rates,
        out,
        preserved: Vec::new(),
        ids: std::collections::BTreeSet::new(),
        budget: &mut budget,
        styles,
        groups: Vec::new(),
    };
    // Start reading text and times at body, not at the tt root.
    for child in std::mem::take(&mut reader.xml[0].children) {
        if reader.xml[child].ns == TT && reader.xml[child].name == "body" {
            reader.build(
                child,
                tree::Context {
                    parent: None,
                    base: 0.0,
                    end: None,
                    sequence: false,
                    preserve,
                    language: language.clone(),
                    computed: reader.styles.base.clone(),
                    region: None,
                },
            )?;
        }
    }
    for n in &mut reader.out.nodes {
        let end = n.end + offset;
        n.start += offset;
        if !n.start.is_finite() || (n.end.is_finite() && !end.is_finite()) {
            return Err(Error::Invalid("playback time overflow".into()));
        }
        n.end = end;
    }
    Ok(reader.out)
}
/// Limit the returned text to this MP4 sample. XML times use the media timeline.
pub fn parse_sample(
    sample: &super::mp4::Sample<'_>,
    timeline: Timeline,
    limits: Limits,
) -> Result<Document, Error> {
    if sample.timescale == 0 {
        return Err(Error::Invalid("zero subtitle timescale".into()));
    }
    let offset = timeline.offset()?;
    let mut out = parse_with_timeline(sample.document, timeline, limits)?;
    let start = sample.presentation_time as f64 / sample.timescale as f64 + offset;
    let end = start + sample.duration as f64 / sample.timescale as f64;
    for n in &mut out.nodes {
        n.start = n.start.max(start);
        n.end = n.end.min(end);
    }
    Ok(out)
}
fn positive(s: &str) -> Result<u32, Error> {
    s.parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| Error::Invalid(format!("expected positive integer: {s}")))
}
fn pair(s: &str) -> Result<(u32, u32), Error> {
    let mut p = s.split_ascii_whitespace();
    let (a, b) = (p.next(), p.next());
    if p.next().is_some() {
        return Err(Error::Invalid("expected two integers".into()));
    }
    Ok((positive(a.unwrap_or(""))?, positive(b.unwrap_or(""))?))
}
fn rates(root: &Element) -> Result<Rates, Error> {
    let mut out = Rates::default();
    if let Some(v) = root.attr(TTP, "frameRate") {
        out.frame_rate = positive(v)? as f64;
        out.explicit_frame_rate = true;
    }
    if let Some(v) = root.attr(TTP, "frameRateMultiplier") {
        let (a, b) = pair(v)?;
        out.frame_rate *= a as f64 / b as f64;
    }
    if out.explicit_frame_rate {
        out.tick_rate = out.frame_rate;
    }
    if let Some(v) = root.attr(TTP, "tickRate") {
        out.tick_rate = positive(v)? as f64;
        out.explicit_tick_rate = true;
    }
    Ok(out)
}
