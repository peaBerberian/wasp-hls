//! Read named styles and apply them. Do not return the style definitions.
use super::*;
use crate::text::{
    properties::{self, Computed, Property},
    Length,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) struct Styles {
    definitions: BTreeMap<String, usize>,
    cache: BTreeMap<usize, Vec<Property>>,
    pub regions: BTreeMap<String, usize>,
    pub base: Computed,
    pixels: Option<f64>,
    depth: usize,
    pub diagnostics: Vec<crate::text::Diagnostic>,
    extent: Option<[Length; 2]>,
}
impl Styles {
    pub fn read(xml: &[Element], limits: Limits, budget: &mut Budget) -> Result<Self, Error> {
        let rows = xml[0]
            .attr(TTP, "cellResolution")
            .map(pair)
            .transpose()?
            .map_or(15, |p| p.1);
        let mut base = Computed::default();
        base.appearance.font_size = Length::VideoHeight(1.0 / rows as f64);
        let extent = xml[0].attr(TTS, "extent").and_then(|s| {
            let mut v = s.split_ascii_whitespace();
            let pair = [
                properties::length(v.next()?, false)?,
                properties::length(v.next()?, false)?,
            ];
            (v.next().is_none() && pair.iter().all(|l| !matches!(l, Length::Em(_)))).then_some(pair)
        });
        let pixels = extent.and_then(|v| {
            if let Length::Pixels(n) = v[1] {
                (n > 0.0).then_some(n)
            } else {
                None
            }
        });
        let mut out = Self {
            definitions: BTreeMap::new(),
            cache: BTreeMap::new(),
            regions: BTreeMap::new(),
            base,
            pixels,
            depth: limits.depth,
            diagnostics: Vec::new(),
            extent,
        };
        // Only styles under head/styling can define named styles. Skip styles
        // in metadata or another XML namespace. Styles inside a region apply
        // only to that region.
        let mut todo = vec![(0, "", false)];
        let mut ids = BTreeSet::new();
        let mut known = Vec::new();
        while let Some((i, parent, local)) = todo.pop() {
            let e = &xml[i];
            if e.ns != TT {
                continue;
            }
            let allowed = match parent {
                "" => e.name == "tt",
                "tt" => matches!(e.name.as_str(), "head" | "body"),
                "head" => matches!(e.name.as_str(), "layout" | "styling"),
                "styling" | "style" | "region" => e.name == "style",
                "layout" => e.name == "region",
                "body" | "div" | "p" | "span" => {
                    matches!(e.name.as_str(), "div" | "p" | "span" | "br")
                }
                _ => false,
            };
            if !allowed {
                continue;
            }
            known.push(i);
            if let Some(id) = e.attr(XML, "id") {
                budget.bytes(128 + id.len())?;
                if !ids.insert(id) {
                    return Err(Error::Invalid(format!("duplicate xml:id {id}")));
                }
                if e.name == "style" && !local {
                    out.definitions.insert(id.into(), i);
                }
                if e.name == "region" {
                    out.regions.insert(id.into(), i);
                }
            } else if e.name == "region" {
                return Err(Error::Invalid("region without xml:id".into()));
            }
            for &c in e.children.iter().rev() {
                todo.push((c, e.name.as_str(), local || e.name == "layout"));
            }
        }
        for i in known {
            for id in xml[i]
                .attr("", "style")
                .unwrap_or("")
                .split_ascii_whitespace()
            {
                if !out.definitions.contains_key(id) {
                    budget.nodes(1)?;
                    budget.bytes(128 + id.len())?;
                    out.diagnostics.push(crate::text::Diagnostic {
                        line: None,
                        message: format!("ignored unknown style reference {id}"),
                    });
                }
            }
        }
        for &i in out.definitions.values().copied().collect::<Vec<_>>().iter() {
            out.resolve(i, xml, &mut Vec::new(), budget)?;
        }
        Ok(out)
    }
    fn resolve(
        &mut self,
        i: usize,
        xml: &[Element],
        path: &mut Vec<usize>,
        budget: &mut Budget,
    ) -> Result<Vec<Property>, Error> {
        if let Some(v) = self.cache.get(&i) {
            budget.bytes(v.len() * 128)?;
            return Ok(v.clone());
        }
        if path.contains(&i) {
            return Err(Error::Invalid("cyclic style reference".into()));
        }
        if path.len() >= self.depth {
            return Err(Error::Limit("style reference depth"));
        }
        path.push(i);
        let mut out = Vec::new();
        let e = &xml[i];
        for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
            if let Some(&j) = self.definitions.get(id) {
                for p in self.resolve(j, xml, path, budget)? {
                    properties::put(&mut out, p);
                }
            }
        }
        for &j in &e.children {
            if xml[j].ns == TT && xml[j].name == "style" {
                for p in self.resolve(j, xml, path, budget)? {
                    properties::put(&mut out, p);
                }
            }
        }
        for a in &e.attrs {
            if a.ns == TTS {
                if let Some(p) = properties::parse(&a.name, &a.value, false, budget)? {
                    budget.nodes(1)?;
                    properties::put(&mut out, p);
                }
            }
        }
        path.pop();
        budget.bytes(128 + out.len() * 128)?;
        self.cache.insert(i, out.clone());
        Ok(out)
    }
    pub fn specified(
        &self,
        i: usize,
        xml: &[Element],
        budget: &mut Budget,
    ) -> Result<Vec<Property>, Error> {
        if let Some(v) = self.cache.get(&i) {
            budget.bytes(v.len() * 128)?;
            return Ok(v.clone());
        }
        let e = &xml[i];
        let mut out = Vec::new();
        for id in e.attr("", "style").unwrap_or("").split_ascii_whitespace() {
            if let Some(v) = self.definitions.get(id).and_then(|i| self.cache.get(i)) {
                budget.bytes(v.len() * 128)?;
                for p in v {
                    properties::put(&mut out, p.clone());
                }
            }
        }
        // Later nested styles replace earlier ones. The region attributes have
        // priority over nested styles. Both have priority over named styles.
        if matches!(e.name.as_str(), "region" | "style") {
            for &j in &e.children {
                if xml[j].ns == TT && xml[j].name == "style" {
                    for p in self.specified(j, xml, budget)? {
                        properties::put(&mut out, p);
                    }
                }
            }
        }
        for a in &e.attrs {
            if a.ns == TTS {
                if let Some(p) = properties::parse(&a.name, &a.value, false, budget)? {
                    budget.nodes(1)?;
                    properties::put(&mut out, p);
                }
            }
        }
        Ok(out)
    }
    pub fn apply(
        &self,
        parent: &Computed,
        local: &[Property],
        budget: &mut Budget,
    ) -> Result<Computed, Error> {
        budget.bytes(256 + local.len() * 32)?;
        Ok(parent.apply(local, self.pixels))
    }
    pub fn region(
        &self,
        i: Option<usize>,
        xml: &[Element],
        budget: &mut Budget,
    ) -> Result<Computed, Error> {
        if let Some(i) = i {
            self.apply(&self.base, &self.specified(i, xml, budget)?, budget)
        } else {
            Ok(self.base.clone())
        }
    }
    pub fn placement(&self, _i: Option<usize>, computed: &Computed) -> crate::text::Placement {
        let mut out = computed.placement.clone();
        // If tt gives the document size in pixels, convert its pixel positions
        // and sizes to percentages. They will then follow the video size.
        for (axis, l) in [(0, &mut out.x.offset), (1, &mut out.y.offset)] {
            if let (Length::Pixels(n), Some(Length::Pixels(total))) =
                (*l, self.extent.map(|v| v[axis]))
            {
                if total > 0.0 {
                    let percent = n / total * 100.0;
                    if percent.is_finite() {
                        *l = Length::Percent(percent);
                    }
                }
            }
        }
        for (axis, l) in [(0, &mut out.width), (1, &mut out.height)] {
            if let (Some(Length::Pixels(n)), Some(Length::Pixels(total))) =
                (*l, self.extent.map(|v| v[axis]))
            {
                if total > 0.0 {
                    let percent = n / total * 100.0;
                    if percent.is_finite() {
                        *l = Some(Length::Percent(percent));
                    }
                }
            }
        }
        out
    }
}
