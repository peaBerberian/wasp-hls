use crate::{
    wasm_bindgen,
    bindings::LogLevel,
    Logger,
    media_element::MediaElementReference,
    utils::url::Url,
    requester::{Requester, PlaylistFileType},
    adaptive::AdaptiveQualitySelector,
    segment_selector::NextSegmentSelectors,
    content_tracker::AvailableAudioTrack,
};

use super::{
    Dispatcher,
    PlayerReadyState,
};

/// Methods exposed to the JavaScript-side
#[wasm_bindgen]
impl Dispatcher {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Dispatcher {
            ready_state: PlayerReadyState::Stopped,
            adaptive_selector: AdaptiveQualitySelector::new(),
            content_tracker: None,
            requester: Requester::new(),
            media_element_ref: MediaElementReference::new(),
            last_position: 0.,
            buffer_goal: 30.,
            segment_selectors: NextSegmentSelectors::new(0., 30.),
            playlist_refresh_timers: vec![],
        }
    }

    pub fn load_content(&mut self, content_url: String) {
        Logger::info("load_content called");
        self.stop();
        self.ready_state = PlayerReadyState::Loading;
        let content_url = Url::new(content_url);
        self.requester.fetch_playlist(content_url, PlaylistFileType::Unknown);
        Logger::info("Attaching MediaSource");
        self.media_element_ref.attach_media_source();
    }

    pub fn get_available_audio_tracks(&self) -> Vec<u8> {
        match self.content_tracker {
            None => vec![],
            Some(ref c) => c.todo_get_available_audio_tracks()
                .iter()
                .map(|x| x.serialize_for_js())
                .flatten()
                .collect(),
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
}

impl<'a> AvailableAudioTrack<'a> {
    // Byte 1-4: Length of the AudioTrack's inner data, as a little-endian u32
    // Byte 5-9: id of the track as a little-endian u32
    // Byte 10: `1` if current / `0` if not
    // Byte 11: length of the language. `0` if no language.
    // Byte 12-x: language as utf-8
    // Byte x+1: length of the assoc_language. `0` if no assoc_language.
    // Byte x+2-y: assoc_language as utf-8
    // Byte y+1: length of the track's name. `0` if no name.
    // Byte y+2-z: name as utf-8
    // Byte z+1-z+5: Channels as a little-endian u32. `0` if unknown
    fn serialize_for_js(&'a self) -> Vec<u8> {
        let mut track_info = vec![];
        // Set length at 0 for now
        track_info.push(0);
        track_info.push(0);
        track_info.push(0);
        track_info.push(0);

        let mut current_length = 4;

        let id_u8 = self.id().to_le_bytes(); current_length += 4;
        track_info.extend(id_u8);
        track_info.push(if self.is_current() { 1 } else { 0 }); current_length += 1;
        if let Some(lang) = self.language() {
            let len = lang.len();
            track_info.extend(len.to_le_bytes());
            track_info.extend(lang.as_bytes());
            current_length += 4 + len;
        } else {
            track_info.push(0);
            track_info.push(0);
            track_info.push(0);
            track_info.push(0);
            current_length += 4;
        }
        if let Some(assoc_lang) = self.assoc_language() {
            let len = assoc_lang.len();
            track_info.extend(len.to_le_bytes());
            track_info.extend(assoc_lang.as_bytes());
            current_length += 4 + len;
        } else {
            track_info.push(0);
            track_info.push(0);
            track_info.push(0);
            track_info.push(0);
            current_length += 4;
        }

        let name = self.name();
        let name_len = name.len();
        track_info.extend(name_len.to_le_bytes());
        track_info.extend(name.as_bytes());
        current_length += 4 + name_len;

        track_info.extend(if let Some(val) = self.channels() {
            val.to_le_bytes()
        } else {
            [0; 4]
        });
        current_length += 4;

        let current_length_le = current_length.to_le_bytes();
        track_info[0] = current_length_le[0];
        track_info[1] = current_length_le[1];
        track_info[2] = current_length_le[2];
        track_info[3] = current_length_le[3];

        track_info
    }
}