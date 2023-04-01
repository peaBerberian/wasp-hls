use std::cmp::Ordering;

use super::{
    event_listeners::JsTimeRanges, Dispatcher, JsMemoryBlob, MediaObservation,
    MediaSourceReadyState, PlaybackTickReason, PlayerReadyState,
};
use crate::{
    bindings::{
        formatters::{
            format_audio_tracks_for_js, format_source_buffer_creation_err_for_js,
            format_variants_info_for_js,
        },
        jsAnnounceFetchedContent, jsAnnounceTrackUpdate, jsAnnounceVariantLockStatusChange,
        jsAnnounceVariantUpdate, jsClearTimer, jsSendMediaPlaylistParsingError,
        jsSendMediaPlaylistRequestError, jsSendMultivariantPlaylistParsingError,
        jsSendMultivariantPlaylistRequestError, jsSendOtherError, jsSendSegmentRequestError,
        jsSendSourceBufferCreationError, jsSetMediaSourceDuration, jsStartObservingPlayback,
        jsStopObservingPlayback, jsTimer, jsUpdateContentInfo, MediaType,
        MultivariantPlaylistParsingErrorCode, OtherErrorCode, RequestId, SourceBufferId, TimerId,
        TimerReason, jsSendSegmentParsingError,
    },
    media_element::{SegmentPushData, SegmentQualityContext, SourceBufferCreationError},
    parser::{MultivariantPlaylist, SegmentTimeInfo},
    playlist_store::{
        LockVariantResponse, MediaPlaylistPermanentId, PlaylistStore, PlaylistStoreError,
        SetAudioTrackResponse, VariantUpdateResult,
    },
    requester::{
        FinishedRequestType, PlaylistFileType, PlaylistRequestInfo, RetryResult, SegmentRequestInfo,
    },
    segment_selector::NextSegmentInfo,
    utils::url::Url,
    Logger,
};

