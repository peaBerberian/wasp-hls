use crate::{
    wasm_bindgen,
    bindings::PlayerId,
    content::WaspHlsContent,
    media_source::MediaSourceHandle,
    requester::Requester,
};

mod api;
mod streaming;

use streaming::SegmentQueues;

/// The `WaspHlsPlayer` is the player Interface exported to the JavaScript-side,
/// providing an API to load contents and influence various parameters about playback.
#[wasm_bindgen]
pub struct WaspHlsPlayer {
    /// Identifier for the current `WaspHlsPlayer` instance on the JS-side.
    /// Many JavaScript-side APIs rely on that `id`.
    id: PlayerId,

    /// Current `WaspHlsPlayerReadyState` the `WaspHlsPlayer` is in.
    ready_state: WaspHlsPlayerReadyState,

    /// Store the "MultiVariant Playlist" (structure which describes the currently
    /// loaded content) alongside some state to keep track of the chosen... tracks.
    /// (More technically of variants and media streams).
    ///
    /// `None` if no "MultiVariant Playlist" has been loaded yet.
    content: Option<WaspHlsContent>,

    requester: Requester,

    /// Abstraction allowing to create and store `SourceBuffer`s on the `MediaSource` attached
    /// to the media element itself linked to the `WaspHlsPlayer`.
    media_source: MediaSourceHandle,

    /// Amount of buffer, ahead of the current position we want to build in seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    buffer_goal: f64,

    /// The last known current position stored.
    /// Changes periodically and immediately on various time-changing events (such as seeks, stops
    /// etc.)
    last_position: f64,

    /// Interface allowing to keep track of which audio and video segments we need to load next.
    segment_queues: SegmentQueues,
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

/// Identify the playback-related state the `WaspHlsPlayer` is in.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
enum WaspHlsPlayerReadyState {
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
