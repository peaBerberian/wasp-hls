//! A conservative budget for parser output and temporary copies.
//!
//! Reserve space before allocating. We do not return space to the budget when a
//! temporary value is dropped. This also limits allocation churn, which matters
//! in WASM because freeing a value does not shrink linear memory.
// TODO: Measure hostile inputs that stay within these memory limits. Add a
// work allowance or cancellation checks if efficient parsing alone is not enough.
// Response-size limits belong to the downloader, before it retains the input.
use super::{Error, Limits};

pub(super) struct Budget {
    bytes_left: usize,
    nodes_left: usize,
}
impl Budget {
    pub fn new(limits: Limits) -> Self {
        Self {
            bytes_left: limits.output_bytes,
            nodes_left: limits.nodes,
        }
    }
    pub fn bytes(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes_left = self
            .bytes_left
            .checked_sub(bytes)
            .ok_or(Error::Limit("output bytes"))?;
        Ok(())
    }
    pub fn nodes(&mut self, nodes: usize) -> Result<(), Error> {
        self.nodes_left = self
            .nodes_left
            .checked_sub(nodes)
            .ok_or(Error::Limit("output nodes"))?;
        Ok(())
    }
}