impl Dispatcher {
    /// Completely stop playback of the current content if one and free all its associated
    /// resources.
    pub(super) fn stop_current_content(&mut self) {
        Logger::info("Core: Stopping current content (if one) and resetting player");
        self.requester.reset();
        jsStopObservingPlayback();
        self.media_element_ref.reset();
        self.segment_selectors.reset(0.);
        self.playlist_store = None;
        self.last_position = 0.;
        self.clean_up_playlist_refresh_timers();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Check which is the best HLS variant to select according to the current conditions
    /// If it changed, handle the consequences (such as requesting new media playlists, loading
    /// and pushing segments etc.).
    pub(super) fn check_best_variant(&mut self) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            if let Some(bandwidth) = self.adaptive_selector.get_estimate() {
                Logger::debug(&format!("Core: New bandwidth estimate: {}", bandwidth));
                let actually_used_bandwidth = bandwidth / self.media_element_ref.wanted_speed();
                let update = pl_store.update_curr_bandwidth(actually_used_bandwidth);
                self.handle_variant_update(update, false);
            }
        }
    }

    /// Begin "locking" HLS variant whose `id` is given in argument, meaning that we will keep only
    /// playing that one.
    pub(super) fn lock_variant_core(&mut self, variant_id: u32) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            let is_audio_track_selected = pl_store.curr_audio_track_id().is_some();
            match pl_store.lock_variant(variant_id) {
                LockVariantResponse::NoVariantWithId => {
                    Logger::warn("Core: Locked variant not found");
                    jsSendOtherError(
                        false,
                        crate::bindings::OtherErrorCode::UnfoundLockedVariant,
                        Some(&format!("Wanted locked variant \"{variant_id}\" not found")),
                    );
                }
                LockVariantResponse::VariantLocked {
                    updates,
                    audio_track_change,
                } => {
                    if let Some(track_id) = audio_track_change {
                        jsAnnounceTrackUpdate(
                            MediaType::Audio,
                            Some(track_id),
                            is_audio_track_selected,
                        );
                    }
                    self.handle_variant_update(updates, true);
                    jsAnnounceVariantLockStatusChange(Some(variant_id));
                }
            }
        }
    }

    /// Remove an HLS variant previously put in place through `lock_variant_core`.
    pub(super) fn unlock_variant_core(&mut self) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            let update = pl_store.unlock_variant();
            self.handle_variant_update(update, false);
        }
    }

    /// Method to call once a timer for Playlist refresh, started with the jsTimer JavaScript
    /// function, has finished, with the corrsonding `TimerId` as argument.
    pub(super) fn on_playlist_refresh_timer_ended(&mut self, id: TimerId) {
        let found = self.playlist_refresh_timers.iter().position(|x| x.0 == id);
        if let Some(idx) = found {
            let (_, playlist_type) = self.playlist_refresh_timers.remove(idx);
            if let Some(playlist_store) = &self.playlist_store {
                match playlist_type {
                    PlaylistFileType::MultivariantPlaylist => self
                        .requester
                        .fetch_playlist(playlist_store.url().clone(), playlist_type),
                    PlaylistFileType::MediaPlaylist { ref id, .. } => {
                        if let Some(u) = playlist_store.media_playlist_url(id) {
                            self.requester.fetch_playlist(u.clone(), playlist_type)
                        } else {
                            Logger::error("Core: Cannot refresh Media Playlist: id not found");
                        }
                    }
                }
            }
        }
    }

    /// Method to call once a timer for retrying a request, started with the jsTimer JavaScript
    /// function, has finished, with the corrsonding `TimerId` as argument.
    pub(super) fn on_retry_request(&mut self, id: TimerId) {
        self.requester.on_timer_finished(id);
    }

    /// Set an audio track whose `id` is given in argument.
    pub(super) fn set_audio_track_core(&mut self, track_id: Option<u32>) {
        if let Some(ref mut pl_store) = self.playlist_store {
            match pl_store.set_audio_track(track_id) {
                SetAudioTrackResponse::AudioMediaUpdate => {
                    self.handle_media_playlist_update(&[MediaType::Audio], true, true)
                }
                SetAudioTrackResponse::VariantUpdate {
                    updates,
                    unlocked_variant,
                } => {
                    self.handle_variant_update(updates, true);
                    if unlocked_variant {
                        jsAnnounceVariantLockStatusChange(None);
                    }
                }
                _ => {}
            }
        }
    }

    /// Method to call once a request started with `jsFetch` finished with success
    pub(super) fn on_request_succeeded(
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
            None => Logger::warn("Core: Unknown request finished"),
        }
    }

    /// Method to call once a request started with `jsFetch` finished with a failure.
    pub(super) fn on_request_failed_core(
        &mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>,
    ) {
        match self
            .requester
            .on_pending_request_failure(request_id, has_timeouted, status)
        {
            RetryResult::Failed {
                request_type: FinishedRequestType::Segment(s),
                reason,
                status,
            } => {
                let time_info = s.time_info();
                jsSendSegmentRequestError(
                    true,
                    s.url().get_ref(),
                    time_info.is_none(),
                    time_info.map(|t| vec![t.start(), t.end()]),
                    s.media_type(),
                    reason,
                    status,
                );
                self.stop_current_content();
            }
            RetryResult::Failed {
                request_type: FinishedRequestType::Playlist(x),
                reason,
                status,
                ..
            } => {
                match x.playlist_type {
                    PlaylistFileType::MediaPlaylist { media_type, .. } => {
                        jsSendMediaPlaylistRequestError(
                            true,
                            x.url.get_ref(),
                            reason,
                            media_type,
                            status,
                        );
                    }
                    PlaylistFileType::MultivariantPlaylist => {
                        jsSendMultivariantPlaylistRequestError(
                            true,
                            x.url.get_ref(),
                            reason,
                            status,
                        );
                    }
                }
                self.stop_current_content();
            }

            RetryResult::RetriedSegment {
                request_info,
                reason,
                status,
            } => {
                jsSendSegmentRequestError(
                    false,
                    request_info.url().get_ref(),
                    request_info.time_info().is_none(),
                    request_info.time_info().map(|t| vec![t.start(), t.end()]),
                    request_info.media_type(),
                    reason,
                    status,
                );
            }

            RetryResult::RetriedPlaylist {
                request_info,
                reason,
                status,
            } => match request_info.playlist_type {
                PlaylistFileType::MultivariantPlaylist => jsSendMultivariantPlaylistRequestError(
                    false,
                    request_info.url.get_ref(),
                    reason,
                    status,
                ),
                PlaylistFileType::MediaPlaylist { media_type, .. } => {
                    jsSendMediaPlaylistRequestError(
                        false,
                        request_info.url.get_ref(),
                        reason,
                        media_type,
                        status,
                    )
                }
            },

            RetryResult::NotFound => {
                Logger::warn("Core: Request failed not found on the current Requester")
            }
        }
    }

    /// Method to call when the `readyState` JS attribute of the linked `MediaSource` object
    /// changed, with that new state in argument.
    pub(super) fn on_media_source_state_change_core(&mut self, state: MediaSourceReadyState) {
        Logger::info(&format!("Core: MediaSource state changed: {:?}", state));
        self.media_element_ref
            .update_media_source_ready_state(state);
        if state == MediaSourceReadyState::Open {
            self.check_ready_to_load_segments();
        }
    }

    /// Method to call when a SourceBuffer triggered an `updateend` event.
    pub(super) fn on_source_buffer_update_core(
        &mut self,
        source_buffer_id: SourceBufferId,
        buffered: JsTimeRanges,
    ) {
        self.media_element_ref
            .on_source_buffer_update(source_buffer_id, buffered);
    }

    /// Method to call when a SourceBuffer triggered an `error` event.
    pub(super) fn on_source_buffer_error_core(&mut self, _source_buffer_id: SourceBufferId) {
        // TODO check QuotaExceededError and so on...
        // TODO better error
        jsSendOtherError(
            true,
            crate::bindings::OtherErrorCode::Unknown,
            Some("A SourceBuffer emitted an error"),
        );
        self.stop_current_content();
    }

    /// Method to call when a new `MediaObservation` has been received.
    pub(super) fn on_observation(&mut self, observation: MediaObservation) {
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

    /// Method to call when a new codec support report has been received.
    pub(super) fn on_codecs_support_update_core(&mut self) {
        self.check_ready_to_load_media_playlists();
    }

    /// For each media type, check if segment need to be requested, and if that's the case, perform
    /// the request.
    ///
    /// This method is intelligent enough to not do new requests if some are already pending for
    /// the same type, meaning that you can call it any time you may want to check if segments can
    /// be requested (when a request finished, when a media playlist has been updated, when the
    /// playhead advances etc.).
    pub(super) fn check_segments_to_request(&mut self) {
        let was_already_locked = self.requester.lock_segment_requests();
        [MediaType::Video, MediaType::Audio]
            .into_iter()
            .for_each(|mt| {
                self.check_segment_to_request_for_type(mt);
            });
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    /// Method called on various events that could lead to start loading segments.
    ///
    /// If all conditions are met, the `Dispatcher` is set to the next
    /// `AwaitingSegments` `PlayerReadyState`, playback observations begin,
    /// and the potential initialization segments are requested.
    fn check_ready_to_load_segments(&mut self) {
        if self.ready_state >= PlayerReadyState::AwaitingSegments {
            return;
        }

        match self.media_element_ref.media_source_ready_state() {
            Some(MediaSourceReadyState::Closed) | None => {
                return;
            }
            _ => {}
        }

        let playlist_store = if let Some(pl_store) = self.playlist_store.as_ref() {
            pl_store
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
                Logger::warn("Core: Unknown content duration");
            }

            if let Some(Err(e)) = self.init_source_buffer(MediaType::Audio) {
                let (code, msg) = format_source_buffer_creation_err_for_js(e);
                jsSendSourceBufferCreationError(true, code, Some(&msg));
                self.stop_current_content();
                return;
            }
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Video) {
                let (code, msg) = format_source_buffer_creation_err_for_js(e);
                jsSendSourceBufferCreationError(true, code, Some(&msg));
                self.stop_current_content();
                return;
            }
            jsStartObservingPlayback();
        }
    }

    /// Method called once a playlist request ended with success
    fn on_playlist_fetch_success(
        &mut self,
        pl_info: PlaylistRequestInfo,
        result: Vec<u8>,
        final_url: Url,
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        if let PlaylistFileType::MediaPlaylist { id, media_type } = playlist_type {
            self.on_media_playlist_loaded(id, result, media_type, final_url);
        } else {
            self.on_multivariant_playlist_loaded(result, final_url);
        }
    }

    /// Method called once a Multivariant Playlist was loaded with success, with its response data
    /// and url as argument.
    fn on_multivariant_playlist_loaded(&mut self, data: Vec<u8>, playlist_url: Url) {
        match MultivariantPlaylist::parse(data.as_ref(), playlist_url) {
            Err(e) => {
                let message = e.to_string();
                jsSendMultivariantPlaylistParsingError(true, e.into(), Some(&message));
                self.stop_current_content();
            }
            Ok(pl) => {
                Logger::info("Core: Multivariant Playlist parsed successfully");
                // TODO lowest/latest bandwidth first? Option?
                match PlaylistStore::try_new(pl, 500_000.) {
                    Ok(pl_store) => {
                        self.playlist_store = Some(pl_store);
                        self.check_ready_to_load_media_playlists();
                    }
                    Err(err) => {
                        match err {
                            PlaylistStoreError::NoSupportedVariant => jsSendOtherError(
                                true,
                                OtherErrorCode::NoSupportedVariant,
                                Some(&err.to_string()),
                            ),
                            PlaylistStoreError::NoInitialVariant => jsSendMultivariantPlaylistParsingError(
                                true,
                                MultivariantPlaylistParsingErrorCode::MultivariantPlaylistWithoutVariant,
                                Some(&err.to_string()),
                            ),
                        };
                        self.stop_current_content();
                    }
                }
            }
        }
    }

    /// Method called once a Media Playlist was loaded with success, with its id, response data
    /// and url as argument.
    fn on_media_playlist_loaded(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        data: Vec<u8>,
        media_type: MediaType,
        playlist_url: Url,
    ) {
        Logger::info(&format!(
            "Media playlist loaded successfully: {}",
            playlist_url.get_ref()
        ));
        if let Some(ref mut playlist_store) = self.playlist_store.as_mut() {
            match playlist_store.update_media_playlist(&playlist_id, data.as_ref(), playlist_url) {
                Err(e) => {
                    let err_message = e.to_string();
                    jsSendMediaPlaylistParsingError(true, e.into(), media_type, Some(&err_message));
                    self.stop_current_content();
                }
                Ok(p) => {
                    if let Some(refresh_interval) = p.refresh_interval() {
                        let timer_id = jsTimer(refresh_interval, TimerReason::MediaPlaylistRefresh);
                        self.playlist_refresh_timers.push((
                            timer_id,
                            PlaylistFileType::MediaPlaylist {
                                id: playlist_id,
                                media_type,
                            },
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
                Some("Media playlist loaded but no MultivariantPlaylist"),
            );
            self.stop_current_content();
        }
    }

    fn check_ready_to_load_media_playlists(&mut self) {
        let playlist_store = if let Some(playlist_store) = self.playlist_store.as_mut() {
            playlist_store
        } else {
            // No PlaylistStore == no loaded Multivariant Playlist yet
            return;
        };

        match playlist_store.check_codecs() {
            Ok(false) => {
                // Awaiting query about codecs support.
                return;
            }
            Err(err) => {
                match err {
                    PlaylistStoreError::NoSupportedVariant => jsSendOtherError(
                        true,
                        OtherErrorCode::NoSupportedVariant,
                        Some(&err.to_string()),
                    ),
                    PlaylistStoreError::NoInitialVariant => jsSendMultivariantPlaylistParsingError(
                        true,
                        MultivariantPlaylistParsingErrorCode::MultivariantPlaylistWithoutVariant,
                        Some(&err.to_string()),
                    ),
                };
                self.stop_current_content();
                return;
            }
            _ => {}
        }

        if playlist_store.supported_variants().is_empty() {
            jsSendOtherError(
                true,
                crate::bindings::OtherErrorCode::NoSupportedVariant,
                Some("Error while parsing MultivariantPlaylist: no compatible variant found."),
            );
            self.stop_current_content();
            return;
        }

        use PlaylistFileType::*;
        [MediaType::Video, MediaType::Audio]
            .into_iter()
            .for_each(|mt| {
                if let Some(id) = playlist_store.curr_media_playlist_id(mt) {
                    if let Some(url) = playlist_store.media_playlist_url(id) {
                        let id = id.clone();
                        let url = url.clone();
                        self.requester
                            .fetch_playlist(url, MediaPlaylist { id, media_type: mt });
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
            unsafe { format_variants_info_for_js(playlist_store.supported_variants().as_slice()) };
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

    fn init_source_buffer(
        &mut self,
        media_type: MediaType,
    ) -> Option<Result<(), SourceBufferCreationError>> {
        let content = self.playlist_store.as_mut()?;
        let media_playlist = content.curr_media_playlist(media_type)?;
        let mime_type = media_playlist.mime_type(media_type).unwrap_or("");
        let codecs = content
            .curr_variant()?
            .codecs(media_type)
            .unwrap_or_default();
        Some(
            self.media_element_ref
                .create_source_buffer(media_type, mime_type, &codecs),
        )
    }

    fn on_regular_tick(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.last_position = wanted_pos;

        // Lock `Requester`, so it only do new segment requests when every
        // wanted segments is scheduled - for better priorization
        let was_already_locked = self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.segment_selectors
            .update_base_position(wanted_pos - 0.2);

        self.check_segments_to_request();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }

        if self.media_element_ref.is_rebuffering() {
            match self.next_scheduled_segment_start() {
                None => {}
                Some(val) => {
                    let buffer_gap = self.media_element_ref.last_buffer_gap();
                    if wanted_pos + buffer_gap < val {
                        Logger::warn(&format!(
                            "Core: Found a skippable discontinuity (p:{}, bg:{}, n:{})",
                            wanted_pos, buffer_gap, val
                        ));
                        self.media_element_ref.seek(val + 0.01);
                    }
                }
            };
        }
    }

    fn next_scheduled_segment_start(&self) -> Option<f64> {
        let wanted_pos = self.media_element_ref.wanted_position();
        let req_min = self.requester.earliest_media_segment_pending();
        let mut seg_min = None;
        [MediaType::Video, MediaType::Audio]
            .into_iter()
            .for_each(|mt| {
                let inventory = self.media_element_ref.inventory(mt);
                let next_segment = inventory
                    .iter()
                    .find(|s| s.last_buffered_end() > wanted_pos);
                if let Some(seg) = next_segment {
                    seg_min = Some(
                        seg_min
                            .map(|m| f64::max(m, seg.last_buffered_start()))
                            .unwrap_or(seg.last_buffered_start()),
                    );
                }
            });
        match (req_min, seg_min) {
            (None, None) => None,
            (Some(rm), None) => Some(rm),
            (None, Some(sm)) => Some(sm),
            (Some(rm), Some(sm)) => Some(f64::min(rm, sm)),
        }
    }

    /// Actions to perform once a seek has been performed on the media element.
    fn on_seek(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.segment_selectors.restart_from(wanted_pos - 0.2);

        // TODO better logic than aborting everything on seek
        self.requester.abort_all_segments();
        self.requester.update_base_position(Some(wanted_pos));
        self.check_segments_to_request()
    }

    fn check_segment_to_request_for_type(&mut self, media_type: MediaType) {
        let pl_store = if let Some(playlist_store) = self.playlist_store.as_ref() {
            playlist_store
        } else {
            return;
        };
        if !self.requester.has_segment_request_pending(media_type) {
            let inventory = self.media_element_ref.inventory(media_type);
            if let Some(seg_info) = pl_store.curr_media_playlist_segment_info(media_type) {
                match self
                    .segment_selectors
                    .get_mut(media_type)
                    .most_needed_segment(seg_info.0, &seg_info.1, inventory)
                {
                    NextSegmentInfo::None => {}
                    NextSegmentInfo::InitSegment(i) => self.requester.request_init_segment(
                        media_type,
                        i.uri().clone(),
                        i.byte_range(),
                        seg_info.1,
                    ),
                    NextSegmentInfo::MediaSegment(seg) => self
                        .requester
                        .request_media_segment(media_type, seg, seg_info.1),
                }
            }
        }
    }

    fn push_and_validate_segment(
        &mut self,
        data: JsMemoryBlob,
        media_type: MediaType,
        time_info: Option<SegmentTimeInfo>,
        context: SegmentQualityContext,
    ) {
        let segment_time = time_info.as_ref().map(|t| (t.start(), t.end()));
        let md = SegmentPushData::new(data, time_info);
        match self.media_element_ref.push_segment(media_type, md, context) {
            Err(x) => {
                let media_type = x.media_type();
                let message = x.to_string();
                jsSendSegmentParsingError(
                    true,
                    x.into(),
                    media_type,
                    Some(&message),
                );
                self.stop_current_content();
            }
            Ok(()) => {
                if let Some((segment_start, segment_end)) = segment_time {
                    self.segment_selectors
                        .get_mut(media_type)
                        .validate_media_until(segment_end);
                    if was_last_segment(self.playlist_store.as_ref(), media_type, segment_start) {
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

    /// Perform all actions that should be commonly taken after the current variant changes.
    fn handle_variant_update(&mut self, result: VariantUpdateResult, flush: bool) {
        let (changed_media_types, has_worsened) = match result {
            VariantUpdateResult::Improved(mt) => (mt, false),
            VariantUpdateResult::EqualOrUnknown(mt) => (mt, false),
            VariantUpdateResult::Worsened(mt) => (mt, true),
            VariantUpdateResult::Unchanged => {
                return;
            }
        };
        self.handle_media_playlist_update(&changed_media_types, flush || has_worsened, flush);
        if let Some(pl_store) = self.playlist_store.as_mut() {
            jsAnnounceVariantUpdate(pl_store.curr_variant().map(|v| v.id()));
        }
    }

    /// Perform all actions that should be commonly taken after one or multiple of the current Media
    /// Playlists change.
    fn handle_media_playlist_update(
        &mut self,
        changed_media_types: &[MediaType],
        abort_prev: bool,
        flush: bool,
    ) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            changed_media_types.iter().for_each(|mt| {
                let mt = *mt;
                Logger::info(&format!("Core: {} MediaPlaylist changed", mt));

                let selector = self.segment_selectors.get_mut(mt);
                if abort_prev {
                    self.requester.abort_segments_with_type(mt);
                }
                if flush {
                    if let Err(e) = self.media_element_ref.flush(mt) {
                        Logger::warn(&format!(
                            "Could not remove data from the previous {mt} buffer: {}",
                            e
                        ));
                    }
                    selector.restart_from(self.media_element_ref.wanted_position() - 0.2);
                }
                if pl_store.curr_media_playlist(mt).is_none() {
                    if let Some(id) = pl_store.curr_media_playlist_id(mt) {
                        if let Some(url) = pl_store.media_playlist_url(id) {
                            use PlaylistFileType::*;
                            Logger::debug("Core: Media changed, requesting its media playlist");
                            let id = id.clone();
                            let url = url.clone();
                            self.requester
                                .fetch_playlist(url, MediaPlaylist { id, media_type: mt });
                        }
                    }
                }
            });
            if !changed_media_types.is_empty() {
                self.clean_up_playlist_refresh_timers();
            }
            self.check_segments_to_request();
        }
    }

    /// Method called once a segment request ended with success
    fn on_segment_fetch_success(
        &mut self,
        segment_req: SegmentRequestInfo,
        result: JsMemoryBlob,
        resource_size: u32,
        duration_ms: f64,
    ) {
        Logger::lazy_info(&|| {
            let media_type = segment_req.media_type();
            match segment_req.time_info() {
                None => format!("Loaded {} init segment", media_type),
                Some(time_info) => format!(
                    "Loaded {} segment: t: {}, d: {}",
                    media_type,
                    time_info.start(),
                    time_info.duration()
                ),
            }
        });

        let media_type = segment_req.media_type();
        let (_, _, time_info, context) = segment_req.deconstruct();
        self.push_and_validate_segment(result, media_type, time_info, context);
        self.adaptive_selector
            .add_metric(duration_ms, resource_size);
        self.check_best_variant();
        self.check_segments_to_request();
    }

    /// Removes from `self.playlist_refresh_timers` timers for playlist that are not current
    /// anymore and abort their corresponding timers
    fn clean_up_playlist_refresh_timers(&mut self) {
        if let Some(ref pl_store) = self.playlist_store {
            self.playlist_refresh_timers.retain(|x| {
                if let PlaylistFileType::MediaPlaylist { id, .. } = &x.1 {
                    if !pl_store.is_curr_media_playlist(id) {
                        jsClearTimer(x.0);
                        return false;
                    }
                }
                true
            });
        } else {
            while let Some(timer_info) = self.playlist_refresh_timers.pop() {
                jsClearTimer(timer_info.0);
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
                    .media()
                    .last()
                    .map(|x| x.start() == seg_start)
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}
