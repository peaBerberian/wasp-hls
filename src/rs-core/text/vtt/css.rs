//! Read a small set of WebVTT CSS rules and keep the original CSS too.
//! Unknown selectors, nested rules and unknown property values stay available
//! as text. This parser does not apply rules or calculate final styles.
use super::super::{budget::Budget, style::Value, Error, Limits};

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
    /// Preserved rather than guessed; matching is not performed by this parser.
    Unsupported(String),
}
#[derive(Debug, Clone, PartialEq)]
/// One CSS property/value pair. Order is kept because later rules may replace
/// earlier values. No rule matching or priority calculation is done here.
pub struct Declaration {
    pub name: String,
    pub value: String,
    /// True when the source value ends with !important.
    pub important: bool,
    /// A value checked against the supported CSS grammar. None means that the
    /// raw value is invalid or outside that subset; it must not be applied as a
    /// shared value without a separate reader that understands it.
    pub portable: Option<(super::super::style::Property, Value)>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}
#[derive(Debug, Clone, Default, PartialEq)]
/// CSS source plus the rules this parser understands.
pub struct Stylesheet {
    /// Full source, including comments and unsupported syntax.
    pub source: String,
    pub rules: Vec<Rule>,
    /// Blocks that were kept as text because they could not be read as rules.
    pub unsupported: Vec<String>,
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
fn selector(s: &str) -> Selector {
    let s = s.trim_matches(|c: char| c.is_ascii_whitespace());
    if s == "::cue" {
        return Selector::Cue;
    }
    if s == "::cue-region" {
        return Selector::Region;
    }
    if let Some(inner) = s.strip_prefix("::cue(").and_then(|s| s.strip_suffix(')')) {
        if let Some(class) = inner.strip_prefix('.') {
            if identifier(class) {
                return Selector::Class(class.into());
            }
        }
        if let Some(id) = inner.strip_prefix('#') {
            if identifier(id) {
                return Selector::CueId(id.into());
            }
        }
        if matches!(inner, "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt") {
            return Selector::Tag(inner.into());
        }
        for (prefix, voice) in [("v[voice=", true), ("lang[lang=", false)] {
            if let Some(v) = inner.strip_prefix(prefix).and_then(|s| s.strip_suffix(']')) {
                if let Some(v) = v
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                {
                    if !v.contains('\\') {
                        return if voice {
                            Selector::Voice(v.into())
                        } else {
                            Selector::Language(v.into())
                        };
                    }
                }
            }
        }
    }
    Selector::Unsupported(s.into())
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
    let portable = super::style_values::parse(&name, value);
    Some(Declaration {
        name,
        value: value.into(),
        important,
        portable,
    })
}
pub(super) fn parse(
    source: &str,
    limits: Limits,
    budget: &mut Budget,
) -> Result<Stylesheet, Error> {
    // The source is kept for future style engines; the clean copy is temporary.
    // Reserve both copies before making either one.
    budget.bytes(
        source
            .len()
            .checked_mul(2)
            .ok_or(Error::Limit("output bytes"))?
            + 128,
    )?;
    let mut result = Stylesheet {
        source: source.into(),
        ..Stylesheet::default()
    };
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
            budget.bytes(rest.len() + 32)?;
            result.unsupported.push(rest.into());
            return Ok(result);
        };
        let body = &body[..end];
        if selector_text.starts_with('@') || body.contains('{') {
            budget.bytes(open + end + 34)?;
            result.unsupported.push(rest[..open + end + 2].into());
        } else {
            let mut selectors = Vec::new();
            for part in split(selector_text, ',') {
                budget.nodes(1)?;
                budget.bytes(256 + part.len() * 2)?;
                selectors.push(selector(part));
            }
            let mut declarations = Vec::new();
            for part in split(body, ';')
                .filter(|s| !s.trim_matches(|c: char| c.is_ascii_whitespace()).is_empty())
            {
                budget.nodes(1)?;
                // Include the raw name/value and the optional portable value.
                budget.bytes(512 + part.len() * 4)?;
                if let Some(value) = declaration(part) {
                    declarations.push(value);
                }
            }
            result.rules.push(Rule {
                selectors,
                declarations,
            });
        }
        rest = &rest[open + end + 2..];
    }
    if !rest
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .is_empty()
    {
        budget.bytes(rest.len() + 32)?;
        result.unsupported.push(rest.into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::super::style::{Color, Property};
    use super::*;
    fn parse(source: &str) -> Stylesheet {
        let limits = Limits::default();
        super::parse(source, limits, &mut Budget::new(limits)).unwrap()
    }
    #[test]
    fn portable_rules_preserve_order_importance_and_unknown_values() {
        let s=parse("/* header */ ::cue(.red), ::cue(v[voice=\"A,B\"]) { color: red; color: blue !important; font-size: 2em; future: value; }");
        assert_eq!(
            s.rules[0].selectors,
            vec![Selector::Class("red".into()), Selector::Voice("A,B".into())]
        );
        let d = &s.rules[0].declarations;
        assert_eq!(d.len(), 4);
        assert!(!d[0].important);
        assert!(d[1].important);
        assert_eq!(
            d[0].portable,
            Some((Property::Color, Value::Color(Color(255, 0, 0, 255))))
        );
        assert!(d[3].portable.is_none());
    }
    #[test]
    fn strings_comments_and_unsupported_constructs() {
        let source="::cue { font-family: 'a;/*b*/:c'; } @media screen { ::cue { color: red } } ::cue(.a .b) { color: rgb(1,2,3); }";
        let s = parse(source);
        assert_eq!(s.source, source);
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.unsupported.len(), 1);
        assert_eq!(s.rules[0].declarations[0].value, "'a;/*b*/:c'");
        assert!(matches!(s.rules[1].selectors[0], Selector::Unsupported(_)));
        assert!(s.rules[1].declarations[0].portable.is_some());
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
