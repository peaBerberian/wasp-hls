use crate::{
    adaptive::AdaptiveQualitySelector,
    bindings::{jsSendOtherError, OtherErrorCode},
    dispatcher::{
        segment_request_contexts::SegmentRequestContexts, InitialAudioTrackSelection,
        PlaylistRefreshTimers,
    },
    media_element::MediaElementReference,
    requester::{PlaylistFileType, Requester},
    segment_selector::NextSegmentSelectors,
    utils::{logger::*, url::Url},
};
use std::convert::TryInto;
use std::slice;

use super::{Dispatcher, PlayerReadyState, StartingPosition, StartingPositionType};

const UNSET_STRING_LENGTH: u32 = u32::MAX;

/// Methods exposed to the JavaScript-side.
///
/// Note that these are not the only methods callable by JavaScript. There's
/// also "event_listeners" which as its name point at, should be called when particular
/// events happen. Such "event_listeners" are defined in its own file.
impl Dispatcher {
    /// Create a new `Dispatcher` allowing to load a content on the HTMLMediaElement that should be
    /// linked to it on the JavaScript-side.
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
            segment_request_contexts: SegmentRequestContexts::new(),
            playlist_refresh_timers: PlaylistRefreshTimers::new(),
            ready_probe_segments: Default::default(),
            initial_audio_track_selection: Vec::new(),
        }
    }

    /// Start loading a new content by communicating its MultivariantPlaylist's URL
    pub fn load_content(
        &mut self,
        content_url: String,
        starting_pos: Option<StartingPosition>,
        initial_audio_track_selection: Vec<InitialAudioTrackSelection>,
    ) {
        log_info!("load_content called");
        self.stop();
        self.initial_audio_track_selection = initial_audio_track_selection
            .into_iter()
            .filter(|selection| !selection.is_empty())
            .collect();
        self.ready_state = PlayerReadyState::AwaitingPlaylistInfo {
            starting_position: starting_pos,
            lifecycle_announced: false,
        };
        let content_url = Url::new(content_url);
        self.requester
            .fetch_playlist(content_url, PlaylistFileType::TopLevelPlaylist);
        log_info!("Attaching MediaSource");
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
            .and_then(|c| c.current_estimated_minimum_position())
    }

    /// Returns the maximum position, in playlist time in seconds, at which media segments can be
    /// loaded currently in the content.
    ///
    /// Returns `None` if unknown or if no content is loaded yet.
    pub fn maximum_position(&self) -> Option<f64> {
        self.playlist_store
            .as_ref()
            .and_then(|c| c.current_estimated_maximum_position())
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
        self.check_best_variant();
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

fn string_from_abi(ptr: *const u8, len: u32) -> String {
    let bytes = unsafe { slice::from_raw_parts(ptr, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn opt_string_from_abi(ptr: *const u8, len: u32) -> Option<String> {
    if ptr.is_null() || len == 0 {
        None
    } else {
        Some(string_from_abi(ptr, len))
    }
}

fn opt_u8_slice_from_abi<'a>(ptr: *const u8, len: u32) -> Option<&'a [u8]> {
    if ptr.is_null() || len == 0 {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(ptr, len as usize) })
    }
}

fn read_u32_le(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let raw: [u8; 4] = bytes.get(*offset..end)?.try_into().ok()?;
    *offset = end;
    Some(u32::from_le_bytes(raw))
}

fn read_string(bytes: &[u8], offset: &mut usize) -> Option<String> {
    let length = read_u32_le(bytes, offset)? as usize;
    let end = offset.checked_add(length)?;
    let value = String::from_utf8_lossy(bytes.get(*offset..end)?).into_owned();
    *offset = end;
    Some(value)
}

fn read_optional_string(bytes: &[u8], offset: &mut usize) -> Option<Option<String>> {
    let length = read_u32_le(bytes, offset)?;
    if length == UNSET_STRING_LENGTH {
        return Some(None);
    }
    let length = length as usize;
    let end = offset.checked_add(length)?;
    let value = String::from_utf8_lossy(bytes.get(*offset..end)?).into_owned();
    *offset = end;
    Some(Some(value))
}

fn deserialize_initial_audio_track_selection(
    initial_audio_ptr: *const u8,
    initial_audio_len: u32,
) -> Vec<InitialAudioTrackSelection> {
    let Some(bytes) = opt_u8_slice_from_abi(initial_audio_ptr, initial_audio_len) else {
        return Vec::new();
    };
    let mut offset = 0;
    let Some(selection_count) = read_u32_le(bytes, &mut offset) else {
        return Vec::new();
    };

    let mut selections = Vec::with_capacity(selection_count as usize);
    for _ in 0..selection_count {
        let Some(language) = read_optional_string(bytes, &mut offset) else {
            return Vec::new();
        };
        let Some(assoc_language) = read_optional_string(bytes, &mut offset) else {
            return Vec::new();
        };
        let Some(name) = read_optional_string(bytes, &mut offset) else {
            return Vec::new();
        };
        let Some(characteristics_count) = read_u32_le(bytes, &mut offset) else {
            return Vec::new();
        };
        let mut characteristics = Vec::with_capacity(characteristics_count as usize);
        for _ in 0..characteristics_count {
            let Some(characteristic) = read_string(bytes, &mut offset) else {
                return Vec::new();
            };
            characteristics.push(characteristic);
        }
        let Some(has_channels) = bytes.get(offset) else {
            return Vec::new();
        };
        offset += 1;
        let channels = if *has_channels == 0 {
            None
        } else {
            let Some(value) = read_u32_le(bytes, &mut offset) else {
                return Vec::new();
            };
            Some(value)
        };

        selections.push(InitialAudioTrackSelection {
            language,
            assoc_language,
            name,
            characteristics,
            channels,
        });
    }
    selections
}

fn dispatcher_mut<'a>(ptr: u32) -> &'a mut Dispatcher {
    unsafe { &mut *(ptr as *mut Dispatcher) }
}

fn dispatcher_ref<'a>(ptr: u32) -> &'a Dispatcher {
    unsafe { &*(ptr as *const Dispatcher) }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_new(initial_bandwidth: f64) -> u32 {
    Box::into_raw(Box::new(Dispatcher::new(initial_bandwidth))) as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_free(ptr: u32) {
    if ptr != 0 {
        unsafe {
            drop(Box::from_raw(ptr as *mut Dispatcher));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_set_logger_level(level: u32) {
    set_log_level(LoggerLevel::from_u32(level));
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_load_content(
    ptr: u32,
    content_url_ptr: *const u8,
    content_url_len: u32,
    has_starting_pos: u32,
    start_type: u32,
    start_position: f64,
    initial_audio_ptr: *const u8,
    initial_audio_len: u32,
) {
    let content_url = string_from_abi(content_url_ptr, content_url_len);
    let starting_pos = if has_starting_pos != 0 {
        Some(StartingPosition::new(
            StartingPositionType::from_raw(start_type),
            start_position,
        ))
    } else {
        None
    };
    let initial_audio_track_selection =
        deserialize_initial_audio_track_selection(initial_audio_ptr, initial_audio_len);
    dispatcher_mut(ptr).load_content(content_url, starting_pos, initial_audio_track_selection);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_minimum_position(ptr: u32) -> f64 {
    dispatcher_ref(ptr).minimum_position().unwrap_or(f64::NAN)
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_maximum_position(ptr: u32) -> f64 {
    dispatcher_ref(ptr).maximum_position().unwrap_or(f64::NAN)
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_wanted_speed(ptr: u32, speed: f64) {
    dispatcher_mut(ptr).set_wanted_speed(speed);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_buffer_goal(ptr: u32, buffer_goal: f64) {
    dispatcher_mut(ptr).set_buffer_goal(buffer_goal);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_stop(ptr: u32) {
    dispatcher_mut(ptr).stop();
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_lock_variant(ptr: u32, variant_id: u32) {
    dispatcher_mut(ptr).lock_variant(variant_id);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_unlock_variant(ptr: u32) {
    dispatcher_mut(ptr).unlock_variant();
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_audio_track(ptr: u32, track_id: u32) {
    dispatcher_mut(ptr).set_audio_track(if track_id == u32::MAX {
        None
    } else {
        Some(track_id)
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_segment_request_max_retry(ptr: u32, max_retry: i32) {
    dispatcher_mut(ptr).set_segment_request_max_retry(max_retry);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_segment_request_timeout(ptr: u32, timeout: f64) {
    dispatcher_mut(ptr).set_segment_request_timeout(timeout);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_segment_backoff_base(ptr: u32, base: f64) {
    dispatcher_mut(ptr).set_segment_backoff_base(base);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_segment_backoff_max(ptr: u32, max: f64) {
    dispatcher_mut(ptr).set_segment_backoff_max(max);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_multi_variant_playlist_request_max_retry(
    ptr: u32,
    max_retry: i32,
) {
    dispatcher_mut(ptr).set_multi_variant_playlist_request_max_retry(max_retry);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_multi_variant_playlist_request_timeout(
    ptr: u32,
    timeout: f64,
) {
    dispatcher_mut(ptr).set_multi_variant_playlist_request_timeout(timeout);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_multi_variant_playlist_backoff_base(ptr: u32, base: f64) {
    dispatcher_mut(ptr).set_multi_variant_playlist_backoff_base(base);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_multi_variant_playlist_backoff_max(ptr: u32, max: f64) {
    dispatcher_mut(ptr).set_multi_variant_playlist_backoff_max(max);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_media_playlist_request_max_retry(ptr: u32, max_retry: i32) {
    dispatcher_mut(ptr).set_media_playlist_request_max_retry(max_retry);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_media_playlist_request_timeout(ptr: u32, timeout: f64) {
    dispatcher_mut(ptr).set_media_playlist_request_timeout(timeout);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_media_playlist_backoff_base(ptr: u32, base: f64) {
    dispatcher_mut(ptr).set_media_playlist_backoff_base(base);
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_dispatcher_set_media_playlist_backoff_max(ptr: u32, max: f64) {
    dispatcher_mut(ptr).set_media_playlist_backoff_max(max);
}
