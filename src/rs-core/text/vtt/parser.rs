use super::{
    css,
    model::*,
    settings::{parse_region, timings},
    text::cue_text,
    timestamp::timestamp_map,
};
use crate::text::{budget::Budget, Diagnostic, Error, Limits};
use std::collections::BTreeMap;

pub(super) fn read(
    bytes: &[u8],
    initialization: Option<&Header>,
    limits: Limits,
    budget: &mut Budget,
) -> Result<Document, Error> {
    if bytes.len() > limits.bytes {
        return Err(Error::Limit("bytes"));
    }
    // Valid UTF-8 normally needs no copy. Invalid bytes can expand to three
    // bytes each when replaced, so check that space before decoding them.
    let decoded = match std::str::from_utf8(bytes) {
        Ok(s) => std::borrow::Cow::Borrowed(s),
        Err(_) => {
            budget.bytes(
                bytes
                    .len()
                    .checked_mul(3)
                    .ok_or(Error::Limit("output bytes"))?,
            )?;
            String::from_utf8_lossy(bytes)
        }
    };
    // Replace NULL and normalize line endings in one pass. Chained replace()
    // calls would briefly keep several copies of a large segment alive.
    let normalized = if decoded.contains(['\0', '\r']) {
        let capacity = decoded
            .len()
            .checked_add(decoded.bytes().filter(|&b| b == 0).count() * 2)
            .ok_or(Error::Limit("output bytes"))?;
        budget.bytes(capacity)?;
        let mut out = String::with_capacity(capacity);
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
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(decoded.as_ref())
    };
    let source = normalized.strip_prefix('\u{feff}').unwrap_or(&normalized);
    let line_count = source.bytes().filter(|&b| b == b'\n').count() + 1;
    if line_count > limits.nodes {
        return Err(Error::Limit("lines"));
    }
    budget.bytes(line_count * std::mem::size_of::<&str>() * 2)?;
    let lines: Vec<_> = source.split('\n').collect();
    let signature =
        lines[0] == "WEBVTT" || lines[0].starts_with("WEBVTT ") || lines[0].starts_with("WEBVTT\t");
    let header = if signature {
        Header::default()
    } else if let Some(header) = initialization {
        reserve_header_copy(header, budget)?;
        header.clone()
    } else {
        Header::default()
    };
    let mut doc = Document {
        header,
        cues: Vec::new(),
        diagnostics: Vec::new(),
    };
    // Keep definitions in source order for consumers. The index points to the
    // last definition of each ID without scanning that list for every cue.
    let mut region_index = BTreeMap::new();
    for (index, region) in doc.header.regions.iter().enumerate() {
        index_region(&mut region_index, &region.id, index, budget)?;
    }
    let mut i = 0;
    if signature {
        i = 1;
        while i < lines.len() && !lines[i].is_empty() {
            if lines[i].contains("-->") {
                break;
            }
            if let Some(map) = lines[i].strip_prefix("X-TIMESTAMP-MAP=") {
                doc.header.timestamp_map = timestamp_map(map)
                    .ok_or_else(|| Error::Header("invalid X-TIMESTAMP-MAP".into()))?;
            } else if let Some(region) = lines[i].strip_prefix("Region:") {
                let region = parse_region(
                    region
                        .split_ascii_whitespace()
                        .filter_map(|s| s.split_once('=')),
                    budget,
                )?;
                index_region(
                    &mut region_index,
                    &region.id,
                    doc.header.regions.len(),
                    budget,
                )?;
                doc.header.regions.push(region);
            }
            i += 1;
        }
    } else if initialization.is_none() {
        return Err(Error::Header(
            "expected WEBVTT or an EXT-X-MAP header".into(),
        ));
    }
    let mut node_count = 0usize;
    while i < lines.len() {
        if lines[i].is_empty() {
            i += 1;
            continue;
        }
        let line_number = i + 1;
        let first = lines[i];
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
            if !doc.cues.is_empty() || (i < lines.len() && lines[i].contains("-->")) {
                diagnostic(
                    &mut doc,
                    line_number,
                    "invalid or late style/region block",
                    budget,
                )?;
            } else if first == "STYLE" {
                let length = lines[begin..i].iter().map(|s| s.len() + 1).sum::<usize>();
                budget.bytes(length + std::mem::size_of::<css::Stylesheet>() * 2 + 32)?;
                let source = lines[begin..i].join("\n");
                doc.header
                    .style_sheets
                    .push(css::parse(&source, limits, budget)?);
            } else {
                // Settings cannot span words, so joining lines would only make
                // an unnecessary temporary copy of the complete region block.
                let region = parse_region(
                    lines[begin..i]
                        .iter()
                        .flat_map(|line| line.split_ascii_whitespace())
                        .filter_map(|s| s.split_once(':')),
                    budget,
                )?;
                index_region(
                    &mut region_index,
                    &region.id,
                    doc.header.regions.len(),
                    budget,
                )?;
                doc.header.regions.push(region);
            }
            continue;
        }
        let mut id = "";
        if !lines[i].contains("-->") {
            id = lines[i];
            i += 1;
        }
        if i >= lines.len() {
            break;
        }
        let timing = timings(lines[i], &doc.header.regions, &region_index, budget)?;
        i += 1;
        let begin = i;
        // A new timing line starts a new cue even without an empty separator.
        while i < lines.len() && !lines[i].is_empty() && !lines[i].contains("-->") {
            i += 1;
        }
        if let Some((start_ms, end_ms, settings)) = timing {
            let length = lines[begin..i].iter().map(|s| s.len() + 1).sum::<usize>();
            budget.bytes(length + id.len() + 32 + std::mem::size_of::<Cue>() * 2)?;
            budget.nodes(1)?;
            let raw_text = lines[begin..i].join("\n");
            let text = cue_text(&raw_text, limits, budget)?;
            node_count = node_count
                .checked_add(text.len())
                .ok_or(Error::Limit("nodes"))?;
            if node_count > limits.nodes {
                return Err(Error::Limit("nodes"));
            }
            doc.cues.push(Cue {
                id: id.into(),
                start_ms,
                end_ms,
                settings,
                text,
            });
        } else {
            diagnostic(&mut doc, line_number, "invalid cue timings", budget)?;
        }
    }
    Ok(doc)
}

