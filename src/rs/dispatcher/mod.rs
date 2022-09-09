use crate::{
    wasm_bindgen,
    media_element::MediaElementReference,
    requester::Requester,
    content_tracker::ContentTracker,
    adaptive::AdaptiveQualitySelector,
    segment_selector::NextSegmentSelectors,
};

mod api;
mod streaming;


/// The `Dispatcher` is the player Interface exported to the JavaScript-side,
/// providing an API to load contents and influence various parameters about playback.
#[wasm_bindgen]
pub struct Dispatcher {

    /// Current `PlayerReadyState` the `Dispatcher` is in.
    ready_state: PlayerReadyState,

    /// Struct allowing to obtain estimate of the optimal variants to play,
    /// mostly based on network metrics.
    adaptive_selector: AdaptiveQualitySelector,

    /// Store the "MultiVariant Playlist" (structure which describes the currently
    /// loaded content) alongside some state to keep track of the chosen... tracks.
    /// (More technically of variants and media streams).
    ///
    /// `None` if no "MultiVariant Playlist" has been loaded yet.
    content_tracker: Option<ContentTracker>,

    ///// Abstraction allowing to create and store `SourceBuffer`s on the `MediaSource` attached
    ///// to the media element itself linked to the `Dispatcher`.
    /////
    ///// None if not `MediaSource` has been attached yet.
    //buffers: Option<MediaBuffers>,

    media_element_ref: MediaElementReference,

    /// Abstraction allowing to perform playlist and segment requests, while
    /// easily monitoring requests that are pending.
    requester: Requester,

    /// Amount of buffer, ahead of the current position we want to build in seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    buffer_goal: f64,

    /// The last known current position stored.
    /// Changes periodically and immediately on various time-changing events (such as seeks, stops
    /// etc.)
    last_position: f64,

    segment_selectors: NextSegmentSelectors,
}

/// Identify the JavaScript `readyState` of a created `MediaSource` instance.
#[wasm_bindgen]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MediaSourceReadyState {
    /// Corresponds to the "closed" JavaScript MediaSource's `readyState`
    Closed = 0,
    /// Corresponds to the "ended" JavaScript MediaSource's `readyState`
    Ended = 1,
    /// Corresponds to the "open" JavaScript MediaSource's `readyState`
    Open = 2,
}

/// Identify the playback-related state the `Dispatcher` is in.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
enum PlayerReadyState {
    /// No content is currently loaded.
    Stopped = 0,

    /// We're preparing a content's playlist, MediaSource and SourceBuffers
    Loading = 1,

    /// The SourceBuffers are all ready but currently awaiting segments before
    /// being aple to play.
    AwaitingSegments = 2,

    /// The content has enough segments to play.
    /// Note that this does not mean the media element is currently playing content:
    /// it can still be paused or at a `0` playback rate.
    Playing = 3,
}
