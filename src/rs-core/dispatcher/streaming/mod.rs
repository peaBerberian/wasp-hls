use std::cmp::Ordering;

use super::{Dispatcher, MediaSourceReadyState, PlayerReadyState};
use crate::{
    bindings::{
        formatters::{
            format_audio_tracks_for_js, format_source_buffer_creation_err_for_js,
            format_variants_info_for_js,
        },
        jsAnnounceFetchedContent, jsAnnounceTrackUpdate, jsAnnounceVariantUpdate, jsSendOtherError,
        jsSendPlaylistParsingError, jsSendSegmentRequestError, jsSendSourceBufferCreationError,
        jsSetMediaSourceDuration, jsStartObservingPlayback, jsStopObservingPlayback, jsTimer,
        jsUpdateContentInfo, JsMemoryBlob, MediaObservation, MediaType, PlaybackTickReason,
        PlaylistType, RequestId, SourceBufferId, TimerId, TimerReason,
    },
    media_element::{PushMetadata, SourceBufferCreationError},
    parser::MultiVariantPlaylist,
    playlist_store::{MediaPlaylistPermanentId, PlaylistStore, VariantUpdateResult},
    requester::{
        FinishedRequestType, PlaylistFileType, PlaylistRequestInfo, RetryResult, SegmentRequestInfo,
    },
    segment_selector::NextSegmentInfo,
    utils::url::Url,
    Logger,
};

impl Dispatcher {
    pub(crate) fn on_request_succeeded(
        &mut self,
        request_id: RequestId,
        data: JsMemoryBlob,
        final_url: Url,
        resource_size: u32,
        duration_ms: f64,
    ) {
        match self.requester.on_pending_request_success(request_id) {
            Some(FinishedRequestType::Segment(seg_info)) => {
                self.on_segment_fetch_success(seg_info, data, resource_size, duration_ms)
            }
            Some(FinishedRequestType::Playlist(pl_info)) => {
                self.on_playlist_fetch_success(pl_info, data.obtain(), final_url)
            }
            _ => Logger::warn("Unknown request finished"),
        }
    }

