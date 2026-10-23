use super::*;
use std::sync::Arc;
#[derive(Clone)]
pub(super) struct Context {
    pub parent: Option<usize>,
    pub base: f64,
    pub end: Option<f64>,
    pub sequence: bool,
    pub preserve: bool,
    pub language: Option<Arc<str>>,
    pub computed: crate::text::properties::Computed,
    pub region: Option<usize>,
}
pub(super) struct Reader<'a> {
    pub xml: &'a mut [Element],
    pub rates: Rates,
    pub out: Document,
    pub preserved: Vec<bool>,
    pub ids: std::collections::BTreeSet<String>,
    pub budget: &'a mut Budget,
    pub styles: styles::Styles,
    pub groups: Vec<usize>,
}
pub(super) fn space(e: &Element, parent: bool) -> Result<bool, Error> {
    match e.attr(XML, "space") {
        None => Ok(parent),
        Some("default") => Ok(false),
        Some("preserve") => Ok(true),
        _ => Err(Error::Invalid("xml:space".into())),
    }
}
pub(super) fn language(
    e: &Element,
    parent: Option<Arc<str>>,
    budget: &mut Budget,
) -> Result<Option<Arc<str>>, Error> {
    if let Some(v) = e.attr(XML, "lang") {
        budget.bytes(v.len() + 32)?;
        Ok(Some(v.into()))
    } else {
        Ok(parent)
    }
}
impl Reader<'_> {
    // In seq, the next child starts at the end time returned here.
    // None means no known end time. It does not mean the segment has ended.
    pub fn build(&mut self, i: usize, mut c: Context) -> Result<Option<f64>, Error> {
        let e = &self.xml[i];
        let group = e.ns == TT && matches!(e.name.as_str(), "body" | "div");
        let text = e.text.is_some();
        if !text && (e.ns != TT || matches!(e.name.as_str(), "metadata" | "set")) {
            // TODO: Support timed style changes from <set>. Ignore them for now.
            if e.ns != TT {
                self.budget.bytes(128 + e.name.len() + e.ns.len())?;
                self.out.diagnostics.push(crate::text::Diagnostic {
                    line: None,
                    message: format!("ignored foreign element {{{}}}{}", e.ns, e.name),
                });
            }
            return Ok(Some(c.base));
        }
        if !group && !text && !matches!(e.name.as_str(), "p" | "span" | "br") {
            return Err(Error::Unsupported(format!("IMSC element {}", e.name)));
        }
        if let Some(id) = e.attr(XML, "id") {
            self.budget.bytes(id.len() + 64)?;
            if !self.ids.insert(id.into()) {
                return Err(Error::Invalid(format!("duplicate xml:id {id}")));
            }
        }
        c.preserve = space(e, c.preserve)?;
        c.language = language(e, c.language, self.budget)?;
        let value = |name| {
            e.attr("", name)
                .map(|v| parse_time(v, self.rates))
                .transpose()
        };
        let start = c.base + value("begin")?.unwrap_or(0.0);
        let written_end = value("end")?.map(|v| c.base + v);
        let duration = value("dur")?.map(|v| start + v);
        let explicit = written_end.is_some() || duration.is_some();
        let end = [written_end, duration, c.end]
            .into_iter()
            .flatten()
            .reduce(f64::min);
        if !start.is_finite() || end.is_some_and(|v| !v.is_finite()) {
            return Err(Error::Invalid("timing overflow".into()));
        }
        let paragraph = e.name == "p" && e.ns == TT;
        let first = self.out.nodes.len();
        let kind = if paragraph {
            Kind::Paragraph
        } else if e.name == "br" {
            Kind::Break
        } else {
            Kind::Span
        };
        // Ruby has normal text and a separate pronunciation guide.
        // Text-only output includes the normal text, without the guide.
        let ruby = e.attr(TTS, "ruby");
        let mut kind = kind;
        match ruby {
            Some("container") => kind = Kind::Ruby,
            Some("text") => kind = Kind::RubyText,
            _ => {}
        }
        // TODO: Support region changes inside a paragraph.
        if group || paragraph {
            if let Some(id) = e.attr("", "region") {
                c.region = Some(
                    *self
                        .styles
                        .regions
                        .get(id)
                        .ok_or_else(|| Error::Invalid(format!("unknown region {id}")))?,
                );
            }
        }
        let local = self.styles.specified(i, self.xml, self.budget)?;
        if paragraph {
            c.computed = self.styles.region(c.region, self.xml, self.budget)?;
            let (mut region_start, mut region_end) = (start, end.unwrap_or(f64::INFINITY));
            if let Some(i) = c.region {
                let value = |name| {
                    self.xml[i]
                        .attr("", name)
                        .map(|v| parse_time(v, self.rates))
                        .transpose()
                };
                let begin = value("begin")?.unwrap_or(0.0);
                let duration = value("dur")?.map(|d| begin + d);
                region_start = region_start.max(begin);
                region_end = [Some(region_end), value("end")?, duration]
                    .into_iter()
                    .flatten()
                    .reduce(f64::min)
                    .unwrap();
            }
            let mut region = Node::new(None, Kind::Region, region_start, region_end);
            region.appearance = c.computed.appearance.clone();
            region.placement = Some(self.styles.placement(c.region, &c.computed));
            c.parent = Some(self.out.push(region, self.budget)?);
            self.preserved.resize(self.out.nodes.len(), false);
            for &g in &self.groups {
                let group = self.styles.specified(g, self.xml, self.budget)?;
                c.computed = self.styles.apply(&c.computed, &group, self.budget)?;
            }
        }
        if group {
            self.groups.push(i);
        } else {
            c.computed = self.styles.apply(&c.computed, &local, self.budget)?;
            if let Some(role) = local.iter().find_map(|p| p.ruby()) {
                kind = role;
            }
        }
        let mut parent = c.parent;
        if !group && (paragraph || parent.is_some()) {
            let mut node = Node::new(parent, kind, start, end.unwrap_or(f64::INFINITY));
            if text {
                node.text = self.xml[i].text.take();
            }
            node.language = c.language.clone();
            node.appearance = c.computed.appearance.clone();
            parent = Some(self.out.push(node, self.budget)?);
            self.preserved.resize(self.out.nodes.len(), c.preserve);
        }
        let mut children = std::mem::take(&mut self.xml[i].children);
        children.retain(|&j| {
            let e = &self.xml[j];
            !(e.ns == TT && matches!(e.name.as_str(), "metadata" | "set"))
                && e.text
                    .as_ref()
                    .is_none_or(|s| !group || !s.trim().is_empty())
        });
        let text_span =
            self.xml[i].name == "span" && children.iter().all(|&j| self.xml[j].text.is_some());
        let sequence = match self.xml[i].attr("", "timeContainer") {
            None | Some("par") => false,
            Some("seq") => !text_span,
            _ => return Err(Error::Invalid("timeContainer".into())),
        };
        let mut implicit = Some(start);
        let mut cursor = start;
        for (index, &child) in children.iter().enumerate() {
            let range = self.build(
                child,
                Context {
                    parent,
                    base: if sequence { cursor } else { start },
                    end,
                    sequence,
                    ..c.clone()
                },
            )?;
            if sequence {
                implicit = range;
                cursor = range.unwrap_or(f64::INFINITY);
                if !cursor.is_finite() && index + 1 < children.len() {
                    return Err(Error::Invalid(
                        "indefinite child before end of sequence".into(),
                    ));
                }
            } else {
                implicit = match (implicit, range) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
        }
        if group {
            self.groups.pop();
        }
        // A span containing only text has a default duration of zero in seq.
        // In par, it lasts as long as its parent. CDATA can split the stored
        // text into several nodes, but that must not change the duration.
        if text_span {
            implicit = c.sequence.then_some(start);
        } else if children.is_empty() {
            implicit =
                (c.sequence || group || paragraph || self.xml[i].name == "span").then_some(start);
        }
        let resolved = if explicit {
            end
        } else {
            match (implicit, c.end) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        };
        if let Some(end) = resolved {
            for node in &mut self.out.nodes[first..] {
                node.end = node.end.min(end);
            }
        }
        if paragraph {
            self.whitespace(first, self.out.nodes.len())?;
        }
        Ok(resolved)
    }
    fn whitespace(&mut self, first: usize, last: usize) -> Result<(), Error> {
        // Remove repeated spaces between spans too. Keep every character for
        // xml:space=preserve. Remove normal spaces at the end of a line.
        // TODO: Check spaces between spans that are active at different times.
        let (mut line_start, mut previous_space) = (true, false);
        let mut trailing: Option<usize> = None;
        let mut i = first;
        while i < last {
            if self.out.nodes[i].kind == Kind::RubyText {
                let stop = (i + 1..last)
                    .find(|&j| self.out.nodes[j].parent.is_none_or(|p| p < i))
                    .unwrap_or(last);
                // The pronunciation guide has its own spaces. It must not
                // change the spaces before or after the normal ruby text.
                self.whitespace(i + 1, stop)?;
                i = stop;
                continue;
            }
            let index = i;
            i += 1;
            let i = index;
            if self.out.nodes[i].kind == Kind::Break {
                if let Some(n) = trailing.take() {
                    self.out.nodes[n].text.as_mut().unwrap().pop();
                }
                line_start = true;
                previous_space = false;
            }
            let Some(source) = self.out.nodes[i].text.take() else {
                continue;
            };
            if source.is_empty() {
                self.out.nodes[i].text = Some(source);
                continue;
            }
            if self.preserved[i] {
                line_start = source.ends_with('\n');
                previous_space = source.ends_with([' ', '\t', '\r', '\n']);
                trailing = None;
                self.out.nodes[i].text = Some(source);
                continue;
            }
            self.budget.bytes(source.len())?;
            let mut out = String::with_capacity(source.len());
            for c in source.chars() {
                if matches!(c, ' ' | '\t' | '\r' | '\n') {
                    if !line_start && !previous_space {
                        out.push(' ');
                        trailing = Some(i);
                    }
                    previous_space = true;
                } else {
                    out.push(c);
                    line_start = false;
                    previous_space = false;
                    trailing = None;
                }
            }
            self.out.nodes[i].text = Some(out);
        }
        if let Some(n) = trailing {
            self.out.nodes[n].text.as_mut().unwrap().pop();
        }
        Ok(())
    }
}
