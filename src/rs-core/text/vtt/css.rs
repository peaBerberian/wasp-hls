//! Read the supported WebVTT CSS rules. Unknown syntax and values are skipped.
use super::super::{budget::Budget, render::css_value, Error, Limits};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// ::cue, which selects all cue text.
    Cue,
    /// ::cue(.name), matched against a cue-text class.
    Class(String),
    /// ::cue(b), ::cue(i), and other supported cue-text tags.
    Tag(String),
    /// ::cue(#id), matched against a cue ID.
    CueId(String),
    /// A voice annotation, such as ::cue(v[voice="Alice"]).
    Voice(String),
    /// A language annotation, such as ::cue(lang[lang="fr"]).
    Language(String),
    /// ::cue-region, which selects all cue regions.
    Region,
}
#[derive(Debug, Clone, PartialEq)]
/// One CSS property/value pair. Order is kept because later rules may replace
/// earlier values. No rule matching or priority calculation is done here.
pub struct Declaration {
    pub property: &'static str,
    pub value: Arc<str>,
    pub important: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}
#[derive(Debug, Clone, Default, PartialEq)]
/// Checked rules needed by a segment or its initialization header.
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

// Count scanner work in tests rather than checking elapsed time. This makes the
// linear-work regression test stable on slow machines and under other tests.
#[cfg(test)]
std::thread_local! { static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// Split outside strings, parentheses and brackets. The same scanner handles
/// delimiters inside font names, selector annotations and functional values.
/// Yield one part at a time. Stop as soon as the delimiter is found, so the
/// stylesheet reader never scans later rules while looking for the next `{`.
fn split(mut source: &str, delimiter: char) -> impl Iterator<Item = &str> {
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let mut quote = None;
        let mut escape = false;
        let mut depth = 0usize;
        for (index, c) in source.char_indices() {
            #[cfg(test)]
            SCANNED.with(|n| n.set(n.get() + 1));
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
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
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if c == delimiter && depth == 0 {
                let part = &source[..index];
                source = &source[index + c.len_utf8()..];
                return Some(part);
            }
        }
        done = true;
        Some(source)
    })
}
fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut quote = None;
    let mut escaped = false;
    while !rest.is_empty() {
        if quote.is_none() && rest.starts_with("/*") {
            rest = rest[2..].find("*/").map_or("", |end| &rest[end + 4..]);
            out.push(' ');
            continue;
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
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
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        }
    }
    out
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
fn selector(s: &str) -> Option<Selector> {
    let s = s.trim_matches(|c: char| c.is_ascii_whitespace());
    if s == "::cue" {
        return Some(Selector::Cue);
    }
    if s == "::cue-region" {
        return Some(Selector::Region);
    }
    if let Some(inner) = s.strip_prefix("::cue(").and_then(|s| s.strip_suffix(')')) {
        if let Some(class) = inner.strip_prefix('.') {
            if identifier(class) {
                return Some(Selector::Class(class.into()));
            }
        }
        if let Some(id) = inner.strip_prefix('#') {
            if identifier(id) {
                return Some(Selector::CueId(id.into()));
            }
        }
        if matches!(inner, "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt") {
            return Some(Selector::Tag(inner.into()));
        }
        for (prefix, voice) in [("v[voice=", true), ("lang[lang=", false)] {
            if let Some(v) = inner.strip_prefix(prefix).and_then(|s| s.strip_suffix(']')) {
                if let Some(v) = v
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                {
                    if !v.contains('\\') {
                        return Some(if voice {
                            Selector::Voice(v.into())
                        } else {
                            Selector::Language(v.into())
                        });
                    }
                }
            }
        }
    }
    None
}
fn declaration(s: &str) -> Option<Declaration> {
    let name = split(s, ':')
        .next()?
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .to_ascii_lowercase();
    if !identifier(&name) {
        return None;
    }
    let colon = s.find(':')?;
    let raw = s[colon + 1..].trim_matches(|c: char| c.is_ascii_whitespace());
    let (value, important) = if let Some(at) = raw.rfind('!') {
        if raw[at + 1..]
            .trim_matches(|c: char| c.is_ascii_whitespace())
            .eq_ignore_ascii_case("important")
        {
            (
                raw[..at].trim_matches(|c: char| c.is_ascii_whitespace()),
                true,
            )
        } else {
            (raw, false)
        }
    } else {
        (raw, false)
    };
    if value.is_empty() {
        return None;
    }
    let (property, value) = super::style_values::parse(&name, value)?;
    let (property, value) = css_value(&property, &value)?;
    Some(Declaration {
        property,
        value,
        important,
    })
}
pub(super) fn parse(
    source: &str,
    limits: Limits,
    budget: &mut Budget,
) -> Result<Stylesheet, Error> {
    budget.bytes(source.len() + 128)?;
    let mut result = Stylesheet::default();
    let clean = strip_comments(source);
    let mut rest = clean.as_str();
    loop {
        let open = split(rest, '{').next().unwrap_or(rest).len();
        if open == rest.len() {
            break;
        }
        if limits.depth == 0 {
            return Err(Error::Limit("CSS depth"));
        }
        budget.nodes(1)?;
        budget.bytes(256)?;
        let selector_text = rest[..open].trim_matches(|c: char| c.is_ascii_whitespace());
        let body = &rest[open + 1..];
        let mut quote = None;
        let mut escape = false;
        let mut depth = 1usize;
        let mut end = None;
        for (i, c) in body.char_indices() {
            #[cfg(test)]
            SCANNED.with(|n| n.set(n.get() + 1));
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
                continue;
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
                continue;
            }
            if c == '{' {
                if depth >= limits.depth {
                    return Err(Error::Limit("CSS depth"));
                }
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
        }
        let Some(end) = end else {
            return Ok(result);
        };
        let body = &body[..end];
        // TODO: Support compound selectors and nested rules if usual cues need them.
        if !selector_text.starts_with('@') && !body.contains('{') {
            let mut selectors = Vec::new();
            for part in split(selector_text, ',') {
                budget.nodes(1)?;
                budget.bytes(256 + part.len() * 2)?;
                if let Some(s) = selector(part) {
                    selectors.push(s);
                }
            }
            let mut declarations = Vec::new();
            for part in split(body, ';')
                .filter(|s| !s.trim_matches(|c: char| c.is_ascii_whitespace()).is_empty())
            {
                budget.nodes(1)?;
                // Reserve temporary declaration parsing and checked values.
                budget.bytes(512 + part.len() * 4)?;
                if let Some(value) = declaration(part) {
                    declarations.push(value);
                }
            }
            if !selectors.is_empty() && !declarations.is_empty() {
                result.rules.push(Rule {
                    selectors,
                    declarations,
                });
            }
        }
        rest = &rest[open + end + 2..];
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(source: &str) -> Stylesheet {
        let limits = Limits::default();
        super::parse(source, limits, &mut Budget::new(limits)).unwrap()
    }
    #[test]
    fn rules_keep_order_and_importance_and_drop_unknown_values() {
        let s=parse("/* header */ ::cue(.red), ::cue(v[voice=\"A,B\"]) { color: red; color: blue !important; font-size: 2em; future: value; }");
        assert_eq!(
            s.rules[0].selectors,
            vec![Selector::Class("red".into()), Selector::Voice("A,B".into())]
        );
        let d = &s.rules[0].declarations;
        assert_eq!(d.len(), 3);
        assert!(!d[0].important);
        assert!(d[1].important);
        assert_eq!(
            (&d[0].property, &d[0].value),
            (&"color", &Arc::<str>::from("#ff0000ff"))
        );
    }
    #[test]
    fn strings_comments_and_unsupported_constructs() {
        let source="::cue { font-family: 'a;/*b*/:c'; } @media screen { ::cue { color: red } } ::cue(.a .b) { color: rgb(1,2,3); }";
        let s = parse(source);
        assert_eq!(s.rules.len(), 1);
        assert_eq!(
            s.rules[0].declarations[0].value,
            Arc::<str>::from("'a;/*b*/:c'")
        );
    }
    #[test]
    fn scanner_work_grows_with_the_input() {
        for count in [500, 1000, 2000] {
            let source = "::cue { color: red; }".repeat(count);
            SCANNED.with(|n| n.set(0));
            let result = parse(&source);
            assert_eq!(result.rules.len(), count);
            let scanned = SCANNED.with(|n| n.get());
            assert!(
                scanned <= source.len() * 8,
                "{count} rules scanned {scanned} characters"
            );
        }
    }
    #[test]
    fn rules_selectors_declarations_and_depth_have_limits() {
        for source in [
            "::cue {} ::cue {} ::cue {}",
            "::cue, ::cue(.x), ::cue(.y) {}",
            "::cue {color:red;color:blue;color:white}",
        ] {
            let limits = Limits {
                nodes: 3,
                ..Limits::default()
            };
            assert!(matches!(
                super::parse(source, limits, &mut Budget::new(limits)),
                Err(Error::Limit("output nodes"))
            ));
        }
        let limits = Limits {
            depth: 2,
            ..Limits::default()
        };
        assert!(matches!(
            super::parse(
                "@a { @b { ::cue { color:red } } }",
                limits,
                &mut Budget::new(limits)
            ),
            Err(Error::Limit("CSS depth"))
        ));
    }
    #[test]
    fn stylesheet_copies_are_checked_before_allocation() {
        let limits = Limits {
            output_bytes: 10,
            ..Limits::default()
        };
        assert!(matches!(
            super::parse("::cue {color:red}", limits, &mut Budget::new(limits)),
            Err(Error::Limit("output bytes"))
        ));
    }
}
