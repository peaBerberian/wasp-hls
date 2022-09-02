use crate::{
    wasm_bindgen,
    bindings::{
        LogLevel,
        PlayerId,
        jsFetchU8,
    },
    Logger,
    media_element::MediaElementReference,
    utils::url::Url,
    requester::{Requester, PlaylistFileType},
    adaptive::AdaptiveQualitySelector,
    segment_selector::NextSegmentSelectors,
};

use super::{
    Dispatcher,
    PlayerReadyState,
};

/// Methods exposed to the JavaScript-side
#[wasm_bindgen]
impl Dispatcher {
    #[wasm_bindgen(constructor)]
    pub fn new(player_id: PlayerId) -> Self {
        Dispatcher {
            id: player_id,
            ready_state: PlayerReadyState::Stopped,
            adaptive_selector: AdaptiveQualitySelector::new(),
            content_tracker: None,
            requester: Requester::new(player_id),
            media_element_ref: MediaElementReference::new(player_id),
            last_position: 0.,
            buffer_goal: 30.,
            segment_selectors: NextSegmentSelectors::new(0., 30.),
        }
    }

    pub fn load_content(&mut self, content_url: String) {
        Logger::info("load_content called");
        self.stop();
        self.ready_state = PlayerReadyState::Loading;
        let content_url = Url::new(content_url);
        self.requester.fetch_playlist(content_url, PlaylistFileType::Unknown);
        Logger::info("Attaching MediaSource");
        if let Err(_) = self.media_element_ref.initialize() {
            // TODO handle exact error
            self.fail_on_error("Unknown error while trying to attach a MediaSource to the Media element");
        }
    }

    pub fn stop(&mut self) {
        self.internal_stop();
    }

    pub fn log(level: LogLevel, msg: &str) {
        match level {
            LogLevel::Error => Logger::error(msg),
            LogLevel::Warn => Logger::warn(msg),
            LogLevel::Info => Logger::info(msg),
            LogLevel::Debug => Logger::debug(msg),
        }
    }

    pub fn test_seg_back_and_forth(&self) {
        jsFetchU8(self.id, "http://127.0.0.1:8080/lowlat_vs_non_lowlat.mp4");
    }
}
