use crate::{
    adaptive::AdaptiveQualitySelector,
    bindings::{jsSendOtherError, OtherErrorCode},
    media_element::MediaElementReference,
    requester::{PlaylistFileType, Requester},
    segment_selector::NextSegmentSelectors,
    utils::url::Url,
    wasm_bindgen, Logger,
};

use super::{Dispatcher, PlayerReadyState};

/// Methods exposed to the JavaScript-side.
///
/// Note that these are not the only methods callable by JavaScript. There's
/// also "event_listeners" which as its name point at, should be called when particular
/// events happen. Such "event_listeners" are defined in its own file:
#[wasm_bindgen]
impl Dispatcher {
    /// Create a new `Dispatcher` allowing to load a content on the HTMLMediaElement that should be
    /// linked to it on the JavaScript-side.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Dispatcher {
            ready_state: PlayerReadyState::Stopped,
            adaptive_selector: AdaptiveQualitySelector::new(),
            playlist_store: None,
            requester: Requester::new(),
            media_element_ref: MediaElementReference::new(),
            last_position: 0.,
            buffer_goal: 30.,
            segment_selectors: NextSegmentSelectors::new(0., 30.),
            playlist_refresh_timers: vec![],
        }
    }

    /// Start loading a new content by communicating its MultiVariantPlaylist's URL
    pub fn load_content(&mut self, content_url: String) {
        Logger::info("load_content called");
        self.stop();
        self.ready_state = PlayerReadyState::Loading;
        let content_url = Url::new(content_url);
        self.requester
            .fetch_playlist(content_url, PlaylistFileType::Unknown);
        Logger::info("Attaching MediaSource");
        if let Err(x) = self.media_element_ref.attach_media_source() {
            jsSendOtherError(
                true,
                OtherErrorCode::MediaSourceAttachmentError,
                Some(&x.to_string()),
            );
            self.internal_stop();
        }
    }

    pub fn minimum_position(&self) -> Option<f64> {
        self.playlist_store
            .as_ref()
            .and_then(|c| c.curr_min_position())
    }

    pub fn maximum_position(&self) -> Option<f64> {
        self.playlist_store
            .as_ref()
            .and_then(|c| c.curr_max_position())
    }

    pub fn set_wanted_speed(&mut self, speed: f64) {
        self.media_element_ref.update_wanted_speed(speed);
    }

    pub fn set_buffer_goal(&mut self, buffer_goal: f64) {
        self.buffer_goal = buffer_goal;
        self.segment_selectors.update_buffer_goal(buffer_goal);
        self.check_segments_to_request();
    }

    /// Stop the currently loaded content.
    pub fn stop(&mut self) {
        self.internal_stop();
    }

    pub fn lock_variant(&mut self, variant_id: String) {
        self.inner_lock_variant(variant_id)
    }

    pub fn unlock_variant(&mut self) {
        self.inner_unlock_variant()
    }

    pub fn set_segment_request_timeout(&mut self, timeout: Option<f64>) {
        self.requester.update_segment_request_timeout(timeout);
    }

    pub fn set_segment_backoff_base(&mut self, base: f64) {
        self.requester.update_segment_backoff_base(base);
    }

    pub fn set_segment_backoff_max(&mut self, max: f64) {
        self.requester.update_segment_backoff_max(max);
    }

    pub fn set_multi_variant_playlist_request_timeout(&mut self, timeout: Option<f64>) {
        self.requester
            .update_multi_variant_playlist_request_timeout(timeout);
    }

    pub fn set_multi_variant_playlist_backoff_base(&mut self, base: f64) {
        self.requester
            .update_multi_variant_playlist_backoff_base(base);
    }

    pub fn set_multi_variant_playlist_backoff_max(&mut self, max: f64) {
        self.requester
            .update_multi_variant_playlist_backoff_max(max);
    }

    pub fn set_media_playlist_request_timeout(&mut self, timeout: Option<f64>) {
        self.requester
            .update_media_playlist_request_timeout(timeout);
    }

    pub fn set_media_playlist_backoff_base(&mut self, base: f64) {
        self.requester.update_media_playlist_backoff_base(base);
    }

    pub fn set_media_playlist_backoff_max(&mut self, max: f64) {
        self.requester.update_media_playlist_backoff_max(max);
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
