use crate::{
    adaptive::AdaptiveQualitySelector,
    bindings::{jsSendOtherError, OtherErrorCode},
    media_element::MediaElementReference,
    requester::{PlaylistFileType, Requester},
    segment_selector::NextSegmentSelectors,
    utils::url::Url,
    wasm_bindgen, Logger,
};

use super::{Dispatcher, PlayerReadyState, StartingPosition};

/// Methods exposed to the JavaScript-side.
///
/// Note that these are not the only methods callable by JavaScript. There's
/// also "event_listeners" which as its name point at, should be called when particular
/// events happen. Such "event_listeners" are defined in its own file.
#[wasm_bindgen]
impl Dispatcher {
    /// Create a new `Dispatcher` allowing to load a content on the HTMLMediaElement that should be
    /// linked to it on the JavaScript-side.
    #[wasm_bindgen(constructor)]
    pub fn new(initial_bandwidth: f64) -> Self {
        Dispatcher {
            ready_state: PlayerReadyState::Stopped,
            adaptive_selector: AdaptiveQualitySelector::new(initial_bandwidth),
            playlist_store: None,
            requester: Requester::new(),
            media_element_ref: MediaElementReference::new(),
            last_position: 0.,
            buffer_goal: 30.,
            segment_selectors: NextSegmentSelectors::new(0., 30.),
            playlist_refresh_timers: vec![],
        }
    }

    /// Start loading a new content by communicating its MultivariantPlaylist's URL
    pub fn load_content(&mut self, content_url: String, starting_pos: Option<StartingPosition>) {
        Logger::info("load_content called");
        self.stop();
        self.ready_state = PlayerReadyState::Loading {
            starting_position: starting_pos,
        };
        let content_url = Url::new(content_url);
        self.requester
            .fetch_playlist(content_url, PlaylistFileType::MultivariantPlaylist);
        Logger::info("Attaching MediaSource");
        if let Err(x) = self.media_element_ref.attach_media_source() {
            jsSendOtherError(
                true,
                OtherErrorCode::MediaSourceAttachmentError,
                &x.to_string(),
            );
            self.stop_current_content();
        }
    }

    /// Returns the minimum position, in playlist time in seconds, at which media segments can be
    /// loaded currently in the content.
    ///
    /// Returns `None` if unknown or if no content is loaded yet.
    pub fn minimum_position(&self) -> Option<f64> {
        self.playlist_store
            .as_ref()
            .and_then(|c| c.curr_min_position())
    }

    /// Returns the maximum position, in playlist time in seconds, at which media segments can be
    /// loaded currently in the content.
    ///
    /// Returns `None` if unknown or if no content is loaded yet.
    pub fn maximum_position(&self) -> Option<f64> {
        self.playlist_store
            .as_ref()
            .and_then(|c| c.curr_max_position())
    }

    /// Set the wanted playback rate, at which we will play when not rebuffering.
    pub fn set_wanted_speed(&mut self, speed: f64) {
        self.media_element_ref.update_wanted_speed(speed);
        self.check_best_variant();
    }

    /// Update the buffer goal to the given value.
    ///
    /// The buffer goal is the amount of buffer, ahead of the current position we want to build in
    /// seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    pub fn set_buffer_goal(&mut self, buffer_goal: f64) {
        self.buffer_goal = buffer_goal;
        self.segment_selectors.update_buffer_goal(buffer_goal);
        self.check_segments_to_request();
    }

    /// Stop the currently loaded content.
    pub fn stop(&mut self) {
        self.stop_current_content();
    }

    /// Begin "locking" HLS variant whose `id` is given in argument, meaning that we will keep only
    /// playing that one.
    pub fn lock_variant(&mut self, variant_id: u32) {
        self.lock_variant_core(variant_id)
    }

    /// Remove an HLS variant previously put in place through `lock_variant`.
    pub fn unlock_variant(&mut self) {
        self.unlock_variant_core()
    }

    /// Set an audio track whose `id` is given in argument.
    pub fn set_audio_track(&mut self, track_id: Option<u32>) {
        self.set_audio_track_core(track_id)
    }

    pub fn set_segment_request_max_retry(&mut self, max_retry: i32) {
        self.requester.config_mut().segment_request_max_retry = max_retry;
    }

    pub fn set_segment_request_timeout(&mut self, timeout: f64) {
        self.requester.config_mut().segment_request_timeout = timeout;
    }

    pub fn set_segment_backoff_base(&mut self, base: f64) {
        self.requester.config_mut().segment_backoff_base = base;
    }

    pub fn set_segment_backoff_max(&mut self, max: f64) {
        self.requester.config_mut().segment_backoff_max = max;
    }

    pub fn set_multi_variant_playlist_request_max_retry(&mut self, max_retry: i32) {
        self.requester.config_mut().multi_variant_playlist_max_retry = max_retry;
    }

    pub fn set_multi_variant_playlist_request_timeout(&mut self, timeout: f64) {
        self.requester
            .config_mut()
            .multi_variant_playlist_request_timeout = timeout;
    }

    pub fn set_multi_variant_playlist_backoff_base(&mut self, base: f64) {
        self.requester
            .config_mut()
            .multi_variant_playlist_backoff_base = base;
    }

    pub fn set_multi_variant_playlist_backoff_max(&mut self, max: f64) {
        self.requester
            .config_mut()
            .multi_variant_playlist_backoff_max = max;
    }

    pub fn set_media_playlist_request_max_retry(&mut self, max_retry: i32) {
        self.requester.config_mut().media_playlist_max_retry = max_retry;
    }

    pub fn set_media_playlist_request_timeout(&mut self, timeout: f64) {
        self.requester.config_mut().media_playlist_request_timeout = timeout;
    }

    pub fn set_media_playlist_backoff_base(&mut self, base: f64) {
        self.requester.config_mut().media_playlist_backoff_base = base;
    }

    pub fn set_media_playlist_backoff_max(&mut self, max: f64) {
        self.requester.config_mut().media_playlist_backoff_max = max;
    }
}
