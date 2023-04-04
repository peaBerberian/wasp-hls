use crate::{
    adaptive::AdaptiveQualitySelector,
    bindings::TimerId,
    media_element::MediaElementReference,
    playlist_store::PlaylistStore,
    requester::{PlaylistFileType, Requester},
    segment_selector::NextSegmentSelectors,
    wasm_bindgen,
};

mod api;
mod core;
mod event_listeners;

pub(crate) use event_listeners::{
    JsMemoryBlob, JsTimeRanges, MediaObservation, PlaybackTickReason,
};

/// The `Dispatcher` is the player Interface exported to the JavaScript-side,
/// providing an API to load contents and influence various parameters about playback.
#[wasm_bindgen]
pub struct Dispatcher {
    /// Current `PlayerReadyState` the `Dispatcher` is in.
    ready_state: PlayerReadyState,

    /// Allows to perform actions related to the HTMLMediaElement on the page, like buffering media,
    /// pausing, seeking etc.
    media_element_ref: MediaElementReference,

    /// Struct allowing to obtain estimate of the optimal variants to play,
    /// mostly based on network metrics.
    adaptive_selector: AdaptiveQualitySelector,

    /// Store the "Multivariant Playlist" (structure which describes the currently
    /// loaded content) alongside some state to keep track of the chosen... tracks.
    /// (More technically of variants and media streams).
    ///
    /// `None` if no "Multivariant Playlist" has been loaded yet.
    playlist_store: Option<PlaylistStore>,

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

    playlist_refresh_timers: Vec<(TimerId, PlaylistFileType)>,
}

/// Identify the JavaScript `readyState` of a created `MediaSource` instance.
#[wasm_bindgen]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MediaSourceReadyState {
    /// Corresponds to the "closed" JavaScript MediaSource's `readyState`
    Closed = 0,
    /// Corresponds to the "ended" JavaScript MediaSource's `readyState`
    Ended = 1,
    /// Corresponds to the "open" JavaScript MediaSource's `readyState`
    Open = 2,
}

/// Identify the playback-related state the `Dispatcher` is in.
#[derive(Clone, Debug)]
enum PlayerReadyState {
    /// No content is currently loaded.
    Stopped,

    /// We're preparing a content's playlist, MediaSource and SourceBuffers
    Loading {
        starting_position: Option<StartingPosition>,
    },

    /// The SourceBuffers are all ready but currently awaiting segments before
    /// being aple to play.
    AwaitingSegments,

    /// The content has enough segments to play.
    /// Note that this does not mean the media element is currently playing content:
    /// it can still be paused or at a `0` playback rate.
    Playing,
}

impl PlayerReadyState {
    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, PlayerReadyState::Loading { .. })
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct StartingPosition {
    start_type: StartingPositionType,
    position: f64,
}

#[wasm_bindgen]
impl StartingPosition {
    #[wasm_bindgen(constructor)]
    pub fn new(start_type: StartingPositionType, position: f64) -> Self {
        Self {
            start_type,
            position,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub enum StartingPositionType {
    Absolute,
    FromBeginning,
    FromEnd,
}
