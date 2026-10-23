//! Resolve timing and ordinary styles directly into the shared rendering tree.
use super::render::{cascade, normalized, Layout};
use super::*;
use crate::text::{
    render::css,
    style::{Property::*, Value},
    Node, Tag,
};

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
#[derive(Clone)]
pub(super) struct Context {
    pub parent: Option<usize>,
    pub base: f64,
    pub parent_end: Option<f64>,
    pub space: Space,
    pub language: Option<Arc<str>>,
    pub region: Option<Arc<str>>,
    pub in_sequence: bool,
    pub style: Arc<Style>,
}
#[derive(PartialEq)]
enum Kind {
    Group,
    Paragraph,
    Span,
    Break,
    Text,
}

pub(super) struct Builder<'a> {
    pub arena: &'a mut [Element],
    pub styles: &'a BTreeMap<String, Arc<Style>>,
    pub regions: &'a BTreeMap<String, Region>,
    pub empty: &'a Arc<Style>,
    pub rates: Rates,
    pub layout: Layout,
    pub groups: Vec<Arc<Style>>,
    pub out: crate::text::Document,
    pub budget: &'a mut Budget,
}
impl Builder<'_> {
    pub fn build(&mut self, i: usize, mut context: Context) -> Result<Interval, Error> {
        let e = &self.arena[i];
        let kind = if e.text.is_some() {
            Kind::Text
        } else {
            if e.ns != TT {
                self.budget.bytes(128 + e.ns.len() + e.name.len())?;
                self.out.diagnostics.push(Diagnostic {
                    line: None,
                    message: format!("ignored foreign element {{{}}}{}", e.ns, e.name),
                });
                return Ok(Interval {
                    begin: context.base,
                    end: Some(context.base),
                });
            }
            match e.name.as_str() {
                "body" | "div" => Kind::Group,
                "p" => Kind::Paragraph,
                "span" => Kind::Span,
                "br" => Kind::Break,
                _ => return Err(Error::Unsupported(format!("IMSC element {}", e.name))),
            }
        };
        context.space = space(e, context.space)?;
        context.language = inherited_text(e.attr(XML, "lang"), context.language, self.budget)?;
        // TODO: Split a paragraph across regions. Inline spans use its region for now.
        if context.parent.is_none() {
            context.region = inherited_text(e.attr("", "region"), context.region, self.budget)?;
        }
        let region = context
            .region
            .as_ref()
            .map(|id| {
                self.regions
                    .get(id.as_ref())
                    .ok_or_else(|| Error::Invalid(format!("unknown region {id}")))
            })
            .transpose()?;
        let (interval, explicit) = timing(e, self.rates, context.base, context.parent_end)?;
        let local = specified_style(e, self.styles, self.empty, self.budget)?;
        let first = self.out.nodes.len();
        let mut parent = context.parent;
        if kind == Kind::Paragraph {
            let start = interval.begin.max(region.map_or(0.0, |r| r.interval.begin));
            let end = interval
                .end
                .unwrap_or(f64::INFINITY)
                .min(region.and_then(|r| r.interval.end).unwrap_or(f64::INFINITY));
            let mut style = region.map_or_else(
                || Ok(self.layout.base.clone()),
                |r| {
                    cascade(
                        &self.layout.base,
                        &r.style,
                        self.layout.pixels,
                        false,
                        self.budget,
                    )
                },
            )?;
            parent = Some(self.out.push(
                Node {
                    parent: None,
                    tag: Tag::Div,
                    start,
                    end,
                    style: self.layout.region(region, &style, self.budget)?,
                    language: None,
                    text: None,
                },
                self.budget,
            )?);
            style.properties.remove(&Opacity); // The region wrapper applies it.
            for group in &self.groups {
                style = cascade(&style, group, self.layout.pixels, true, self.budget)?;
            }
            context.style = Arc::new(style);
        }
        if kind == Kind::Group {
            self.groups.push(local);
        } else if kind == Kind::Paragraph || parent.is_some() {
            context.style = Arc::new(cascade(
                &context.style,
                &local,
                self.layout.pixels,
                kind == Kind::Paragraph,
                self.budget,
            )?);
            let mut style = css(&context.style, self.budget)?;
            self.budget.bytes(512)?;
            let no_wrap = matches!(context.style.properties.get(&WrapOption),Some(Value::Keyword(s)) if s.as_ref()=="noWrap");
            let white_space = match (context.space, no_wrap) {
                (Space::Default, false) => "normal",
                (Space::Default, true) => "nowrap",
                (Space::Preserve, false) => "pre-wrap",
                (Space::Preserve, true) => "pre",
            };
            style.insert("white-space", white_space.into());
            style.remove("justify-content");
            if kind != Kind::Text {
                style.remove("text-decoration");
            }
            if kind == Kind::Paragraph {
                style.insert("margin", "0".into());
            }
            let tag = match kind {
                Kind::Paragraph => Tag::P,
                Kind::Break => Tag::Br,
                _ => Tag::Span,
            };
            let text = self.arena[i]
                .text
                .take()
                .map(|s| normalized(s, context.space == Space::Preserve, self.budget))
                .transpose()?;
            parent = Some(self.out.push(
                Node {
                    parent,
                    tag,
                    start: interval.begin,
                    end: interval.end.unwrap_or(f64::INFINITY),
                    style,
                    language: context.language.clone(),
                    text,
                },
                self.budget,
            )?);
        }
        let mut children = std::mem::take(&mut self.arena[i].children);
        // TODO: Support timed <set> styling if usual content needs it.
        children.retain(|&child| {
            let e = &self.arena[child];
            !(e.ns == TT && matches!(e.name.as_str(), "set" | "metadata"))
                && e.text
                    .as_ref()
                    .is_none_or(|s| kind != Kind::Group || !s.trim().is_empty())
        });
        let text_only_span =
            kind == Kind::Span && children.iter().all(|&c| self.arena[c].text.is_some());
        let sequence = match self.arena[i].attr("", "timeContainer") {
            None | Some("par") => false,
            Some("seq") => !text_only_span,
            _ => return Err(Error::Invalid("timeContainer".into())),
        };
        let mut implicit = Some(interval.begin);
        let mut cursor = interval.begin;
        for &child in &children {
            let range = self.build(
                child,
                Context {
                    parent,
                    base: if sequence { cursor } else { interval.begin },
                    parent_end: interval.end,
                    in_sequence: sequence,
                    ..context.clone()
                },
            )?;
            if sequence {
                cursor = range.end.unwrap_or(f64::INFINITY);
                implicit = range.end;
                if !cursor.is_finite() && Some(&child) != children.last() {
                    return Err(Error::Invalid(
                        "indefinite element before end of seq container".into(),
                    ));
                }
            } else {
                implicit = match (implicit, range.end) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
        }
        if kind == Kind::Group {
            self.groups.pop();
        }
        if text_only_span {
            implicit = context.in_sequence.then_some(interval.begin);
        } else if children.is_empty() {
            implicit = (context.in_sequence
                || matches!(kind, Kind::Group | Kind::Paragraph | Kind::Span))
            .then_some(interval.begin);
        }
        let end = if explicit {
            interval.end
        } else {
            match (implicit, context.parent_end) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        };
        if let Some(end) = end {
            for n in &mut self.out.nodes[first..] {
                n.end = n.end.min(end);
            }
        }
        Ok(Interval {
            begin: interval.begin,
            end,
        })
    }
}
fn inherited_text(
    value: Option<&str>,
    parent: Option<Arc<str>>,
    budget: &mut Budget,
) -> Result<Option<Arc<str>>, Error> {
    if let Some(value) = value {
        budget.bytes(value.len() + 32)?;
        Ok(Some(value.into()))
    } else {
        Ok(parent)
    }
}
