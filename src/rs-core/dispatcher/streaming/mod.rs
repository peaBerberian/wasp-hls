use crate::{
    bindings::{
        JsMemoryBlob,
        MediaObservation,
        MediaType,
        PlaybackTickReason,
        RequestId,
        SourceBufferId,
        TimerId,
        TimerReason,
        jsSetMediaSourceDuration,
        jsStartObservingPlayback,
        jsStopObservingPlayback,
        jsTimer,
        jsUpdateContentInfo,
        jsSendSegmentRequestError,
        RequestErrorReason,
        jsSendSourceBufferCreationError,
        jsSendPlaylistParsingError,
        PlaylistType,
        jsSendOtherError,
    },
    Logger,
    content_tracker::{MediaPlaylistPermanentId, ContentTracker},
    parser::MultiVariantPlaylist,
    requester::{
        PlaylistFileType,
        SegmentRequestInfo,
        PlaylistRequestInfo,
        FinishedRequestType,
        RetryResult,
    },
    media_element::{PushMetadata, SourceBufferCreationError},
    utils::url::Url,
    segment_selector::NextSegmentInfo,
};
use super::{
    MediaSourceReadyState,
    Dispatcher,
    PlayerReadyState,
};

impl Dispatcher {
    pub(crate) fn on_request_succeeded(&mut self,
        request_id: RequestId,
        data: JsMemoryBlob,
        final_url: Url,
        resource_size: u32,
        duration_ms: f64,
    ) {
        match self.requester.on_pending_request_success(request_id) {
            Some(FinishedRequestType::Segment(seg_info)) =>
                self.on_segment_fetch_success(seg_info, data, resource_size, duration_ms),
            Some(FinishedRequestType::Playlist(pl_info)) =>
                self.on_playlist_fetch_success(pl_info, data.obtain(), final_url),
            _ => Logger::warn("Unknown request finished"),
        }
    }

    pub(crate) fn on_request_failed_inner(&mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>
    ) {
        match self.requester.on_pending_request_failure(request_id, has_timeouted, status) {
            RetryResult::Failed(FinishedRequestType::Segment(s)) => {
                jsSendSegmentRequestError(
                    true,
                    s.url.get_ref(),
                    s.time_info.is_none(),
                    s.time_info.map(|t| vec![t.0, t.1]),
                    s.media_type,
                    RequestErrorReason::Other, // TODO
                    None);
                self.internal_stop();
            },
            RetryResult::Failed(FinishedRequestType::Playlist(_)) => {
                jsSendOtherError(true, crate::bindings::OtherErrorCode::Unknown,
                    Some(&"Failed to fetch Playlist"));
                self.internal_stop();
            },
            _ => {},
        }
    }

