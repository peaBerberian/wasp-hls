//! Only TTML elements on a known document path can define styles or regions.
//! A foreign element or metadata starts an ignored subtree. Do not search below
//! it for familiar names: those names belong to the ignored data, too.
use super::*;

pub(super) struct Structure {
    pub elements: Vec<usize>,
    pub definitions: BTreeMap<String, usize>,
    pub regions: Vec<usize>,
}
pub(super) fn collect(arena: &[Element], budget: &mut Budget) -> Result<Structure, Error> {
    let mut result = Structure {
        elements: Vec::new(),
        definitions: BTreeMap::new(),
        regions: Vec::new(),
    };
    let mut ids = std::collections::BTreeSet::new();
    let mut pending = vec![(0, "", false)];
    while let Some((index, parent, in_layout)) = pending.pop() {
        let e = &arena[index];
        if e.ns != TT || e.text.is_some() {
            continue;
        }
        let allowed = match parent {
            "" => e.name == "tt",
            "tt" => matches!(e.name.as_str(), "head" | "body"),
            "head" => matches!(e.name.as_str(), "styling" | "layout"),
            "styling" | "style" => e.name == "style",
            "layout" => e.name == "region",
            "region" => e.name == "style",
            // Content checks stay with the tree reader. These paths only
            // collect IDs and references; they cannot introduce definitions.
            "body" | "div" | "p" | "span" => {
                matches!(e.name.as_str(), "div" | "p" | "span" | "br")
            }
            _ => false,
        };
        if !allowed {
            continue;
        }
        budget.bytes(32)?;
        result.elements.push(index);
        if let Some(id) = e.attr(XML, "id") {
            budget.bytes(64 + id.len())?;
            if !ids.insert(id) {
                return Err(Error::Invalid(format!("duplicate xml:id {id}")));
            }
        }
        if e.name == "style" && !in_layout {
            if let Some(id) = e.attr(XML, "id") {
                budget.bytes(128 + id.len())?;
                result.definitions.insert(id.into(), index);
            }
        } else if e.name == "region" {
            result.regions.push(index);
        }
        for &child in e.children.iter().rev() {
            pending.push((child, &e.name, in_layout || e.name == "layout"));
        }
    }
    Ok(result)
}
