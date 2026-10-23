//! Read a small set of WebVTT CSS rules. Apply them during parsing.
use crate::text::{
    budget::Budget,
    properties::{self, Computed, Property},
    Error, Limits,
};
#[derive(Debug, Clone)]
enum Selector {
    All,
    Id(String),
    Class(String),
    Tag(String),
    Annotation(&'static str, String),
    Region,
}
#[derive(Debug, Clone)]
pub(super) struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<(Property, bool)>,
}
pub(super) enum Target<'a> {
    Root(&'a str),
    Inline {
        tag: &'a str,
        classes: Vec<&'a str>,
        annotation: &'a str,
    },
    Region,
}
#[cfg(test)]
std::thread_local! {static SCANNED:std::cell::Cell<usize>=const {std::cell::Cell::new(0)};}

// Read characters only up to this separator. Reading the whole file again
// for each rule would be slow. Ignore separators inside quotes or brackets.
fn cut(s: &str, delimiter: char, limit: usize) -> Result<(&str, Option<&str>), Error> {
    let (mut quote, mut escaped, mut depth) = (None, false, 0usize);
    for (i, c) in s.char_indices() {
        #[cfg(test)]
        SCANNED.with(|n| n.set(n.get() + 1));
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        if matches!(c, '\'' | '"') {
            quote = Some(c);
            continue;
        }
        if c == delimiter && depth == 0 {
            return Ok((&s[..i], Some(&s[i + c.len_utf8()..])));
        }
        if matches!(c, '(' | '[' | '{') {
            if depth >= limit {
                return Err(Error::Limit("CSS depth"));
            }
            depth += 1;
        } else if matches!(c, ')' | ']' | '}') {
            depth = depth.saturating_sub(1);
        }
    }
    Ok((s, None))
}
fn comments(s: &str) -> String {
    let (mut quote, mut escaped) = (None, false);
    let mut out = String::new();
    let mut tail = s;
    while !tail.is_empty() {
        if quote.is_none() && tail.starts_with("/*") {
            tail = tail[2..].find("*/").map_or("", |n| &tail[n + 4..]);
            out.push(' ');
            continue;
        }
        let c = tail.chars().next().unwrap();
        tail = &tail[c.len_utf8()..];
        out.push(c);
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && matches!(c, '\'' | '"') {
            quote = Some(c);
        }
    }
    out
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
}
fn selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    if s == "::cue" {
        return Some(Selector::All);
    }
    if s == "::cue-region" {
        return Some(Selector::Region);
    }
    let inner = s.strip_prefix("::cue(")?.strip_suffix(')')?;
    if let Some(v) = inner.strip_prefix('.') {
        return identifier(v).then(|| Selector::Class(v.into()));
    }
    if let Some(v) = inner.strip_prefix('#') {
        return identifier(v).then(|| Selector::Id(v.into()));
    }
    if matches!(inner, "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt") {
        return Some(Selector::Tag(inner.into()));
    }
    for (prefix, tag) in [("v[voice=", "v"), ("lang[lang=", "lang")] {
        if let Some(v) = inner.strip_prefix(prefix).and_then(|v| v.strip_suffix(']')) {
            let v = v
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
            return (!v.contains('\\')).then(|| Selector::Annotation(tag, v.into()));
        }
    }
    // TODO: Compound selectors, nested rules and :past/:future highlighting.
    None
}
pub(super) fn parse(source: &str, limits: Limits, budget: &mut Budget) -> Result<Vec<Rule>, Error> {
    budget.bytes(source.len() + 128)?;
    let clean = comments(source);
    let mut tail = clean.as_str();
    let mut rules = Vec::new();
    while !tail.is_empty() {
        let (selectors, body) = cut(tail, '{', limits.depth)?;
        let Some(body) = body else {
            break;
        };
        if limits.depth == 0 {
            return Err(Error::Limit("CSS depth"));
        }
        let (body, rest) = cut(body, '}', limits.depth - 1)?;
        let Some(rest) = rest else {
            break;
        };
        tail = rest;
        if selectors.trim_start().starts_with('@') {
            continue;
        }
        budget.nodes(1)?;
        budget.bytes(256)?;
        let mut selected = Vec::new();
        let mut parts = selectors;
        while !parts.is_empty() {
            let (part, rest) = cut(parts, ',', limits.depth)?;
            parts = rest.unwrap_or("");
            budget.nodes(1)?;
            budget.bytes(128 + part.len())?;
            if let Some(s) = selector(part) {
                selected.push(s);
            }
        }
        let mut declarations = Vec::new();
        let mut parts = body;
        while !parts.is_empty() {
            let (part, rest) = cut(parts, ';', limits.depth)?;
            parts = rest.unwrap_or("");
            let Some((name, value)) = part.split_once(':') else {
                continue;
            };
            budget.bytes(part.len())?;
            let name = name.trim().to_ascii_lowercase();
            if !identifier(&name) {
                continue;
            }
            let mut value = value.trim();
            let mut important = false;
            if let Some((before, after)) = value.rsplit_once('!') {
                if after.trim().eq_ignore_ascii_case("important") {
                    value = before.trim();
                    important = true;
                }
            }
            budget.nodes(1)?;
            if let Some(p) = properties::parse(&name, value, true, budget)? {
                declarations.push((p, important));
            }
        }
        if !selected.is_empty() && !declarations.is_empty() {
            rules.push(Rule {
                selectors: selected,
                declarations,
            });
        }
    }
    Ok(rules)
}
fn specificity(s: &Selector, t: &Target<'_>) -> Option<u8> {
    match (s, t) {
        (Selector::All, Target::Root(_)) | (Selector::Region, Target::Region) => Some(0),
        (Selector::Id(id), Target::Root(other)) if id == other => Some(100),
        (Selector::Class(c), Target::Inline { classes, .. }) if classes.contains(&c.as_str()) => {
            Some(10)
        }
        (Selector::Tag(n), Target::Inline { tag, .. }) if n == tag => Some(1),
        (
            Selector::Annotation(n, v),
            Target::Inline {
                tag, annotation, ..
            },
        ) if n == tag && v == annotation => Some(11),
        _ => None,
    }
}
pub(super) fn apply(
    rules: &[Rule],
    target: Target<'_>,
    computed: &mut Computed,
    budget: &mut Budget,
) -> Result<(), Error> {
    let mut values = Vec::new();
    if matches!(target, Target::Root(_)) {
        properties::put(&mut values, Property::Background([0, 0, 0, 204]));
    }
    if let Target::Inline {
        tag, ref classes, ..
    } = target
    {
        let builtin = match tag {
            "b" => Some(Property::Weight(700.0)),
            "i" => Some(Property::FontStyle(crate::text::FontStyle::Italic)),
            "u" => Some(Property::Decoration(1, 0)),
            "rt" => Some(Property::Size(crate::text::Length::Em(0.5))),
            _ => None,
        };
        if let Some(p) = builtin {
            properties::put(&mut values, p);
        }
        for c in classes {
            let (background, name) = c.strip_prefix("bg_").map_or((false, *c), |s| (true, s));
            if [
                "white", "lime", "cyan", "red", "yellow", "magenta", "blue", "black",
            ]
            .contains(&name)
            {
                if let Some(p) = properties::parse(
                    if background {
                        "background-color"
                    } else {
                        "color"
                    },
                    name,
                    true,
                    budget,
                )? {
                    properties::put(&mut values, p);
                }
            }
        }
    }
    // IDs have priority over classes; classes have priority over tag names.
    // !important has priority over those scores. Later equal scores win.
    let mut chosen: Vec<(bool, u8, &Property)> = Vec::new();
    for rule in rules {
        let score = rule
            .selectors
            .iter()
            .filter_map(|s| specificity(s, &target))
            .max();
        if let Some(score) = score {
            for (p, important) in &rule.declarations {
                if let Some(old) = chosen
                    .iter_mut()
                    .find(|(_, _, old)| properties::same(old, p))
                {
                    if (*important, score) >= (old.0, old.1) {
                        *old = (*important, score, p);
                    }
                } else {
                    chosen.push((*important, score, p));
                }
            }
        }
    }
    budget.bytes((chosen.len() + values.len()) * 256)?;
    for (_, _, p) in chosen {
        properties::put(&mut values, p.clone());
    }
    *computed = computed.apply(&values, None);
    Ok(())
}
pub(super) fn reserve(rules: &[Rule], budget: &mut Budget) -> Result<(), Error> {
    // The font list is shared. Selector strings and small vectors are copied.
    for rule in rules {
        budget.nodes(1 + rule.selectors.len() + rule.declarations.len())?;
        budget.bytes(256 + rule.declarations.len() * 128)?;
        for s in &rule.selectors {
            let text = match s {
                Selector::Id(s)
                | Selector::Class(s)
                | Selector::Tag(s)
                | Selector::Annotation(_, s) => s.len(),
                _ => 0,
            };
            budget.bytes(128 + text)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scanning_grows_with_input_and_nested_rules_cannot_escape() {
        for count in [500, 1000, 2000] {
            let source = "::cue { color:red }".repeat(count);
            SCANNED.with(|n| n.set(0));
            let rules = parse(
                &source,
                Limits::default(),
                &mut Budget::new(Limits::default()),
            )
            .unwrap();
            assert_eq!(rules.len(), count);
            assert!(SCANNED.with(|n| n.get()) <= source.len() * 8);
        }
        let mut budget = Budget::new(Limits::default());
        let rules = parse(
            "@media screen { ::cue {color:red} } ::cue {color:blue}",
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        let mut computed = Computed::vtt();
        apply(&rules, Target::Root(""), &mut computed, &mut budget).unwrap();
        assert_eq!(computed.appearance.color, [0, 0, 255, 255]);
        assert!(parse(
            "@media x { ::cue {color:red} }",
            Limits {
                depth: 1,
                ..Limits::default()
            },
            &mut Budget::new(Limits::default())
        )
        .is_err());
    }
}
