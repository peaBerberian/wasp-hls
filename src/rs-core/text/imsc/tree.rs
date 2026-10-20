//! Resolve timed hierarchy while retaining specified styles and whitespace.
use super::*;

pub(super) fn space(e: &Element, parent: Space) -> Result<Space, Error> {
    match e.attr(XML, "space") {
        None => Ok(parent),
        Some("default") => Ok(Space::Default),
        Some("preserve") => Ok(Space::Preserve),
        _ => Err(Error::Invalid("xml:space".into())),
    }
}
pub(super) fn timing(
    e: &Element,
    r: Rates,
    base: f64,
    parent_end: Option<f64>,
) -> Result<(Interval, bool), Error> {
    let value = |name| e.attr("", name).map(|v| parse_time(v, r)).transpose();
    let begin = base + value("begin")?.unwrap_or(0.0);
    let end = value("end")?.map(|v| base + v);
    let dur = value("dur")?.map(|v| begin + v);
    let explicit = end.is_some() || dur.is_some();
    let end = [end, dur, parent_end]
        .into_iter()
        .flatten()
        .reduce(f64::min);
    if !begin.is_finite() || end.is_some_and(|v| !v.is_finite()) {
        return Err(Error::Invalid("timing overflow".into()));
    }
    Ok((Interval { begin, end }, explicit))
}
pub(super) struct Context {
    pub parent: Option<usize>,
    pub base: f64,
    pub parent_end: Option<f64>,
    pub parent_space: Space,
    pub parent_language: Option<Arc<str>>,
    pub parent_region: Option<Arc<str>>,
    pub in_sequence: bool,
}
pub(super) fn build(
    i: usize,
    arena: &[Element],
    doc: &mut Document,
    context: Context,
    empty_style: &Arc<Style>,
    budget: &mut Budget,
) -> Result<Interval, Error> {
    let Context {
        parent,
        base,
        parent_end,
        parent_space,
        parent_language,
        parent_region,
        in_sequence,
    } = context;
    let e = &arena[i];
    let kind = if e.text.is_some() {
        Kind::Text
    } else {
        if e.ns != TT {
            budget.nodes(1)?;
            budget.bytes(128 + e.ns.len() + e.name.len())?;
            doc.diagnostics.push(Diagnostic {
                line: None,
                message: format!("ignored foreign element {{{}}}{}", e.ns, e.name),
            });
            return Ok(Interval {
                begin: base,
                end: Some(base),
            });
        }
        match e.name.as_str() {
            "body" => Kind::Body,
            "div" => Kind::Division,
            "p" => Kind::Paragraph,
            "span" => Kind::Span,
            "br" => Kind::Break,
            "set" => Kind::Set,
            "metadata" => {
                return Ok(Interval {
                    begin: base,
                    end: Some(base),
                })
            }
            _ => return Err(Error::Unsupported(format!("IMSC element {}", e.name))),
        }
    };
    let space = space(e, parent_space)?;
    let language = inherited_text(e.attr(XML, "lang"), parent_language, budget)?;
    let region = inherited_text(e.attr("", "region"), parent_region, budget)?;
    if let Some(id) = &region {
        if !doc.regions.contains_key(id.as_ref()) {
            return Err(Error::Invalid(format!("unknown region {id}")));
        }
    }
    let (interval, explicit) = timing(e, doc.rates, base, parent_end)?;
    let index = doc.nodes.len();
    if let Some(p) = parent {
        doc.nodes[p].children.push(index);
    }
    budget.nodes(1)?;
    budget.bytes(
        std::mem::size_of::<Node>() * 2
            + 32
            + e.attr(XML, "id").map_or(0, str::len)
            + e.text.as_ref().map_or(0, String::len),
    )?;
    doc.nodes.push(Node {
        parent,
        children: Vec::new(),
        kind,
        id: e.attr(XML, "id").map(String::from),
        interval,
        region: region.clone(),
        language: language.clone(),
        space,
        style: specified_style(e, &doc.styles, empty_style, budget)?,
        text: e.text.clone(),
    });
    let sequence = match e.attr("", "timeContainer") {
        None | Some("par") => false,
        Some("seq") => true,
        _ => return Err(Error::Invalid("timeContainer".into())),
    };
    let mut implicit = Some(interval.begin);
    let mut cursor = interval.begin;
    budget.bytes(e.children.len() * std::mem::size_of::<usize>())?;
    let children: Vec<_> = e
        .children
        .iter()
        .copied()
        .filter(|i| {
            let e = &arena[*i];
            // Indentation outside paragraphs/spans has no presentation semantics.
            e.text.as_ref().is_none_or(|t| {
                matches!(kind, Kind::Paragraph | Kind::Span) || !t.trim().is_empty()
            })
        })
        .collect();
    let text_only_span =
        kind == Kind::Span && children.iter().all(|&child| arena[child].text.is_some());
    // Text children are storage details of a timing leaf. Do not give them
    // separate sequential timing, even if the span itself says timeContainer.
    let sequence = sequence && !text_only_span;
    for child in children.iter().copied() {
        let child_interval = build(
            child,
            arena,
            doc,
            Context {
                parent: Some(index),
                base: if sequence { cursor } else { interval.begin },
                parent_end: interval.end,
                parent_space: space,
                parent_language: language.clone(),
                parent_region: region.clone(),
                in_sequence: sequence,
            },
            empty_style,
            budget,
        )?;
        if sequence {
            cursor = child_interval.end.unwrap_or(f64::INFINITY);
            implicit = child_interval.end;
            if !cursor.is_finite() && Some(child) != children.last().copied() {
                return Err(Error::Invalid(
                    "indefinite element before end of seq container".into(),
                ));
            }
        } else {
            implicit = match (implicit, child_interval.end) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
        }
    }
    // TTML treats a span containing only text as a timing leaf, even though
    // our output keeps separate text nodes. Its parent container decides the
    // implicit duration: zero in seq, indefinite in par.
    if text_only_span {
        implicit = if in_sequence {
            Some(interval.begin)
        } else {
            None
        };
    } else if children.is_empty() {
        implicit = if in_sequence
            || matches!(
                kind,
                Kind::Body | Kind::Division | Kind::Paragraph | Kind::Span
            ) {
            Some(interval.begin)
        } else {
            None
        };
    }
    if !explicit {
        doc.nodes[index].interval.end = match (implicit, parent_end) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
    let resolved = doc.nodes[index].interval;
    // Descendants cannot remain active beyond an implicit container end.
    clip(index, resolved.end, &mut doc.nodes);
    Ok(resolved)
}
fn clip(index: usize, end: Option<f64>, nodes: &mut [Node]) {
    if let Some(end) = end {
        nodes[index].interval.end = Some(nodes[index].interval.end.map_or(end, |v| v.min(end)));
        // Copy one index at a time instead of allocating a temporary vector
        // at every ancestor while clipping a large subtree.
        for position in 0..nodes[index].children.len() {
            let child = nodes[index].children[position];
            clip(child, Some(end), nodes);
        }
    }
}

// Language and region IDs can be repeated through a deep tree. Share inherited
// text instead of allocating another copy each time a child receives it.
fn inherited_text(
    value: Option<&str>,
    parent: Option<Arc<str>>,
    budget: &mut Budget,
) -> Result<Option<Arc<str>>, Error> {
    if let Some(value) = value {
        budget.bytes(value.len() + 32)?;
        Ok(Some(Arc::from(value)))
    } else {
        Ok(parent)
    }
}