/// An external header belongs to the caller. Check the cost before cloning it,
/// including checked rules. A segment with its own header never needs this copy.
fn reserve_header_copy(header: &Header, budget: &mut Budget) -> Result<(), Error> {
    budget.bytes(std::mem::size_of::<Header>() + 64)?;
    for region in &header.regions {
        budget.nodes(1)?;
        budget.bytes(256 + region.id.len())?;
    }
    for sheet in &header.style_sheets {
        budget.nodes(1)?;
        budget.bytes(256)?;
        for rule in &sheet.rules {
            budget.nodes(1)?;
            budget.bytes(256)?;
            for selector in &rule.selectors {
                budget.nodes(1)?;
                let text = match selector {
                    css::Selector::Class(s)
                    | css::Selector::Tag(s)
                    | css::Selector::CueId(s)
                    | css::Selector::Voice(s)
                    | css::Selector::Language(s) => s.len(),
                    _ => 0,
                };
                budget.bytes(256 + text)?;
            }
            for _ in &rule.declarations {
                budget.nodes(1)?;
                // Portable text values share their strings. This also covers
                // the small length list, if the property has one.
                budget.bytes(512)?;
            }
        }
    }
    Ok(())
}

// Diagnostics are retained output too. Count both their vector slots and text
// before allocating, including when many malformed cues are skipped.
fn diagnostic(
    doc: &mut Document,
    line: usize,
    message: &str,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.nodes(1)?;
    budget.bytes(std::mem::size_of::<Diagnostic>() * 2 + message.len() + 32)?;
    doc.diagnostics.push(Diagnostic {
        line: Some(line),
        message: message.into(),
    });
    Ok(())
}

// Repeated definitions update an existing entry without copying the ID again.
// New keys are charged before allocating either their text or their map node.
fn index_region(
    index: &mut BTreeMap<String, usize>,
    id: &str,
    position: usize,
    budget: &mut Budget,
) -> Result<(), Error> {
    if let Some(previous) = index.get_mut(id) {
        *previous = position;
    } else {
        budget.bytes(256 + id.len())?;
        index.insert(id.into(), position);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_index_points_to_the_last_definition_and_is_budgeted() {
        let mut index = BTreeMap::new();
        let mut budget = Budget::new(Limits {
            output_bytes: 257,
            ..Limits::default()
        });
        index_region(&mut index, "r", 0, &mut budget).unwrap();
        // Updating a duplicate changes the position without another allocation.
        index_region(&mut index, "r", 7, &mut budget).unwrap();
        assert_eq!(index["r"], 7);
        assert_eq!(index.len(), 1);
        assert_eq!(
            index_region(&mut index, "s", 8, &mut budget),
            Err(Error::Limit("output bytes"))
        );
        assert!(!index.contains_key("s"));
    }
}
