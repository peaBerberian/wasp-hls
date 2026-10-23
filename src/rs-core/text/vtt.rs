//! Read WebVTT segments. Save the initialization header for later segments.
mod css;
mod entities;
mod placement;
mod text;
mod timestamp;
use super::{budget::Budget, Document, Error, Kind, Limits, Node, Timeline};
use timestamp::{timestamp, timestamp_map};
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct TimestampMap {
    local_ms: u64,
    mpegts: u64,
}
impl TimestampMap {
    fn map_seconds(self, local: u64, anchor: f64) -> f64 {
        let raw = self.mpegts as f64 / 90_000.0;
        let wrap = (1u64 << 33) as f64 / 90_000.0;
        // MPEG timestamps repeat after 2^33 ticks. Choose the repeat closest
        // to the given audio/video time.
        {
            let time = raw + (local as f64 - self.local_ms as f64) / 1000.0;
            time + ((anchor - time + wrap / 2.0) / wrap).floor() * wrap
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct Header {
    map: TimestampMap,
    rules: Vec<css::Rule>,
    regions: std::collections::BTreeMap<String, placement::Region>,
}
pub fn parse_init(bytes: &[u8]) -> Result<Header, Error> {
    let mut header = None;
    read(
        bytes,
        None,
        Timeline::default(),
        Limits::default(),
        &mut header,
    )?;
    Ok(header.unwrap())
}
pub fn parse(bytes: &[u8], init: Option<&Header>) -> Result<Document, Error> {
    parse_with_limits(bytes, init, Limits::default())
}
pub fn parse_with_limits(
    bytes: &[u8],
    init: Option<&Header>,
    limits: Limits,
) -> Result<Document, Error> {
    parse_with_timeline(bytes, init, Timeline::default(), limits)
}
pub fn parse_with_timeline(
    bytes: &[u8],
    init: Option<&Header>,
    timeline: Timeline,
    limits: Limits,
) -> Result<Document, Error> {
    read(bytes, init, timeline, limits, &mut None)
}

fn read(
    bytes: &[u8],
    init: Option<&Header>,
    timeline: Timeline,
    limits: Limits,
    save: &mut Option<Header>,
) -> Result<Document, Error> {
    let offset = timeline.offset()?;
    if bytes.len() > limits.bytes {
        return Err(Error::Limit("bytes"));
    }
    let mut budget = Budget::new(limits);
    // Replace invalid UTF-8 and NULL characters. Check the memory allowance
    // before creating strings. Replace both CRLF and CR with a newline.
    let source = normalize(bytes, &mut budget)?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let count = source.bytes().filter(|&b| b == b'\n').count() + 1;
    budget.bytes(count * std::mem::size_of::<&str>())?;
    if count > limits.nodes {
        return Err(Error::Limit("lines"));
    }
    let lines: Vec<_> = source.split('\n').collect();
    let signature =
        lines[0] == "WEBVTT" || lines[0].starts_with("WEBVTT ") || lines[0].starts_with("WEBVTT\t");
    if !signature && init.is_none() {
        return Err(Error::Header(
            "expected WEBVTT or initialization header".into(),
        ));
    }
    let mut header = Header::default();
    if !signature {
        let previous = init.unwrap();
        css::reserve(&previous.rules, &mut budget)?;
        budget.bytes(previous.regions.keys().map(|id| 128 + id.len()).sum())?;
        header = previous.clone();
    }
    let mut i = usize::from(signature);
    if signature {
        while i < lines.len() && !lines[i].is_empty() && !lines[i].contains("-->") {
            if let Some(v) = lines[i].strip_prefix("X-TIMESTAMP-MAP=") {
                header.map = timestamp_map(v)
                    .ok_or_else(|| Error::Header("invalid X-TIMESTAMP-MAP".into()))?;
            }
            if let Some(v) = lines[i].strip_prefix("Region:") {
                let (id, r) = placement::region(
                    v.split_ascii_whitespace().filter_map(|t| t.split_once('=')),
                    &mut budget,
                )?;
                header.regions.insert(id, r);
            }
            i += 1;
        }
    }
    let mut out = Document::default();
    while i < lines.len() {
        let first = lines[i];
        let line = i + 1;
        if first.is_empty() {
            i += 1;
            continue;
        }
        if first == "NOTE" || first.starts_with("NOTE ") || first.starts_with("NOTE\t") {
            while i < lines.len() && !lines[i].is_empty() {
                i += 1;
            }
            continue;
        }
        if first == "STYLE" || first == "REGION" {
            i += 1;
            let begin = i;
            while i < lines.len() && !lines[i].is_empty() && !lines[i].contains("-->") {
                i += 1;
            }
            if !out.nodes.is_empty() || (i < lines.len() && lines[i].contains("-->")) {
                diagnostic(
                    &mut out,
                    line,
                    "invalid or late style/region block",
                    &mut budget,
                )?;
            } else {
                if first == "STYLE" {
                    budget.bytes(lines[begin..i].iter().map(|s| s.len() + 1).sum())?;
                    header.rules.extend(css::parse(
                        &lines[begin..i].join("\n"),
                        limits,
                        &mut budget,
                    )?);
                }
                if first == "REGION" {
                    let (id, r) = placement::region(
                        lines[begin..i]
                            .iter()
                            .flat_map(|s| s.split_ascii_whitespace())
                            .filter_map(|t| t.split_once(':')),
                        &mut budget,
                    )?;
                    header.regions.insert(id, r);
                }
            }
            continue;
        }
        let mut id = "";
        if !lines[i].contains("-->") {
            id = lines[i];
            i += 1;
        }
        if i == lines.len() {
            break;
        }
        let timing = timings(lines[i]);
        i += 1;
        let begin = i;
        while i < lines.len() && !lines[i].is_empty() && !lines[i].contains("-->") {
            i += 1;
        }
        if let Some((start, end, _settings)) = timing {
            let duration = (end as f64 - start as f64) / 1000.0;
            let start = header.map.map_seconds(start, timeline.media_time) + offset;
            let end = start + duration;
            let mut parent = None;
            let mut computed = super::properties::Computed::vtt();
            let settings = placement::cue(_settings, &header.regions);
            if let Some(r) = settings.region {
                css::apply(
                    &header.rules,
                    css::Target::Region,
                    &mut computed,
                    &mut budget,
                )?;
                let mut n = Node::new(None, Kind::Region, start, end);
                n.appearance = computed.appearance.clone();
                n.placement = Some(placement::region_box(r));
                parent = Some(out.push(n, &mut budget)?);
                computed = computed.child();
            }
            let mut node = Node::new(parent, Kind::Paragraph, start, end);
            css::apply(
                &header.rules,
                css::Target::Root(id),
                &mut computed,
                &mut budget,
            )?;
            node.appearance = computed.appearance.clone();
            node.placement = if settings.region.is_some() {
                None
            } else {
                Some(settings.placement)
            };
            computed.appearance.text_align = settings.align;
            computed.appearance.writing_mode = settings.writing;
            node.appearance = computed.appearance.clone();
            let parent = out.push(node, &mut budget)?;
            budget.bytes(lines[begin..i].iter().map(|s| s.len() + 1).sum())?;
            text::parse(
                &lines[begin..i].join("\n"),
                parent,
                &mut out,
                limits,
                &mut budget,
                text::Styles {
                    rules: &header.rules,
                    computed,
                },
            )?;
        } else {
            diagnostic(&mut out, line, "invalid cue timings", &mut budget)?;
        }
    }
    *save = Some(header);
    Ok(out)
}
fn timings(s: &str) -> Option<(u64, u64, &str)> {
    let (start, tail) = s.split_once("-->")?;
    let start = timestamp(start.trim_matches([' ', '\t']))?;
    let tail = tail.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && !matches!(c, ':' | '.'))
        .unwrap_or(tail.len());
    Some((start, timestamp(&tail[..end])?, &tail[end..]))
}
fn diagnostic(
    out: &mut Document,
    line: usize,
    message: &str,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.nodes(1)?;
    budget.bytes(64 + message.len())?;
    out.diagnostics.push(super::Diagnostic {
        line: Some(line),
        message: message.into(),
    });
    Ok(())
}

fn normalize<'a>(bytes: &'a [u8], budget: &mut Budget) -> Result<std::borrow::Cow<'a, str>, Error> {
    use std::borrow::Cow;
    let decoded = match std::str::from_utf8(bytes) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => {
            budget.bytes(bytes.len().saturating_mul(3))?;
            String::from_utf8_lossy(bytes)
        }
    };
    if !decoded.contains(['\0', '\r']) {
        return Ok(decoded);
    }
    budget.bytes(decoded.len() + decoded.bytes().filter(|&b| b == 0).count() * 2)?;
    let mut out = String::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\0' => out.push('\u{fffd}'),
            '\r' => {
                out.push('\n');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    Ok(Cow::Owned(out))
}
