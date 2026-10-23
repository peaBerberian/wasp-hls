use super::*;
use std::sync::Arc;
struct Frame<'a> {
    tag: &'a str,
    parent: usize,
    language: Option<Arc<str>>,
    computed: crate::text::properties::Computed,
}
pub(super) struct Styles<'a> {
    pub rules: &'a [super::css::Rule],
    pub computed: crate::text::properties::Computed,
}
pub(super) fn parse(
    source: &str,
    parent: usize,
    out: &mut Document,
    limits: Limits,
    budget: &mut Budget,
    styles: Styles<'_>,
) -> Result<(), Error> {
    let (start, end) = (out.nodes[parent].start, out.nodes[parent].end);
    let mut stack = vec![Frame {
        tag: "",
        parent,
        language: None,
        computed: styles.computed,
    }];
    let mut tail = source;
    while !tail.is_empty() {
        if let Some(after) = tail.strip_prefix('<') {
            let (token, rest) = after.split_once('>').unwrap_or((after, ""));
            tail = rest;
            if let Some(close) = token.strip_prefix('/') {
                if stack.len() > 1 && stack.last().unwrap().tag == close {
                    stack.pop();
                } else if close == "ruby" && stack.last().unwrap().tag == "rt" {
                    stack.pop();
                    if stack.last().unwrap().tag == "ruby" {
                        stack.pop();
                    }
                }
                continue;
            }
            let split = token
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(token.len());
            let mut classes = token[..split].split('.');
            let tag = classes.next().unwrap_or("");
            if !matches!(tag, "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt") {
                continue;
            }
            if tag == "rt" && stack.last().unwrap().tag != "ruby" {
                continue;
            }
            if stack.len() >= limits.depth {
                return Err(Error::Limit("markup depth"));
            }
            let previous = stack.last().unwrap();
            let mut frame = Frame {
                tag,
                parent: previous.parent,
                language: previous.language.clone(),
                computed: previous.computed.child(),
            };
            if tag == "lang" {
                budget.bytes(token[split..].len().saturating_mul(4) + 32)?;
                frame.language = Some(entities::decode(token[split..].trim()).into());
            }
            if matches!(tag, "ruby" | "rt" | "lang") {
                let kind = match tag {
                    "ruby" => Kind::Ruby,
                    "rt" => Kind::RubyText,
                    _ => Kind::Span,
                };
                let mut n = Node::new(Some(previous.parent), kind, start, end);
                n.language = frame.language.clone();
                n.appearance = frame.computed.appearance.clone();
                frame.parent = out.push(n, budget)?;
            }
            // Use cue IDs, classes and voice names to find the styles here.
            // Return the resulting appearance, without that source data.
            budget.bytes(
                classes.clone().count() * std::mem::size_of::<&str>()
                    + token[split..].len().saturating_mul(4),
            )?;
            let decoded = if matches!(tag, "v" | "lang") {
                entities::decode(token[split..].trim())
            } else {
                String::new()
            };
            let mut annotation = String::with_capacity(decoded.len());
            for word in decoded.split_ascii_whitespace() {
                if !annotation.is_empty() {
                    annotation.push(' ');
                }
                annotation.push_str(word);
            }
            css::apply(
                styles.rules,
                css::Target::Inline {
                    tag,
                    classes: classes.collect(),
                    annotation: &annotation,
                },
                &mut frame.computed,
                budget,
            )?;
            if !matches!(tag, "ruby" | "rt" | "lang") {
                let mut n = Node::new(Some(previous.parent), Kind::Span, start, end);
                n.language = frame.language.clone();
                n.appearance = frame.computed.appearance.clone();
                frame.parent = out.push(n, budget)?;
            } else {
                out.nodes[frame.parent].appearance = frame.computed.appearance.clone();
            }
            stack.push(frame);
        } else {
            let end_text = tail.find('<').unwrap_or(tail.len());
            let frame = stack.last().unwrap();
            budget.bytes(end_text.saturating_mul(4))?;
            let mut n = Node::new(Some(frame.parent), Kind::Span, start, end);
            n.text = Some(entities::decode(&tail[..end_text]));
            n.language = frame.language.clone();
            n.appearance = frame.computed.appearance.clone();
            out.push(n, budget)?;
            tail = &tail[end_text..];
        }
    }
    // TODO: Use timestamps inside a cue to highlight words as time passes.
    Ok(())
}
