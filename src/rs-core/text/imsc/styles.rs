//! Read and combine specified styles. Shared styles keep repeated references
//! cheap. Every copy made by a merge is charged before it is allocated.
//! Arc also lets a native backend share parsed documents across threads.
use super::*;

/// Upper estimate of the space needed to copy this style. The extra space per
/// property covers map nodes and allocator overhead on native and WASM targets.
fn copy_bytes(style: &Style) -> usize {
    let mut bytes = std::mem::size_of::<Style>() + 32;
    for value in style.properties.values() {
        bytes += 256;
        // Keyword and text strings use Arc<str>, so a map copy only copies
        // their handles. Length lists are small but do need their own vector.
        if let super::super::style::Value::Lengths(v) = value {
            bytes += v.len() * std::mem::size_of::<super::super::style::Length>();
        }
    }
    bytes
}
fn merge(target: &mut Arc<Style>, source: Arc<Style>, budget: &mut Budget) -> Result<(), Error> {
    if source.properties.is_empty() {
        return Ok(());
    }
    if target.properties.is_empty() {
        // The first reference can be shared without copying its strings or map.
        *target = source;
        return Ok(());
    }
    budget.bytes(copy_bytes(target))?;
    budget.bytes(copy_bytes(&source))?;
    // Copy only when another node still uses this map. The budget also covers
    // the values copied by overlay, even when the target map is already unique.
    Arc::make_mut(target).overlay(&source);
    Ok(())
}
pub(super) fn inline_style(
    e: &Element,
    empty: &Arc<Style>,
    budget: &mut Budget,
) -> Result<Arc<Style>, Error> {
    // TODO: Add usual EBU linePadding/multiRowAlign values if content needs them.
    let mut result = None;
    for a in &e.attrs {
        if a.ns != TTS {
            continue;
        }
        budget.bytes(256 + a.value.len())?;
        if let Some((property, value)) = style_values::parse(&a.name, &a.value) {
            budget.nodes(1)?;
            result
                .get_or_insert_with(Style::default)
                .properties
                .insert(property, value);
        }
    }
    if let Some(style) = result {
        budget.bytes(std::mem::size_of::<Style>() + 32)?;
        Ok(Arc::new(style))
    } else {
        Ok(empty.clone())
    }
}

/// Cache named styles once. The active path finds cycles; it is separate from
/// the cache so forward references and repeated references remain valid.
pub(super) struct Resolver<'a> {
    pub cache: BTreeMap<String, Arc<Style>>,
    arena: &'a [Element],
    definitions: &'a BTreeMap<String, usize>,
    path: Vec<String>,
    depth: usize,
    empty: &'a Arc<Style>,
    budget: &'a mut Budget,
}
impl<'a> Resolver<'a> {
    pub fn new(
        arena: &'a [Element],
        definitions: &'a BTreeMap<String, usize>,
        depth: usize,
        empty: &'a Arc<Style>,
        budget: &'a mut Budget,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            arena,
            definitions,
            path: Vec::new(),
            depth,
            empty,
            budget,
        }
    }
    pub fn resolve(&mut self, id: &str) -> Result<Arc<Style>, Error> {
        if let Some(style) = self.cache.get(id) {
            return Ok(style.clone());
        }
        if self.path.iter().any(|p| p == id) {
            return Err(Error::Invalid(format!("cyclic style reference {id}")));
        }
        if self.path.len() >= self.depth {
            return Err(Error::Limit("style reference depth"));
        }
        let Some(&index) = self.definitions.get(id) else {
            return Ok(self.empty.clone());
        };
        self.budget.bytes(128 + id.len() * 2)?;
        self.path.push(id.into());
        let style = self.element(index)?;
        self.path.pop();
        self.cache.insert(id.into(), style.clone());
        Ok(style)
    }
    fn element(&mut self, index: usize) -> Result<Arc<Style>, Error> {
        if self.path.len() >= self.depth {
            return Err(Error::Limit("style reference depth"));
        }
        let e = &self.arena[index];
        let mut style = self.empty.clone();
        for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
            let referenced = self.resolve(id)?;
            merge(&mut style, referenced, self.budget)?;
        }
        for &child in &e.children {
            let e = &self.arena[child];
            if e.ns != TT || e.name != "style" {
                continue;
            }
            let nested = if let Some(id) = e.attr(XML, "id") {
                self.resolve(id)?
            } else {
                self.path.push(String::new());
                let result = self.element(child)?;
                self.path.pop();
                result
            };
            merge(&mut style, nested, self.budget)?;
        }
        let inline = inline_style(e, self.empty, self.budget)?;
        merge(&mut style, inline, self.budget)?;
        Ok(style)
    }
}
pub(super) fn specified_style(
    e: &Element,
    styles: &BTreeMap<String, Arc<Style>>,
    empty: &Arc<Style>,
    budget: &mut Budget,
) -> Result<Arc<Style>, Error> {
    let mut result = referenced_style(e, styles, empty, budget)?;
    let inline = inline_style(e, empty, budget)?;
    merge(&mut result, inline, budget)?;
    Ok(result)
}

/// Local region styles are not global definitions. Later nested styles win
/// over earlier ones; inline attributes win over all nested styles.
/// Referenced styles have the lowest priority.
pub(super) fn region_style(
    e: &Element,
    arena: &[Element],
    styles: &BTreeMap<String, Arc<Style>>,
    empty: &Arc<Style>,
    budget: &mut Budget,
) -> Result<Arc<Style>, Error> {
    let mut result = referenced_style(e, styles, empty, budget)?;
    for &child in &e.children {
        let child = &arena[child];
        if child.ns == TT && child.name == "style" {
            let nested = specified_style(child, styles, empty, budget)?;
            merge(&mut result, nested, budget)?;
        }
    }
    let inline = inline_style(e, empty, budget)?;
    merge(&mut result, inline, budget)?;
    Ok(result)
}

fn referenced_style(
    e: &Element,
    styles: &BTreeMap<String, Arc<Style>>,
    empty: &Arc<Style>,
    budget: &mut Budget,
) -> Result<Arc<Style>, Error> {
    let mut result = empty.clone();
    for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
        if let Some(style) = styles.get(id) {
            merge(&mut result, style.clone(), budget)?;
        }
    }
    Ok(result)
}