    pub(crate) fn on_request_failed_inner(
        &mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>,
    ) {
        match self
            .requester
            .on_pending_request_failure(request_id, has_timeouted, status)
        {
            RetryResult::Failed((FinishedRequestType::Segment(s), reason)) => {
                jsSendSegmentRequestError(
                    true,
                    s.url.get_ref(),
                    s.time_info.is_none(),
                    s.time_info.map(|t| vec![t.0, t.1]),
                    s.media_type,
                    reason,
                    None,
                ); // TODO status
                self.internal_stop();
            }
            RetryResult::Failed((FinishedRequestType::Playlist(_), _reason)) => {
                jsSendOtherError(
                    true,
                    crate::bindings::OtherErrorCode::Unknown,
                    Some("Failed to fetch Playlist"),
                );
                self.internal_stop();
            }
            _ => {}
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
            Some(MediaSourceReadyState::Closed) | None => {
                return;
            }
            _ => {}
        }

        let playlist_store = if let Some(pl) = self.playlist_store.as_ref() {
            pl
        } else {
            return;
        };
        if playlist_store.are_playlists_ready() {
            self.ready_state = PlayerReadyState::AwaitingSegments;
            let start_time = playlist_store.expected_start_time();
            if start_time > 0. {
                self.media_element_ref.seek(start_time);
            }
            if let Some(duration) = playlist_store.curr_duration() {
                jsSetMediaSourceDuration(duration);
            } else {
                Logger::warn("Unknown content duration");
            }

            if let Some(Err(e)) = self.init_source_buffer(MediaType::Audio) {
                let (code, msg) = format_source_buffer_creation_err_for_js(e);
                jsSendSourceBufferCreationError(code, Some(&msg));
                self.internal_stop();
                return;
            }
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Video) {
                let (code, msg) = format_source_buffer_creation_err_for_js(e);
                jsSendSourceBufferCreationError(code, Some(&msg));
                self.internal_stop();
                return;
            }
            jsStartObservingPlayback();
        }
    }

    /// Method called as a MultiVariant Playlist is loaded
    pub(super) fn on_multivariant_playlist_loaded(&mut self, data: Vec<u8>, playlist_url: Url) {
        match MultiVariantPlaylist::parse(data.as_ref(), playlist_url) {
            Err(e) => {
                jsSendPlaylistParsingError(
                    true,
                    PlaylistType::MultiVariantPlaylist,
                    None,
                    Some(&e.to_string()),
                );
                self.internal_stop();
            }
            Ok(pl) => {
                Logger::info("MultiVariant Playlist parsed successfully");
                self.playlist_store = Some(PlaylistStore::new(pl));
                let playlist_store = self.playlist_store.as_mut().unwrap();
                if playlist_store.variants().is_empty() {
                    jsSendPlaylistParsingError(
                        true,
                        PlaylistType::MultiVariantPlaylist,
                        None,
                        Some("Error while parsing MultiVariantPlaylist: no variant found."),
                    );
                    self.internal_stop();
                    return;
                }

                use PlaylistFileType::*;
                // TODO lowest/latest bandwidth first?
                playlist_store.update_curr_bandwidth(2_000_000.);
                [MediaType::Video, MediaType::Audio]
                    .into_iter()
                    .for_each(|mt| {
                        if let Some(id) = playlist_store.curr_media_playlist_id(mt) {
                            if let Some(url) = playlist_store.media_playlist_url(id) {
                                let id = id.clone();
                                let url = url.clone();
                                self.requester.fetch_playlist(url, MediaPlaylist { id });
                            }
                        }
                    });

                // SAFETY: The following lines are unsafe because they may actually define raw pointers
                // to point to Rust's heap memory and put it in the returned values.
                //
                // However, we're calling the JS binding function it is communicated to directly
                // after and thus before the corresponding underlying data had a chance to be
                // dropped.
                //
                // Because one of the rules of those bindings is to copy all pointed data
                // synchronously on call, we should not encounter any issue.
                let variants_info =
                    unsafe { format_variants_info_for_js(playlist_store.variants()) };
                let audio_tracks_info =
                    unsafe { format_audio_tracks_for_js(playlist_store.audio_tracks()) };
                let selected_audio_track = playlist_store.selected_audio_track_id();
                let is_selected = selected_audio_track.is_some();
                let curr_audio_track = if let Some(selected) = selected_audio_track {
                    Some(selected)
                } else {
                    playlist_store.curr_audio_track_id()
                };
                jsAnnounceFetchedContent(variants_info, audio_tracks_info);
                jsAnnounceVariantUpdate(playlist_store.curr_variant().map(|v| v.id()));
                jsAnnounceTrackUpdate(MediaType::Audio, curr_audio_track, is_selected);
            }
        }
    }

    /// Method called as a MediaPlaylist Playlist is loaded
    pub(super) fn on_media_playlist_loaded(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        data: Vec<u8>,
        playlist_url: Url,
    ) {
        Logger::info(&format!(
            "Media playlist loaded successfully: {}",
            playlist_url.get_ref()
        ));
        if let Some(ref mut playlist_store) = self.playlist_store.as_mut() {
            match playlist_store.update_media_playlist(&playlist_id, data.as_ref(), playlist_url) {
                Err(e) => {
                    jsSendPlaylistParsingError(
                        true,
                        PlaylistType::MediaPlaylist,
                        None, // TODO? Or maybe at least the URL
                        Some(&e.to_string()),
                    );
                    self.internal_stop();
                }
                Ok(p) => {
                    if let Some(refresh_interval) = p.refresh_interval() {
                        let timer_id = jsTimer(refresh_interval, TimerReason::MediaPlaylistRefresh);
                        self.playlist_refresh_timers.push((
                            timer_id,
                            PlaylistFileType::MediaPlaylist { id: playlist_id },
                        ));
                    }
                    match self.ready_state.cmp(&PlayerReadyState::Loading) {
                        Ordering::Greater => self.check_segments_to_request(),
                        Ordering::Equal => self.check_ready_to_load_segments(),
                        _ => {}
                    };

                    if let Some(playlist_store) = self.playlist_store.as_ref() {
                        let min_pos = playlist_store.curr_min_position();
                        let max_pos = playlist_store.curr_max_position();
                        jsUpdateContentInfo(min_pos, max_pos);
                    }
                }
            }
        } else {
            jsSendOtherError(
                true,
                crate::bindings::OtherErrorCode::Unknown,
                Some("Media playlist loaded but no MultiVariantPlaylist"),
            );
            self.internal_stop();
        }
    }

    fn init_source_buffer(
        &mut self,
        media_type: MediaType,
    ) -> Option<Result<(), SourceBufferCreationError>> {
        let content = self.playlist_store.as_mut()?;

        let media_playlist = content.curr_media_playlist(media_type)?;
        let mime_type = media_playlist.mime_type(media_type).unwrap_or("");

        // TODO to_string should be unneeded here as &str is sufficient
        let codecs = content
            .curr_variant()?
            .codecs(media_type)
            .unwrap_or_default();
        Some(
            self.media_element_ref
                .create_source_buffer(media_type, mime_type, &codecs),
        )
    }

    pub(crate) fn internal_on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        Logger::info(&format!("MediaSource state changed: {:?}", state));
        self.media_element_ref
            .update_media_source_ready_state(state);
        if state == MediaSourceReadyState::Open {
            self.check_ready_to_load_segments();
        }
    }

    pub(crate) fn internal_on_source_buffer_update(&mut self, source_buffer_id: SourceBufferId) {
        self.media_element_ref
            .on_source_buffer_update(source_buffer_id);
    }

    pub(crate) fn internal_on_source_buffer_error(&mut self, _source_buffer_id: SourceBufferId) {
        // TODO check QuotaExceededError and so on...
        // TODO better error
        jsSendOtherError(
            true,
            crate::bindings::OtherErrorCode::Unknown,
            Some("A SourceBuffer emitted an error"),
        );
        self.internal_stop();
    }

    pub fn on_observation(&mut self, observation: MediaObservation) {
        let reason = observation.reason();
        Logger::debug(&format!(
            "Tick received: {:?} {}",
            reason,
            observation.current_time()
        ));
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
        self.segment_selectors
            .update_base_position(wanted_pos - 0.5);
        self.check_segments_to_request();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    pub fn on_seek(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.segment_selectors.reset_position(wanted_pos - 0.5);

        // TODO better logic than aborting everything on seek?
        self.requester.abort_all_segments();
        self.requester.update_base_position(Some(wanted_pos));
        self.check_segments_to_request()
    }

    pub fn check_segments_to_request(&mut self) {
        match self.playlist_store.as_ref() {
            None => {}
            Some(pl) => {
                let was_already_locked = self.requester.lock_segment_requests();
                [MediaType::Video, MediaType::Audio].iter().for_each(|mt| {
                    let mt = *mt;
                    if !self.requester.has_segment_request_pending(mt) {
                        if let Some(pl) = pl.curr_media_playlist(mt) {
                            match self.segment_selectors.get_mut(mt).get_next_segment_info(pl) {
                                NextSegmentInfo::None => {}
                                NextSegmentInfo::InitSegment(i) => self
                                    .requester
                                    .request_init_segment(mt, i.uri.clone(), i.byte_range.as_ref()),
                                NextSegmentInfo::MediaSegment(seg) => {
                                    self.requester.request_media_segment(mt, seg)
                                }
                            }
                        }
                    }
                });
                if !was_already_locked {
                    self.requester.unlock_segment_requests();
                }
            }
        }
    }

    pub fn internal_stop(&mut self) {
        Logger::info("Stopping current content (if one) and resetting player");
        self.requester.reset();
        jsStopObservingPlayback();
        self.media_element_ref.reset();
        self.segment_selectors.reset_position(0.);
        self.playlist_store = None;
        self.last_position = 0.;
        self.playlist_refresh_timers.clear();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Method called once a playlist request ended with success
    pub(super) fn on_playlist_fetch_success(
        &mut self,
        pl_info: PlaylistRequestInfo,
        result: Vec<u8>,
        final_url: Url,
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        if let PlaylistFileType::MediaPlaylist { id, .. } = playlist_type {
            self.on_media_playlist_loaded(id, result, final_url);
        } else {
            self.on_multivariant_playlist_loaded(result, final_url);
        }
    }

    fn push_and_validate_segment(
        &mut self,
        data: JsMemoryBlob,
        media_type: MediaType,
        time_info: Option<(f64, f64)>,
    ) {
        let md = PushMetadata::new(data, time_info);
        match self.media_element_ref.push_segment(media_type, md) {
            Err(x) => {
                jsSendOtherError(
                    true,
                    crate::bindings::OtherErrorCode::Unknown,
                    Some(&format!("Can't push {} segment: {:?}", media_type, x)),
                );
                self.internal_stop();
            }
            Ok(()) => {
                if let Some(ti) = time_info {
                    self.segment_selectors
                        .get_mut(media_type)
                        .validate_media(ti.0);
                    if was_last_segment(self.playlist_store.as_ref(), media_type, ti.0) {
                        Logger::info(&format!(
                            "Last {} segment request finished, declaring its buffer's end",
                            media_type
                        ));
                        self.media_element_ref.end_buffer(media_type);
                    }
                } else {
                    self.segment_selectors.get_mut(media_type).validate_init();
                }
            }
        }
    }

    fn process_request_metrics(&mut self, resource_size: u32, duration_ms: f64) {
        self.adaptive_selector
            .add_metric(duration_ms, resource_size);
        if let Some(pl) = self.playlist_store.as_mut() {
            if let Some(bandwidth) = self.adaptive_selector.get_estimate() {
                Logger::debug(&format!("New bandwidth estimate: {}", bandwidth));
                let update = pl.update_curr_bandwidth(bandwidth);
                self.handle_variant_update(update, false);
            }
        }
    }

    pub(super) fn inner_lock_variant(&mut self, variant_id: String) {
        if let Some(pl) = self.playlist_store.as_mut() {
            match pl.lock_variant(&variant_id) {
                None => Logger::warn("Locked variant not found"),
                Some(update) => self.handle_variant_update(update, true),
            }
        }
    }

    pub(super) fn inner_unlock_variant(&mut self) {
        if let Some(pl) = self.playlist_store.as_mut() {
            let update = pl.unlock_variant();
            self.handle_variant_update(update, false);
        }
    }

    fn handle_variant_update(&mut self, result: VariantUpdateResult, force_urgent: bool) {
        let (changed_media_types, has_improved) = match result {
            VariantUpdateResult::Improved(mt) => (mt, true),
            VariantUpdateResult::EqualOrUnknown(mt) => (mt, false),
            VariantUpdateResult::Worsened(mt) => (mt, false),
            VariantUpdateResult::Unchanged => {
                return;
            }
        };
        self.on_media_playlist_changed(&changed_media_types, has_improved || force_urgent);
        if let Some(pl) = self.playlist_store.as_mut() {
            jsAnnounceVariantUpdate(pl.curr_variant().map(|v| v.id()));
        }
    }

    fn on_media_playlist_changed(&mut self, changed_media_types: &[MediaType], abort_prev: bool) {
        if let Some(pl) = self.playlist_store.as_mut() {
            changed_media_types.iter().for_each(|mt| {
                let mt = *mt;
                Logger::info(&format!("{} MediaPlaylist changed", mt));
                if abort_prev {
                    self.requester.abort_segments_with_type(mt);
                    self.segment_selectors
                        .reset_position_for_type(mt, self.last_position - 0.5);
                }
                self.requester.abort_segments_with_type(mt);
                let selector = self.segment_selectors.get_mut(mt);
                selector.rollback();
                selector.reset_init_segment();
                if pl.curr_media_playlist(mt).is_none() {
                    if let Some(id) = pl.curr_media_playlist_id(mt) {
                        if let Some(url) = pl.media_playlist_url(id) {
                            use PlaylistFileType::*;
                            Logger::debug("Media changed, requesting its media playlist");
                            let id = id.clone();
                            let url = url.clone();
                            self.requester.fetch_playlist(url, MediaPlaylist { id });
                        }
                    }
                }
            });
            self.check_segments_to_request();
        }
    }

    /// Method called once a segment request ended with success
    pub(super) fn on_segment_fetch_success(
        &mut self,
        segment_req: SegmentRequestInfo,
        result: JsMemoryBlob,
        resource_size: u32,
        duration_ms: f64,
    ) {
        Logger::lazy_info(&|| {
            let media_type = segment_req.media_type;
            match segment_req.time_info {
                None => format!("Loaded {} init segment", media_type),
                Some((start, end)) => format!(
                    "Loaded {} segment: t: {}, d: {}",
                    media_type,
                    start,
                    end - start
                ),
            }
        });

        self.push_and_validate_segment(result, segment_req.media_type, segment_req.time_info);
        self.process_request_metrics(resource_size, duration_ms);
        self.check_segments_to_request();
    }

    pub fn on_playlist_refresh_timer_ended(&mut self, id: TimerId) {
        let found = self.playlist_refresh_timers.iter().position(|x| x.0 == id);
        if let Some(idx) = found {
            let (_, playlist_type) = self.playlist_refresh_timers.remove(idx);
            if let Some(playlist_store) = &self.playlist_store {
                match playlist_type {
                    PlaylistFileType::MultiVariantPlaylist => self
                        .requester
                        .fetch_playlist(playlist_store.url().clone(), playlist_type),
                    PlaylistFileType::MediaPlaylist { ref id } => {
                        if let Some(u) = playlist_store.media_playlist_url(id) {
                            self.requester.fetch_playlist(u.clone(), playlist_type)
                        } else {
                            Logger::error("Cannot refresh Media Playlist: id not found");
                        }
                    }
                    PlaylistFileType::Unknown => {
                        Logger::error("Cannot refresh Media Playlist: type unknown")
                    }
                }
            }
        }
    }

    pub fn on_retry_request(&mut self, id: TimerId) {
        self.requester.on_timer_finished(id);
    }

    pub fn inner_set_audio_track(&mut self, track_id: Option<String>) {
        if let Some(ref mut pl) = self.playlist_store {
            if pl.set_audio_track(track_id) {
                if let Err(e) = self.media_element_ref.flush(MediaType::Audio) {
                    Logger::warn(&format!(
                        "Could not remove data from the previous audio track: {}",
                        e
                    ));
                }
                self.on_media_playlist_changed(&[MediaType::Audio], true);
            }
        }
    }
}

fn was_last_segment(
    playlist_store: Option<&PlaylistStore>,
    media_type: MediaType,
    seg_start: f64,
) -> bool {
    playlist_store
        .and_then(|c| c.curr_media_playlist(media_type))
        .map(|pl| {
            pl.is_ended()
                && pl
                    .segment_list()
                    .last()
                    .map(|x| x.start == seg_start)
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}
