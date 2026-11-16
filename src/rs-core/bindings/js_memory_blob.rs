use super::{jsFreeResource, jsGetResourceData, ResourceId};

/// Special structure to handle data that is only present in JavaScript's
/// memory.
///
/// The data is identified through a unique `ResourceId` identifier.
///
/// The idea behind this struct is to prevent memory leaks by implementing the
/// Drop trait on it, so the resource is freed when no ownership of it is left.
#[derive(Debug)]
pub struct JsMemoryBlob {
    /// Its unique identifier
    id: ResourceId,
}

impl JsMemoryBlob {
    /// Create a `JsMemoryBlob` object from the `ResourceId` given by the
    /// JavaScript-side
    pub fn from_resource_id(id: ResourceId) -> Self {
        Self { id }
    }

    /// Recuperates the ResourceId behind this `JsMemoryBlob`.
    pub fn id(&self) -> ResourceId {
        self.id
    }

    /// Obtain the data behind this `JsMemoryBlob`, if the resource still exists.
    pub fn obtain(self) -> Option<Vec<u8>> {
        jsGetResourceData(self.id)
    }
}

impl Drop for JsMemoryBlob {
    fn drop(&mut self) {
        jsFreeResource(self.id);
    }
}
