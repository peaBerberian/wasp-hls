pub(crate) type SegmentRequestId = u32;

use std::collections::HashMap;

use crate::{
    bindings::MediaType, media_element::SegmentQualityContext, parser::SegmentTimeInfo,
    playlist_store::ProbeSegmentMetadata,
};

/// Simple store util which maps pending async segment operations to an u32
/// `SegmentRequestId`.
///
/// Modules doing async operation often just refer back to an identifier when done. This
/// util allows to store the necessary information to take back the context once done.
///
/// This type of structure is necessary here because async operations might go through JS,
/// so we cannot just rely easily on Rust's `async` abstraction to save such state.
pub(crate) struct SegmentRequestContexts {
    /// `SegmentRequestId` that will be returned for the next inserted item.
    next_request_id: SegmentRequestId,
    /// The stored "contexts" themselves.
    contexts: HashMap<SegmentRequestId, PendingSegmentRequest>,
}

impl SegmentRequestContexts {
    /// Create a new `SegmentRequestContexts`.
    pub(crate) fn new() -> Self {
        Self {
            next_request_id: 0,
            contexts: HashMap::new(),
        }
    }

    /// Remove all operations currently stored on this `SegmentRequestContexts`.
    pub(crate) fn clear(&mut self) {
        self.contexts.clear();
    }

    /// Add context for a new request, returning a `SegmentRequestId`.
    pub(crate) fn insert(&mut self, context: PendingSegmentRequest) -> SegmentRequestId {
        let request_id = self.next_request_id;

        // NOTE: We do not assume here the number of requests nor the longevity of the
        // runtime, so we use `wrapping_add` as a security against overflows.
        // The risk of conflict is so ridiculously low that it should not matter.
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.contexts.insert(request_id, context);
        request_id
    }

    /// Get back the context associated to the given `SegmentRequestId`.
    pub(crate) fn take(&mut self, request_id: SegmentRequestId) -> Option<PendingSegmentRequest> {
        self.contexts.remove(&request_id)
    }

    /// Check a predicate against all stored contexts, returning `true` if any of them matches.
    pub(crate) fn has<F>(&self, predicate: F) -> bool
    where
        F: Fn(&PendingSegmentRequest) -> bool,
    {
        self.contexts.values().any(predicate)
    }

    /// Return the ids for all contexts matching the given predicate.
    pub(crate) fn ids_matching<F>(&self, predicate: F) -> Vec<SegmentRequestId>
    where
        F: Fn(&PendingSegmentRequest) -> bool,
    {
        self.contexts
            .iter()
            .filter_map(|(request_id, context)| predicate(context).then_some(*request_id))
            .collect()
    }
}

/// Singular item inserted into the `SegmentRequestContexts`.
pub(crate) enum PendingSegmentRequest {
    /// This request concerns an initialization segment
    Init {
        /// The `MediaType` the initialization segment is linked to.
        media_type: MediaType,
        /// Unique identifier identifying that initialization segment for the
        /// corresponding media and `MediaType`.
        init_segment_id: f64,
    },
    /// This request concerns a media segment
    Media {
        /// The `MediaType` the media segment is linked to.
        media_type: MediaType,
        /// Unique identifier of the initialization segment applying to that media segment, if any.
        init_segment_id: Option<f64>,
        /// Time-related metadata linked to that segment.
        time_info: SegmentTimeInfo,
        /// Additional context required for ABR
        quality_context: SegmentQualityContext,
        // The media segment sequence number, as infered by the HLS playlist
        sequence_number: u32,
        // The media segment discontinuity sequence number, as infered by the HLS playlist
        discontinuity_sequence: u32,
    },
    /// This request concerns a "probe" request: we're loading a segment to know about a media's
    /// characteristics
    Probe {
        probe_segment: ProbeSegmentMetadata,
        requested_media_type: Option<MediaType>,
    },
}

impl PendingSegmentRequest {
    pub(crate) fn is_probe(&self) -> bool {
        matches!(self, Self::Probe { .. })
    }
    pub(crate) fn is_media(&self) -> bool {
        matches!(self, Self::Media { .. })
    }
    pub(crate) fn is_init(&self) -> bool {
        matches!(self, Self::Init { .. })
    }

    pub(crate) fn requested_media_type(&self) -> Option<MediaType> {
        match self {
            Self::Init { media_type, .. } | Self::Media { media_type, .. } => Some(*media_type),
            Self::Probe {
                requested_media_type,
                ..
            } => *requested_media_type,
        }
    }
}