    /// Method called on various events that could lead to start loading segments.
    /// If all conditions are met, the `Dispatcher` is set to the next
    /// `AwaitingSegments` `PlayerReadyState`, playback observations begin,
    /// and the potential initialization segments are requested.
    pub(super) fn check_ready_to_load_segments(&mut self) {
        if self.ready_state >= PlayerReadyState::AwaitingSegments {
            return;
        }

        match self.media_element_ref.media_source_ready_state() {
            Some(MediaSourceReadyState::Closed) |
            None => { return; },
            _ => {},
        }

        let content_tracker = if let Some(ctnt) = self.content_tracker.as_ref() {
            ctnt
        } else { return; };
        if content_tracker.all_curr_media_playlists_ready() {
            self.ready_state = PlayerReadyState::AwaitingSegments;
            let start_time = content_tracker.get_expected_start_time();
            if start_time > 0. {
                self.media_element_ref.seek(start_time);
            }
            if let Some(duration) = content_tracker.curr_duration() {
                jsSetMediaSourceDuration(duration);
            } else {
                Logger::warn("Unknown content duration");
            }

            if let Some(Err(e)) = self.init_source_buffer(MediaType::Audio) {
                handle_source_buffer_creation_error(e);
                self.internal_stop();
                return;
            }
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Video) {
                handle_source_buffer_creation_error(e);
                self.internal_stop();
                return;
            }
            jsStartObservingPlayback();
        }
    }

    /// Method called as a MultiVariant Playlist is loaded
    pub(super) fn on_multivariant_playlist_loaded(&mut self,
        data: Vec<u8>,
        playlist_url: Url
    ) {
        match MultiVariantPlaylist::parse(data.as_ref(), playlist_url) {
            Err(e) => {
                jsSendPlaylistParsingError(
                    true,
                    PlaylistType::MultiVariantPlaylist,
                    None,
                    Some(&e.to_string())
                );
                self.internal_stop();
            },
            Ok(pl) => {
                Logger::info("MultiVariant Playlist parsed successfully");
                self.content_tracker = Some(ContentTracker::new(pl));
                let content_tracker = self.content_tracker.as_mut().unwrap();
                if content_tracker.variants().is_empty() {
                    jsSendPlaylistParsingError(
                        true,
                        PlaylistType::MultiVariantPlaylist,
                        None,
                        Some("Error while parsing MultiVariantPlaylist: no variant found.")
                    );
                    self.internal_stop();
                    return;
                }

                use PlaylistFileType::*;
                // TODO lowest/latest bandwidth first?
                content_tracker.update_curr_bandwidth(2_000_000.);
                if let Some((url, id)) = content_tracker.curr_media_playlist_request_info(MediaType::Video) {
                    let id = id.clone();
                    let url = url.clone();
                    self.requester.fetch_playlist(url, MediaPlaylist { id });
                }
                if let Some((url, id)) = content_tracker.curr_media_playlist_request_info(MediaType::Audio) {
                    let id = id.clone();
                    let url = url.clone();
                    self.requester.fetch_playlist(url, MediaPlaylist { id });
                }
            },
        }
    }

    /// Method called as a MediaPlaylist Playlist is loaded
    pub(super) fn on_media_playlist_loaded(&mut self,
        playlist_id: MediaPlaylistPermanentId,
        data: Vec<u8>,
        playlist_url: Url
    ) {
        Logger::info(&format!("Media playlist loaded successfully: {}", playlist_url.get_ref()));
        if let Some(ref mut content_tracker) = self.content_tracker.as_mut() {
            match content_tracker.update_media_playlist(&playlist_id, data.as_ref(), playlist_url) {
                Err(e) => {
                    jsSendPlaylistParsingError(
                        true,
                        PlaylistType::MediaPlaylist,
                        None, // TODO? Or maybe at least the URL
                        Some(&e.to_string())
                    );
                    self.internal_stop();
                },
                Ok(p) => {
                    if !p.end_list {
                        let target_duration = p.target_duration;
                        let timer_id = jsTimer(target_duration as f64 * 1000., TimerReason::MediaPlaylistRefresh);
                        let url = p.url.clone();
                        self.playlist_refresh_timers.push(
                            (timer_id, url, PlaylistFileType::MediaPlaylist { id: playlist_id }));
                    }
                    if self.ready_state == PlayerReadyState::Loading {
                        self.check_ready_to_load_segments();
                    } else if self.ready_state > PlayerReadyState::Loading {
                        self.check_segments_to_request();
                    }

                    if let Some(content_tracker) = self.content_tracker.as_ref() {
                        let min_pos = content_tracker.curr_min_position();
                        let max_pos = content_tracker.curr_max_position();
                        jsUpdateContentInfo( min_pos, max_pos);
                    }
                },
            }
        } else {
            jsSendOtherError(true, crate::bindings::OtherErrorCode::Unknown,
                Some(&"Media playlist loaded but no MultiVariantPlaylist"));
            self.internal_stop();
        }
    }

    fn init_source_buffer(&mut self,
        media_type: MediaType
    ) -> Option<Result<(), SourceBufferCreationError>> {
        let content = self.content_tracker.as_mut()?;

        let media_playlist = content.curr_media_playlist(media_type)?;
        let mime_type = media_playlist.mime_type(media_type).unwrap_or("");

        // TODO to_string should be unneeded here as &str is sufficient
        let codecs = content.curr_variant()?.codecs(media_type).unwrap_or("".to_string());
        Some(self.media_element_ref.create_source_buffer(media_type, mime_type, &codecs))
    }

    pub(crate) fn internal_on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        Logger::info(&format!("MediaSource state changed: {:?}", state));
        self.media_element_ref.update_media_source_ready_state(state);
        if state == MediaSourceReadyState::Open {
            self.check_ready_to_load_segments();
        }
    }

    pub(crate) fn internal_on_source_buffer_update(&mut self,
        source_buffer_id: SourceBufferId
    ) {
        self.media_element_ref.on_source_buffer_update(source_buffer_id);
    }

    pub(crate) fn internal_on_source_buffer_error(&mut self,
        _source_buffer_id: SourceBufferId
    ) {
        // TODO check QuotaExceededError and so on...
        // TODO better error
        jsSendOtherError(true, crate::bindings::OtherErrorCode::Unknown,
            Some("A SourceBuffer emitted an error"));
        self.internal_stop();
    }

    pub fn on_observation(&mut self, observation: MediaObservation) {
        let reason = observation.reason();
        Logger::debug(&format!("Tick received: {:?} {}",
                reason, observation.current_time()));
        self.media_element_ref.on_observation(observation);
        match reason {
            PlaybackTickReason::Seeking => self.on_seek(),
            _ => self.on_regular_tick(),
        }
    }

    pub fn on_regular_tick(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.last_position = wanted_pos;

        // Lock `Requester`, so it only do new segment requests when every
        // wanted segments is scheduled - for better priorization
        let was_already_locked = self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.segment_selectors.update_base_position(wanted_pos);
        self.check_segments_to_request();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    pub fn on_seek(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.segment_selectors.reset_position(wanted_pos);

        // TODO better logic than aborting everything on seek?
        self.requester.abort_all_segments();
        self.requester.update_base_position(Some(wanted_pos));
        self.check_segments_to_request()
    }

    pub fn check_segments_to_request(&mut self) {
        match self.content_tracker.as_ref() {
            None => {},
            Some(ctnt) => {
                let was_already_locked = self.requester.lock_segment_requests();
                [MediaType::Video, MediaType::Audio].iter().for_each(|mt| {
                    let mt = *mt;
                    if !self.requester.has_segment_request_pending(mt) {
                        if let Some(pl) = ctnt.curr_media_playlist(mt) {
                            match self.segment_selectors.get_mut(mt).get_next_segment_info(pl) {
                                NextSegmentInfo::None => {},
                                NextSegmentInfo::InitSegment(i) =>
                                    self.requester.request_init_segment(mt, i.uri.clone()),
                                NextSegmentInfo::MediaSegment(seg) =>
                                    self.requester.request_media_segment(mt, seg),
                            }
                        }
                    }
                });
                if !was_already_locked {
                    self.requester.unlock_segment_requests();
                }
            },
        }
    }

    pub fn internal_stop(&mut self) {
        Logger::info("Stopping current content (if one) and resetting player");
        self.requester.reset();
        jsStopObservingPlayback();
        self.media_element_ref.reset();
        self.segment_selectors.reset_position(0.);
        self.content_tracker = None;
        self.last_position = 0.;
        self.playlist_refresh_timers.clear();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Method called once a playlist request ended with success
    pub(super) fn on_playlist_fetch_success(&mut self,
        pl_info: PlaylistRequestInfo,
        result: Vec<u8>,
        final_url: Url
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        if let PlaylistFileType::MediaPlaylist { id, .. } = playlist_type {
            self.on_media_playlist_loaded(id, result, final_url);
        } else {
            self.on_multivariant_playlist_loaded(result, final_url);
        }
    }

    fn push_and_validate_segment(&mut self,
        data: JsMemoryBlob,
        media_type: MediaType,
        time_info: Option<(f64, f64)>
    ) {
        let md = PushMetadata::new(data, time_info);
        match self.media_element_ref.push_segment(media_type, md) {
            Err(x) => {
                jsSendOtherError(true, crate::bindings::OtherErrorCode::Unknown,
                    Some(&format!("Can't push {} segment: {:?}", media_type, x)));
                self.internal_stop();
                return;
            }
            Ok(()) => if let Some(ti) = time_info {
                self.segment_selectors.get_mut(media_type).validate_media(ti.0);
                if was_last_segment(self.content_tracker.as_ref(), media_type, ti.0) {
                    Logger::info(&format!("Last {} segment request finished, declaring its buffer's end", media_type));
                    self.media_element_ref.end_buffer(media_type);
                }
            } else {
                self.segment_selectors.get_mut(media_type).validate_init();
            }
        }
    }

    fn process_request_metrics(&mut self,
        resource_size: u32,
        duration_ms: f64,
    ) {
        self.adaptive_selector.add_metric(duration_ms, resource_size);
        if let Some(ctnt) = self.content_tracker.as_mut() {
            if let Some(bandwidth) = self.adaptive_selector.get_estimate() {
                Logger::debug(&format!("New bandwidth estimate: {}", bandwidth));
                ctnt.update_curr_bandwidth(bandwidth).iter().for_each(|mt| {
                    let mt = *mt;
                    Logger::info(&format!("{} MediaPlaylist changed", mt));
                    self.requester.abort_segments_with_type(mt);
                    let selector = self.segment_selectors.get_mut(mt);
                    selector.rollback();
                    selector.reset_init_segment();
                    if ctnt.curr_media_playlist(mt).is_none() {
                        if let Some((url, id)) = ctnt.curr_media_playlist_request_info(mt) {
                            use PlaylistFileType::*;
                            Logger::debug("Media changed, requesting its media playlist");
                            let id = id.clone();
                            let url = url.clone();
                            self.requester.fetch_playlist(url, MediaPlaylist { id });
                        }
                    }
                })
            }
        }
    }

    /// Method called once a segment request ended with success
    pub(super) fn on_segment_fetch_success(&mut self,
        segment_req: SegmentRequestInfo,
        result: JsMemoryBlob,
        resource_size: u32,
        duration_ms: f64,
    ) {
        Logger::lazy_info(&|| {
            let media_type = segment_req.media_type;
            match segment_req.time_info {
                None => format!("Loaded {} init segment", media_type),
                Some((start, end)) =>
                    format!("Loaded {} segment: t: {}, d: {}", media_type, start, end - start),
            }
        });

        self.push_and_validate_segment(result, segment_req.media_type, segment_req.time_info);
        self.process_request_metrics(resource_size, duration_ms);
        self.check_segments_to_request();
    }

    pub fn on_playlist_refresh_timer_ended(&mut self, id: TimerId) {
        let found = self.playlist_refresh_timers.iter().position(|x| {
            x.0 == id
        });
        if let Some(idx) = found {
            let (_, url, playlist_type) = self.playlist_refresh_timers.remove(idx);
            let url = url.clone();
            self.requester.fetch_playlist(url, playlist_type);
        }
    }

    pub fn on_retry_request(&mut self, id: TimerId) {
        self.requester.on_timer_finished(id);
    }
}

fn was_last_segment(
    content_tracker: Option<&ContentTracker>,
    media_type: MediaType,
    seg_start: f64,
) -> bool {
    match content_tracker {
        None => false,
        Some(ctnt) => {
            match ctnt.curr_media_playlist(media_type) {
                None => false,
                Some(pl) => {
                    pl.last_segment_start() == Some(seg_start)
                },
            }
        },
    }
}

fn handle_source_buffer_creation_error(err: SourceBufferCreationError) {
    match err {
        SourceBufferCreationError::EmptyMimeType =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::EmptyMimeType,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::NoMediaSourceAttached { .. } =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::NoMediaSourceAttached,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::MediaSourceIsClosed =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::MediaSourceIsClosed,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::QuotaExceededError { .. } =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::QuotaExceededError,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::CantPlayType { .. } =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::CantPlayType,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::AlreadyCreatedWithSameType { .. } =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::AlreadyCreatedWithSameType,
                Some(&err.to_string())
            ),
        SourceBufferCreationError::UnknownError { .. } =>
            jsSendSourceBufferCreationError(
                crate::bindings::SourceBufferCreationErrorCode::Unknown,
                Some(&err.to_string())
            ),
    }
}

