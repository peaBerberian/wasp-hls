//! Namespace-aware, bounded XML arena. DTDs and external entities are not accepted.
use super::super::{budget::Budget, Error, Limits};
use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};

#[derive(Debug)]
pub(super) struct Attribute {
    pub ns: String,
    pub name: String,
    pub value: String,
}
#[derive(Debug)]
pub(super) struct Element {
    pub ns: String,
    pub name: String,
    pub attrs: Vec<Attribute>,
    pub children: Vec<usize>,
    pub text: Option<String>,
}
impl Element {
    pub fn attr(&self, ns: &str, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.ns == ns && a.name == name)
            .map(|a| a.value.as_str())
    }
}
fn namespace(r: ResolveResult<'_>, budget: &mut Budget) -> Result<String, Error> {
    match r {
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Bound(ns) => {
            budget.bytes(ns.as_ref().len() + 32)?;
            std::str::from_utf8(ns.as_ref())
                .map(String::from)
                .map_err(|e| Error::Invalid(e.to_string()))
        }
        ResolveResult::Unknown(_) => Err(Error::Invalid("unbound XML namespace prefix".into())),
    }
}
pub(super) fn parse(
    bytes: &[u8],
    limits: Limits,
    budget: &mut Budget,
) -> Result<Vec<Element>, Error> {
    if bytes.len() > limits.bytes {
        return Err(Error::Limit("bytes"));
    }
    let source = std::str::from_utf8(bytes).map_err(|e| Error::Invalid(e.to_string()))?;
    if source.chars().any(
        |c| matches!(c,'\0'..='\u{8}'|'\u{b}'|'\u{c}'|'\u{e}'..='\u{1f}'|'\u{fffe}'|'\u{ffff}'),
    ) {
        return Err(Error::Invalid("forbidden XML character".into()));
    }
    // XML 1.0 end-of-line normalization precedes entity expansion.
    // Most documents need no copy. When CR is present, normalize in one pass
    // rather than building two whole-document temporary strings.
    let normalized = if source.contains('\r') {
        budget.bytes(source.len())?;
        let mut out = String::with_capacity(source.len());
        let mut chars = source.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\r' {
                out.push('\n');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            } else {
                out.push(c);
            }
        }
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(source)
    };
    // TODO: Measure whether a small XML tokenizer could reduce the final WASM
    // size by replacing quick-xml and its memchr dependency. HLS uses the IMSC1
    // text profile, which requires UTF-8, and MP4 extraction already gives us
    // complete XML samples. We do not need to read XML across input chunks or
    // support other encodings. RxPlayer's MPD tokenizer could be a starting
    // point, but we must keep whitespace, entities and CDATA, resolve namespace
    // bindings (including foreign XML), and keep our XML checks and limits.
    // quick-xml already reads from a slice here. Measure speed and memory use
    // before claiming improvements in either.
    let mut reader = NsReader::from_str(&normalized);
    let mut arena: Vec<Element> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if stack.len() >= limits.depth {
                    return Err(Error::Limit("depth"));
                }
                if arena.len() >= limits.nodes {
                    return Err(Error::Limit("nodes"));
                }
                if stack.is_empty() && !arena.is_empty() {
                    return Err(Error::Invalid("multiple XML roots".into()));
                }
                budget.bytes(std::mem::size_of::<Element>() * 2 + 64 + e.name().as_ref().len())?;
                let (ns, name) = reader.resolve_element(e.name());
                let ns = namespace(ns, budget)?;
                let name = std::str::from_utf8(name.as_ref())
                    .map_err(|e| Error::Invalid(e.to_string()))?
                    .into();
                let mut attrs = Vec::new();
                let mut names = std::collections::BTreeSet::new();
                let mut raw_names = std::collections::BTreeSet::new();
                // quick-xml's duplicate check scans every earlier attribute.
                // Borrow raw names for a sorted set instead, including xmlns
                // declarations, then check expanded names separately below.
                for a in e.attributes().with_checks(false) {
                    let a = a.map_err(|e| Error::Invalid(e.to_string()))?;
                    budget.bytes(64)?;
                    if !raw_names.insert(a.key.into_inner()) {
                        return Err(Error::Invalid("duplicate XML attribute".into()));
                    }
                    if a.key.as_ref() == b"xmlns" || a.key.as_ref().starts_with(b"xmlns:") {
                        continue;
                    }
                    budget.bytes(
                        std::mem::size_of::<Attribute>() * 2
                            + 128
                            + a.key.as_ref().len() * 3
                            + a.value.len(),
                    )?;
                    let (ns, name) = reader.resolve_attribute(a.key);
                    let ns = namespace(ns, budget)?;
                    let name = std::str::from_utf8(name.as_ref())
                        .map_err(|e| Error::Invalid(e.to_string()))?
                        .to_string();
                    budget.bytes(ns.len() + 32)?;
                    if !names.insert((ns.clone(), name.clone())) {
                        return Err(Error::Invalid("duplicate expanded XML attribute".into()));
                    }
                    attrs.push(Attribute {
                        ns,
                        name,
                        value: a
                            .unescape_value()
                            .map_err(|e| Error::Invalid(e.to_string()))?
                            .into_owned(),
                    });
                }
                let index = arena.len();
                if let Some(p) = stack.last() {
                    arena[*p].children.push(index);
                }
                arena.push(Element {
                    ns,
                    name,
                    attrs,
                    children: Vec::new(),
                    text: None,
                });
                if matches!(event, Event::Start(_)) {
                    stack.push(index);
                }
            }
            Event::End(_) => {
                if stack.pop().is_none() {
                    return Err(Error::Invalid("unexpected XML end tag".into()));
                }
            }
            Event::Text(e) => {
                budget.bytes(std::mem::size_of::<Element>() * 2 + 32 + e.as_ref().len())?;
                let text = e
                    .unescape()
                    .map_err(|e| Error::Invalid(e.to_string()))?
                    .into_owned();
                append_text(&mut arena, &stack, text, limits)?;
            }
            Event::CData(e) => {
                budget.bytes(std::mem::size_of::<Element>() * 2 + 32 + e.as_ref().len())?;
                let text = std::str::from_utf8(e.as_ref())
                    .map_err(|e| Error::Invalid(e.to_string()))?
                    .into();
                append_text(&mut arena, &stack, text, limits)?;
            }
            Event::DocType(_) => return Err(Error::Unsupported("XML DTD".into())),
            Event::Decl(e) => {
                if let Some(encoding) = e.encoding() {
                    let encoding = encoding.map_err(|e| Error::Invalid(e.to_string()))?;
                    if !encoding.eq_ignore_ascii_case(b"UTF-8") {
                        return Err(Error::Unsupported("XML encoding other than UTF-8".into()));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if arena.is_empty() || !stack.is_empty() {
        return Err(Error::Invalid("missing or unclosed XML root".into()));
    }
    Ok(arena)
}
fn append_text(
    arena: &mut Vec<Element>,
    stack: &[usize],
    text: String,
    limits: Limits,
) -> Result<(), Error> {
    if let Some(p) = stack.last() {
        if arena.len() >= limits.nodes {
            return Err(Error::Limit("nodes"));
        }
        let i = arena.len();
        arena[*p].children.push(i);
        arena.push(Element {
            ns: String::new(),
            name: String::new(),
            attrs: Vec::new(),
            children: Vec::new(),
            text: Some(text),
        });
    } else if !text.trim().is_empty() {
        return Err(Error::Invalid("text outside XML root".into()));
    }
    Ok(())
}
