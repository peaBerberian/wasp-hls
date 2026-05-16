use super::{
    event_listeners::JsTimeRanges, utils, Dispatcher, JsMemoryBlob, MediaObservation,
    MediaSourceReadyState, PlaybackTickReason, PlayerReadyState, ReadyProbeSegment,
};
use crate::{
    bindings::{
        formatters::format_source_buffer_creation_err_for_js, jsAnnounceTrackUpdate,
        jsAnnounceVariantLockStatusChange, jsAnnounceVariantUpdate, jsInspectSegment,
        jsSendMediaPlaylistParsingError, jsSendMediaPlaylistRequestError,
        jsSendMultivariantPlaylistParsingError, jsSendMultivariantPlaylistRequestError,
        jsSendOtherError, jsSendPushedSegmentError, jsSendRemoveBufferError,
        jsSendSegmentParsingError, jsSendSegmentRequestError, jsSendSourceBufferCreationError,
        jsSetMediaSourceDuration, jsStopObservingPlayback, jsUpdateContentInfo,
        AddSourceBufferErrorCode, MediaType, MultivariantPlaylistParsingErrorCode, OtherErrorCode,
        PushedSegmentErrorCode, RequestId, SourceBufferId, TimerId,
    },
    dispatcher::segment_request_contexts::PendingSegmentRequest,
    media_element::SegmentPushMetadata,
    parser::{TopLevelPlaylist, TopLevelPlaylistParsingError},
    playlist_store::{
        LockVariantResponse, MediaPlaylistPermanentId, PlaylistStore, ProbeSegmentMetadata,
        SetAudioTrackResponse, VariantUpdateResult,
    },
    requester::{
        FinishedRequestType, PlaylistFileType, PlaylistRequestInfo, RetryResult, SegmentRequestInfo,
    },
    utils::{logger::*, url::Url},
};

mod startup;

impl Dispatcher {
    fn announce_current_audio_track(&self) {
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        let fixed_audio_track = pl_store.fixed_audio_track_id();
        jsAnnounceTrackUpdate(
            MediaType::Audio,
            fixed_audio_track.or_else(|| pl_store.current_audio_track_id()),
            fixed_audio_track.is_some(),
        );
    }

    /// Completely stop playback of the current content if one and free all its associated
    /// resources.
    pub(super) fn stop_current_content(&mut self) {
        log_info!("Core: Stopping current content (if one) and resetting player");
        self.requester.reset();
        self.segment_request_contexts.clear();
        jsStopObservingPlayback();
        self.media_element_ref.reset();
        self.segment_selectors.reset_selectors(0.);
        self.playlist_store = None;
        self.ready_probe_segments.clear();
        self.initial_audio_track_selection.clear();
        self.last_position = 0.;
        self.clean_up_playlist_refresh_timers();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Check which is the best HLS variant to select according to the current conditions
    /// If it changed, handle the consequences (such as requesting new media playlists, loading
    /// and pushing segments etc.).
    pub(super) fn check_best_variant(&mut self) {
        let bandwidth = self.adaptive_selector.get_estimate();
        log_debug!("Core: received bandwidth estimate: {}", bandwidth);
        let speed = self.media_element_ref.wanted_speed();
        let buffer_level = self.media_element_ref.last_buffer_gap();
        let actually_used_bandwidth = if speed.is_finite() && speed > 0.0 {
            bandwidth / speed
        } else {
            bandwidth
        };

        let variant_id = {
            let Some(pl_store) = self.playlist_store.as_ref() else {
                return;
            };
            let segment_duration = pl_store.segment_target_duration();
            let variants = pl_store.variants_for_curr_track();
            self.adaptive_selector.select_variant(
                &variants,
                pl_store.curr_variant().map(|v| v.id()),
                actually_used_bandwidth,
                buffer_level,
                self.buffer_goal,
                segment_duration,
            )
        };

        if let Some(pl_store) = self.playlist_store.as_mut() {
            let _ = pl_store.update_curr_bandwidth(actually_used_bandwidth);
            if let Some(variant_id) = variant_id {
                let update = pl_store.update_curr_variant(variant_id);
                self.handle_variant_update(update, false);
            }
        }
    }

    /// Begin "locking" HLS variant whose `id` is given in argument, meaning that we will keep only
    /// playing that one.
    pub(super) fn lock_variant_core(&mut self, variant_id: u32) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            let is_audio_track_distinct = pl_store.current_audio_track_id().is_some();
            match pl_store.lock_variant(variant_id) {
                LockVariantResponse::NoVariantWithId => {
                    log_warn!("Core: Locked variant not found");
                    jsSendOtherError(
                        false,
                        crate::bindings::OtherErrorCode::UnfoundLockedVariant,
                        &format!("Wanted locked variant \"{variant_id}\" not found"),
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
                            is_audio_track_distinct,
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
            self.check_best_variant();
        }
    }

    /// Method to call once a timer for Playlist refresh, started with the jsTimer JavaScript
    /// function, has finished, with the corrsonding `TimerId` as argument.
    pub(super) fn on_playlist_refresh_timer_ended(&mut self, id: TimerId) {
        let (Some(playlist_id), Some(playlist_store)) = (
            self.playlist_refresh_timers.resolve_timer(id),
            &self.playlist_store,
        ) else {
            return;
        };

        if let (Some(url), Some(media_type)) = (
            playlist_store.media_playlist_url(&playlist_id),
            playlist_store.media_type_for(&playlist_id),
        ) {
            let playlist_type = PlaylistFileType::MediaPlaylist {
                id: playlist_id,
                media_type,
            };
            if !self.requester.is_requesting_playlist(url, &playlist_type) {
                self.requester.fetch_playlist(url.clone(), playlist_type);
            }
        } else {
            log_error!("Core: Cannot refresh Media Playlist: id not found");
        }
    }

    /// Method to call once a timer for retrying a request, started with the jsTimer JavaScript
    /// function, has finished, with the corrsonding `TimerId` as argument.
    pub(super) fn on_retry_request(&mut self, id: TimerId) {
        self.requester.on_timer_finished(id);
    }

    /// Set an audio track whose `id` is given in argument.
    pub(super) fn set_audio_track_core(&mut self, track_id: Option<u32>) {
        let update_result = if let Some(ref mut pl_store) = self.playlist_store {
            match pl_store.set_audio_track(track_id) {
                SetAudioTrackResponse::AudioMediaUpdate => Some((true, None)),
                SetAudioTrackResponse::VariantUpdate {
                    updates,
                    unlocked_variant,
                } => Some((true, Some((updates, unlocked_variant)))),
                SetAudioTrackResponse::NoUpdate => Some((false, None)),
            }
        } else {
            None
        };

        let Some((should_announce_track, variant_update)) = update_result else {
            return;
        };

        if should_announce_track {
            self.announce_current_audio_track();
        }

        if let Some((updates, unlocked_variant)) = variant_update {
            self.handle_variant_update(updates, true);
            if unlocked_variant {
                jsAnnounceVariantLockStatusChange(None);
            }
            self.check_best_variant();
        } else if should_announce_track {
            self.handle_media_playlist_update(&[MediaType::Audio], true, true);
            self.check_best_variant();
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
            None => log_warn!("Core: Unknown request finished"),
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
            // Failing segment request
            RetryResult::Failed {
                request_type: FinishedRequestType::Segment(s),
                reason,
                status,
            } => {
                let req_ctxt = self.segment_request_contexts.take(s.id());
                if let Some(media_type) = req_ctxt
                    .as_ref()
                    .and_then(|ctxt| ctxt.requested_media_type())
                {
                    self.ready_probe_segments.clear_media_type(media_type);
                }

                if status.is_some_and(|s| s == 404 || s == 410)
                    && req_ctxt.as_ref().is_some_and(|ctxt| {
                        utils::is_stale_segment_request_context(self.playlist_store.as_ref(), ctxt)
                    })
                {
                    log_info!(
                        "Core: Ignoring terminal 404/410 for segment no longer in the live window",
                    );
                    if req_ctxt
                        .as_ref()
                        .is_some_and(PendingSegmentRequest::is_probe)
                    {
                        self.recheck_player_state();
                    } else {
                        self.check_segments_to_request();
                    }
                    return;
                }

                let time_info = s.time_info();
                jsSendSegmentRequestError(
                    true,
                    s.url().get_ref(),
                    time_info.is_none(),
                    time_info.map(|t| (t.start(), t.end())),
                    s.media_type(),
                    reason,
                    status,
                );
                self.stop_current_content();
            }
            // Failing playlist request
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
                            Some(media_type),
                            status,
                        );
                    }
                    PlaylistFileType::TopLevelPlaylist => {
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
                    request_info.time_info().map(|t| (t.start(), t.end())),
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
                PlaylistFileType::TopLevelPlaylist => jsSendMultivariantPlaylistRequestError(
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
                        Some(media_type),
                        status,
                    )
                }
            },

            RetryResult::NotFound => {
                log_warn!("Core: Request failed not found on the current Requester")
            }
        }
    }

    pub(super) fn on_request_progress_core(
        &mut self,
        request_id: RequestId,
        bytes_loaded: u32,
        bytes_total: Option<u32>,
        duration_ms: f64,
    ) {
        self.requester.on_segment_request_progress(
            request_id,
            bytes_loaded,
            bytes_total,
            duration_ms,
        );
        self.maybe_abandon_pending_segment_request();
    }

    /// Method to call when the `readyState` JS attribute of the linked `MediaSource` object
    /// changed, with that new state in argument.
    pub(super) fn on_media_source_state_change_core(&mut self, state: MediaSourceReadyState) {
        log_info!("Core: MediaSource state changed: {:?}", state);
        self.media_element_ref
            .update_media_source_ready_state(state);
        self.recheck_player_state();
    }

    /// Method to call when a `SourceBuffer`'s creation failed.
    pub(super) fn on_source_buffer_creation_error_core(
        &mut self,
        source_buffer_id: SourceBufferId,
        original_error: (AddSourceBufferErrorCode, Option<String>),
    ) {
        if let Some((media_type, e)) = self
            .media_element_ref
            .on_source_buffer_creation_error(source_buffer_id, original_error)
        {
            let (code, msg) = format_source_buffer_creation_err_for_js(e);
            jsSendSourceBufferCreationError(true, code, media_type, &msg);
            self.stop_current_content();
        }
    }

    /// Method to call when a SourceBuffer triggered an `updateend` event.
    pub(super) fn on_source_buffer_update_core(
        &mut self,
        source_buffer_id: SourceBufferId,
        buffered: JsTimeRanges,
    ) {
        self.media_element_ref
            .on_source_buffer_update(source_buffer_id, buffered, true);
    }

    /// Method to call when a `SourceBuffer`'s `appendBuffer` call led to an `error` event.
    pub(super) fn on_append_buffer_error_core(
        &mut self,
        source_buffer_id: SourceBufferId,
        code: PushedSegmentErrorCode,
        buffered: JsTimeRanges,
    ) {
        self.media_element_ref
            .on_source_buffer_update(source_buffer_id, buffered, false);

        match self.media_element_ref.media_type_for(source_buffer_id) {
            Some(mt) => {
                if code == PushedSegmentErrorCode::BufferFull {
                    let wanted_pos = self.media_element_ref.wanted_position();
                    let min_pos = if wanted_pos < 10. {
                        0.
                    } else {
                        wanted_pos - 10.
                    };
                    let max_pos = wanted_pos + self.buffer_goal + 10.;

                    let has_segments_to_delete =
                        self.media_element_ref.inventory(mt).iter().any(|x| {
                            x.last_buffered_start() < min_pos || x.last_buffered_end() > max_pos
                        });
                    if has_segments_to_delete {
                        log_warn!(
                            "BufferFull error received for {}. Cleaning < {}, > {}.",
                            mt,
                            min_pos,
                            max_pos
                        );
                        if let (Ok(_), Ok(_)) = (
                            self.media_element_ref.remove_data(mt, 0., min_pos),
                            self.media_element_ref.remove_data(mt, max_pos, f64::MAX),
                        ) {
                            self.segment_selectors
                                .restart_from_position(wanted_pos - 0.2);
                            return;
                        }
                    }

                    // TODO Dynamically reduce the buffer goal after repeated
                    // BufferFull errors?
                }

                let message = match code {
                    PushedSegmentErrorCode::BufferFull => format!(
                        "The {mt} `SourceBuffer` was full and could not accept anymore segment"
                    ),
                    PushedSegmentErrorCode::UnknownError => format!(
                        "An error happened while calling `appendBuffer` on the {mt} `SourceBuffer`"
                    ),
                };
                jsSendPushedSegmentError(true, code, mt, &message);
            }
            None => jsSendOtherError(
                true,
                OtherErrorCode::Unknown,
                "An unknown SourceBuffer failed during a push operation.",
            ),
        }
        self.stop_current_content();
    }

    /// Method to call when a `SourceBuffer`'s `remove` call led to an `error` event.
    pub(super) fn on_remove_buffer_error_core(
        &mut self,
        source_buffer_id: SourceBufferId,
        buffered: JsTimeRanges,
    ) {
        self.media_element_ref
            .on_source_buffer_update(source_buffer_id, buffered, false);
        match self.media_element_ref.media_type_for(source_buffer_id) {
            Some(mt) => {
                let message =
                    &format!("An error happened while calling `remove` on the {mt} `SourceBuffer`");
                jsSendRemoveBufferError(true, mt, message);
            }
            None => jsSendOtherError(
                true,
                OtherErrorCode::Unknown,
                "An unknown SourceBuffer failed during a remove operation.",
            ),
        }
        self.stop_current_content();
    }

    /// Method to call when a new `MediaObservation` has been received.
    pub(super) fn on_observation(&mut self, observation: MediaObservation) {
        let reason = observation.reason();
        log_debug!("Tick received: {:?} {}", reason, observation.current_time());
        self.media_element_ref.on_observation(observation);
        match reason {
            PlaybackTickReason::Seeking => self.on_seek(),
            _ => self.on_regular_tick(),
        }
    }

    /// Method to call when a new codec support report has been received.
    pub(super) fn on_codecs_support_update_core(&mut self) {
        self.recheck_player_state();
    }

    /// For each media type, check if segment need to be requested, and if that's the case, perform
    /// the request.
    ///
    /// This method is intelligent enough to not do new requests if some are already pending for
    /// the same type, meaning that you can call it any time you may want to check if segments can
    /// be requested (when a request finished, when a media playlist has been updated, when the
    /// playhead advances etc.).
    pub(super) fn check_segments_to_request(&mut self) {
        if !self.ready_to_load_segments() {
            return;
        }

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

    /// Once a "probe segment" has been loaded, it needs to be inspected so information
    /// can be extracted from it and the state be updated accordingly.
    ///
    /// This is what this method does: inspect the segment and update the playlist state
    /// accordingly.
    fn do_probe_segment_inspection(
        &mut self,
        probe_segment: ProbeSegmentMetadata,
        requested_media_type: Option<MediaType>,
        data: JsMemoryBlob,
    ) {
        let Some(playlist_store) = self.playlist_store.as_mut() else {
            log_error!("Core: asked to do segment inspection without having a playlist store",);
            return;
        };

        let inspection = match jsInspectSegment(data.id()) {
            Ok(inspection) => inspection,
            Err((code, message)) => {
                jsSendSegmentParsingError(
                    true,
                    code,
                    requested_media_type,
                    message
                        .as_deref()
                        .unwrap_or("Unknown probe segment parsing error"),
                );
                self.stop_current_content();
                return;
            }
        };

        // NOTE: we prefer giving the requested media type than what the segment turned out to be
        // because if what was assumed to be a Video playlist turns out to be finally an audio-only
        // one, the `PlaylistStore` will be completely lost when we tell him to update the `Audio`
        // playlist where all it can see is a `Video` playlist.
        // We could consider that this case needs to be fixed but it's for a condition that is so
        // rare that I don't think we really gain in doing a complex playlist-type-change path.
        let considered_media_type = if let Some(media_type) = requested_media_type {
            media_type
        } else {
            inspection.media_type
        };

        // Now update playlist information with inspection result
        let media_info = crate::parser::ExternalMediaInfo {
            mime_type: inspection.mime_type,
            media_type: inspection.media_type,
            codec: inspection.codec,
        };
        playlist_store.set_external_media_info(media_info, considered_media_type);

        // That might have led to more timing-related information
        jsUpdateContentInfo(
            playlist_store.current_estimated_minimum_position(),
            playlist_store.current_estimated_maximum_position(),
            playlist_store.playlist_type(),
            playlist_store.is_finalized(),
            playlist_store.uses_program_date_time(),
        );
        sync_media_source_duration(playlist_store);
        self.refresh_terminal_buffer_state();

        // Store segment for future playback
        self.ready_probe_segments.insert(ReadyProbeSegment {
            request: probe_segment,
            media_type: considered_media_type,
            data,
        });
    }

    /// Method called once a playlist request ended with success
    fn on_playlist_fetch_success(
        &mut self,
        pl_info: PlaylistRequestInfo,
        data: Vec<u8>,
        final_url: Url,
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        match playlist_type {
            PlaylistFileType::TopLevelPlaylist => {
                self.on_top_level_playlist_loaded(data, final_url)
            }

            PlaylistFileType::MediaPlaylist { id, media_type } => {
                log_info!(
                    "Media playlist loaded successfully: {}",
                    final_url.get_ref(),
                );
                let refresh_interval = {
                    let Some(playlist_store) = self.playlist_store.as_mut() else {
                        jsSendOtherError(
                            true,
                            OtherErrorCode::Unknown,
                            "Media playlist loaded but no top-level playlist",
                        );
                        self.stop_current_content();
                        return;
                    };
                    match playlist_store.update_media_playlist(&id, data.as_ref(), final_url) {
                        Err(e) => {
                            let err_message = e.to_string();
                            jsSendMediaPlaylistParsingError(
                                true,
                                e.into(),
                                Some(media_type),
                                &err_message,
                            );
                            self.stop_current_content();
                            return;
                        }
                        Ok(p) => p.refresh_interval(),
                    }
                };
                self.process_parsed_media_playlist(id, refresh_interval);
            }
        }
    }

    /// Method called once the top-level Playlist was loaded with success, with its response data
    /// and url as argument.
    fn on_top_level_playlist_loaded(&mut self, data: Vec<u8>, playlist_url: Url) {
        log_info!("Core: top-level playlist loaded");
        match TopLevelPlaylist::parse(data.as_ref(), playlist_url) {
            Err(err) => {
                let message = err.to_string();
                match err {
                    TopLevelPlaylistParsingError::Multivariant(err) => {
                        jsSendMultivariantPlaylistParsingError(true, err.into(), &message);
                    }
                    TopLevelPlaylistParsingError::Media(err) => {
                        jsSendMediaPlaylistParsingError(true, err.into(), None, &message);
                    }
                    TopLevelPlaylistParsingError::NotAPlaylist => {
                        jsSendMultivariantPlaylistParsingError(
                            true,
                            MultivariantPlaylistParsingErrorCode::NotAPlaylist,
                            &message,
                        );
                    }
                }
                self.stop_current_content();
            }
            Ok(pl) => {
                log_info!("Core: top-level playlist parsed successfully");
                let estimate = self.adaptive_selector.get_estimate();
                match PlaylistStore::try_new(pl, estimate) {
                    Ok(mut pl_store) => {
                        self.apply_initial_audio_track_selection(&mut pl_store);
                        let direct_media_refresh = pl_store
                            .direct_media_playlist()
                            .map(|(id, playlist)| (*id, playlist.refresh_interval()));

                        self.playlist_store = Some(pl_store);
                        if let Some((playlist_id, refresh_interval)) = direct_media_refresh {
                            self.process_parsed_media_playlist(playlist_id, refresh_interval);
                        }
                        self.recheck_player_state();
                    }
                    Err(err) => {
                        utils::handle_playlist_store_error(err);
                        self.stop_current_content();
                    }
                }
            }
        }
    }

    fn apply_initial_audio_track_selection(&mut self, pl_store: &mut PlaylistStore) {
        let initial_audio_track_selection = std::mem::take(&mut self.initial_audio_track_selection);
        let Some(track_id) = initial_audio_track_selection.iter().find_map(|selection| {
            pl_store
                .audio_tracks()
                .iter()
                .find(|track| selection.matches(track))
                .map(|track| track.id())
        }) else {
            return;
        };
        pl_store.set_audio_track(Some(track_id));
    }

    /// Method called once a Media Playlist was parsed with success
    fn process_parsed_media_playlist(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        refresh_interval: Option<f64>,
    ) {
        self.playlist_refresh_timers
            .set_timer(playlist_id, refresh_interval);

        let Some(playlist_store) = self.playlist_store.as_ref() else {
            return;
        };
        if let Some(duration) = playlist_store.segment_target_duration() {
            let mut min_buffer_time = f64::max(3., duration - 1.);
            min_buffer_time = f64::min(8., min_buffer_time);
            log_debug!("Core: Updating min_buffer_time: {min_buffer_time}");
            self.media_element_ref
                .update_min_buffer_time(min_buffer_time);
        }

        // That might have led to more timing-related information
        jsUpdateContentInfo(
            playlist_store.current_estimated_minimum_position(),
            playlist_store.current_estimated_maximum_position(),
            playlist_store.playlist_type(),
            playlist_store.is_finalized(),
            playlist_store.uses_program_date_time(),
        );
        sync_media_source_duration(playlist_store);
        self.refresh_terminal_buffer_state();
        self.recheck_player_state();
    }

    /// If a playlist only became terminal on a later refresh, its last segment
    /// may already have been appended. Re-check buffered tails so the
    /// corresponding SourceBuffer can still transition to end-of-stream.
    fn refresh_terminal_buffer_state(&mut self) {
        let Some(playlist_store) = self.playlist_store.as_ref() else {
            return;
        };
        if !playlist_store.is_finalized() {
            return;
        }

        for media_type in [MediaType::Audio, MediaType::Video] {
            let Some(last_buffered_segment) = self.media_element_ref.inventory(media_type).last()
            else {
                continue;
            };

            if playlist_store
                .is_last_media_segment(media_type, last_buffered_segment.playlist_start())
            {
                self.media_element_ref.end_buffer(media_type);
            }
        }
    }

    fn on_regular_tick(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.last_position = wanted_pos;

        // Lock `Requester`, so it only do new segment requests when every
        // wanted segments is scheduled - for better priorization
        let was_already_locked = self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.segment_selectors.advance_position(wanted_pos - 0.2);

        self.check_best_variant();
        self.maybe_abandon_pending_segment_request();
        self.check_segments_to_request();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }

        self.check_gap_jumping_after_segment_scheduling(wanted_pos);
    }

    /// Once every segments have been scheduled for the current tick, check if gap jumping (jumping
    /// after holes in the buffer) is needed. If so, seek over it.
    fn check_gap_jumping_after_segment_scheduling(&mut self, wanted_pos: f64) {
        if !self.media_element_ref.is_rebuffering() {
            return; // We can play, no point in skipping anything
        }
        if !self.ready_to_load_segments() {
            return; // ensure everything is ready before doing anything
        }
        let playable_until = self
            .media_element_ref
            .current_buffered_range()
            .map(|(_, end)| end)
            .unwrap_or(wanted_pos);

        if let Some(next_range_start) = self.media_element_ref.next_buffered_range().map(|x| x.0) {
            if next_range_start - playable_until >= 0.
                && !self.requester.has_pending_segment_before(next_range_start)
            {
                log_warn!(
                    "Core: Jumping over a browser buffer hole (p:{}, e:{}, n:{})",
                    wanted_pos,
                    playable_until,
                    next_range_start
                );
                self.media_element_ref.seek(next_range_start + 0.01);
            }
        }
    }

    /// Actions to perform once a seek has been performed on the media element.
    fn on_seek(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.segment_selectors
            .restart_from_position(wanted_pos - 0.2);

        self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.check_requested_segments_still_needed();
        self.check_segments_to_request();
        self.requester.unlock_segment_requests();
    }

    /// Check that all pending initialization and media requests still correspond to the most
    /// needed segments.
    ///
    /// If not, abort the corresponding pending requests.
    ///
    /// This method is intended to be called on exceptional events which may have led to a
    /// potential change of segment priorization, such as a seek.
    fn check_requested_segments_still_needed(&mut self) {
        [MediaType::Audio, MediaType::Video]
            .into_iter()
            .for_each(|mt| {
                let Some(pl_store) = self.playlist_store.as_ref() else {
                    self.abort_segment_requests_with_type(mt);
                    return;
                };

                let inventory = self.media_element_ref.inventory(mt);
                if let Some(seg_info) = pl_store.loaded_media_playlist_segment_info(mt) {
                    let needed_segment = self.segment_selectors.get_mut(mt).most_needed_segment(
                        seg_info.0,
                        &seg_info.1,
                        inventory,
                    );

                    if let Some(i) = needed_segment.init_segment() {
                        if !self
                            .requester
                            .is_requesting_segment(mt, i.url(), i.byte_range())
                        {
                            log_debug!(
                                "Core: {mt} init segment request not needed anymore, abort."
                            );
                            self.abort_segment_requests_with_type(mt);
                        } else {
                            log_debug!("Core: {mt} init segment request still needed.");
                        }
                    } else if let Some(seg) = needed_segment.media_segment() {
                        if !self
                            .requester
                            .is_requesting_segment(mt, seg.url(), seg.byte_range())
                        {
                            log_debug!(
                                "Core: {mt} media segment request not needed anymore, abort."
                            );
                            self.abort_segment_requests_with_type(mt);
                        } else {
                            log_debug!("Core: {mt} media segment request still needed.");
                        }
                    } else {
                        self.abort_segment_requests_with_type(mt);
                    }
                }
            });
    }

    fn check_segment_to_request_for_type(&mut self, media_type: MediaType) {
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        if !self.requester.has_segment_request_pending(media_type) {
            let inventory = self.media_element_ref.inventory(media_type);
            if let Some(seg_info) = pl_store.loaded_media_playlist_segment_info(media_type) {
                let most_needed_segment = self
                    .segment_selectors
                    .get_mut(media_type)
                    .most_needed_segment(seg_info.0, &seg_info.1, inventory);
                if let Some(i) = most_needed_segment.init_segment() {
                    let req_id =
                        self.segment_request_contexts
                            .insert(PendingSegmentRequest::Init {
                                media_type,
                                init_segment_id: i.id(),
                            });
                    self.requester.request_init_segment(
                        media_type,
                        i.url().clone(),
                        i.byte_range(),
                        req_id,
                    );
                } else if let Some(seg) = most_needed_segment.media_segment() {
                    let init_segment_id = seg_info.0.init_for(seg).map(|i| i.id());
                    let req_id =
                        self.segment_request_contexts
                            .insert(PendingSegmentRequest::Media {
                                media_type,
                                time_info: seg.time_info().clone(),
                                init_segment_id,
                                quality_context: seg_info.1,
                                variant_bandwidth: pl_store
                                    .curr_variant()
                                    .map(|variant| variant.bandwidth())
                                    .unwrap_or(0),
                                sequence_number: seg.sequence(),
                                discontinuity_sequence: seg.discontinuity_sequence(),
                            });
                    self.requester
                        .request_media_segment(media_type, seg, req_id);
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
            jsAnnounceVariantUpdate(pl_store.current_variant_id());
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
        if self.playlist_store.is_none() {
            return;
        }

        for mt in changed_media_types.iter().copied() {
            log_info!("Core: {} MediaPlaylist changed", mt);
            self.ready_probe_segments.clear_media_type(mt);

            if abort_prev {
                self.abort_segment_requests_with_type(mt);
            }
            if flush {
                if let Err(e) = self.media_element_ref.flush(mt) {
                    log_warn!("Could not remove data from the previous {mt} buffer: {}", e);
                }
                self.segment_selectors
                    .get_mut(mt)
                    .restart_from_position(self.media_element_ref.wanted_position() - 0.2);
            }

            let playlist_to_fetch = self.playlist_store.as_ref().and_then(|pl_store| {
                if pl_store.has_loaded_media_playlist(mt) {
                    None
                } else {
                    let id = *pl_store.media_playlist_id_for(mt)?;
                    let url = pl_store.media_playlist_url(&id)?.clone();
                    Some((id, url))
                }
            });

            if let Some((id, url)) = playlist_to_fetch {
                use PlaylistFileType::*;
                log_debug!("Core: Media changed, requesting its media playlist");
                let playlist_type = MediaPlaylist { id, media_type: mt };
                if !self.requester.is_requesting_playlist(&url, &playlist_type) {
                    self.requester.fetch_playlist(url, playlist_type);
                }
            }
        }

        if !changed_media_types.is_empty() {
            self.clean_up_playlist_refresh_timers();
        }
        self.check_segments_to_request();
    }

    /// Method called once a segment request ended with success
    fn on_segment_fetch_success(
        &mut self,
        segment_req: SegmentRequestInfo,
        result: JsMemoryBlob,
        resource_size: u32,
        duration_ms: f64,
    ) {
        log_info!(lazy: || {
            let lane_label = segment_req.lane_tag().label();
            match segment_req.time_info() {
                None => format!("Loaded {} init segment", lane_label),
                Some(time_info) => format!(
                    "Loaded {} segment: t: {}, d: {}",
                    lane_label,
                    time_info.start(),
                    time_info.duration()
                ),
            }
        });

        self.adaptive_selector
            .add_metric(duration_ms, resource_size);

        let media_type = segment_req.media_type();
        let Some(req_ctxt) = self.segment_request_contexts.take(segment_req.id()) else {
            log_warn!("Loaded segment with unknown pending context.");
            return;
        };

        match req_ctxt {
            PendingSegmentRequest::Media {
                media_type: req_media_type,
                init_segment_id,
                time_info,
                quality_context,
                sequence_number,
                discontinuity_sequence,
                ..
            } => {
                if Some(req_media_type) != media_type {
                    log_warn!("Loaded media segment with mismatched media type context.");
                }
                self.on_media_segment_loaded(SegmentPushMetadata {
                    data: result,
                    media_type: req_media_type,
                    init_segment_id,
                    time_info,
                    context: quality_context,
                    sequence_number,
                    discontinuity_sequence,
                });
            }
            PendingSegmentRequest::Init {
                media_type: req_media_type,
                init_segment_id,
            } => {
                if Some(req_media_type) != media_type {
                    log_warn!("Loaded init segment with mismatched media type context.");
                }
                self.on_init_segment_loaded(result, req_media_type, init_segment_id);
            }
            PendingSegmentRequest::Probe {
                probe_segment,
                requested_media_type,
            } => {
                self.do_probe_segment_inspection(probe_segment, requested_media_type, result);
                self.recheck_player_state();
            }
        }
    }

    fn on_media_segment_loaded(&mut self, push_md: SegmentPushMetadata) {
        let segment_start = push_md.time_info.start();
        let segment_end = push_md.time_info.end();
        let media_type = push_md.media_type;
        let prepared_data = self
            .media_element_ref
            .announce_incoming_media_segment(push_md);

        self.check_best_variant();

        // Check next segment BEFORE actually pushing, as the pushing operation could take in the
        // tens of ms or even in the hundreds depending on segment size and platform performance.
        //
        // We still announce the incoming segment first to ensure the `MediaElementReference`'s
        // inventory is up-to-date.
        self.segment_selectors
            .get_mut(media_type)
            .validate_media_until(segment_end);
        self.check_segments_to_request();

        match self
            .media_element_ref
            .push_media_segment(media_type, prepared_data)
        {
            Err(x) => {
                let media_type = x.media_type();
                let message = x.to_string();
                log_warn!(
                    "Core: {} media segment push failed start:{} end:{} err:{}",
                    media_type,
                    segment_start,
                    segment_end,
                    message
                );
                jsSendSegmentParsingError(true, x.into(), Some(media_type), &message);
                self.stop_current_content();
            }
            Ok(()) => {
                if utils::was_last_segment(self.playlist_store.as_ref(), media_type, segment_start)
                {
                    log_info!(
                        "Last {} segment request finished, declaring its buffer's end",
                        media_type
                    );
                    self.media_element_ref.end_buffer(media_type);
                }
            }
        }
    }

    fn on_init_segment_loaded(&mut self, data: JsMemoryBlob, media_type: MediaType, init_id: f64) {
        match self.media_element_ref.push_init_segment(media_type, data) {
            Err(x) => {
                let media_type = x.media_type();
                let message = x.to_string();
                log_warn!(
                    "Core: {} init segment push failed id:{} err:{}",
                    media_type,
                    init_id,
                    message
                );
                jsSendSegmentParsingError(true, x.into(), Some(media_type), &message);
                self.stop_current_content();
            }
            Ok(()) => self
                .segment_selectors
                .get_mut(media_type)
                .validate_init(init_id),
        }

        self.check_best_variant();
        self.check_segments_to_request();
    }

    fn abort_segment_requests_with_type(&mut self, media_type: MediaType) {
        let aborted_reqs = self.requester.abort_segments_with_type(media_type);
        for req_id in aborted_reqs {
            if let Some(media_type) = self
                .segment_request_contexts
                .take(req_id)
                .and_then(|ctxt| ctxt.requested_media_type())
            {
                self.ready_probe_segments.clear_media_type(media_type);
            }
        }
    }

    fn maybe_abandon_pending_segment_request(&mut self) {
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        if pl_store.is_variant_locked() || !self.media_element_ref.can_monitor_abr_requests() {
            return;
        }
        let Some(buffer_starvation_delay) = self.media_element_ref.starvation_delay() else {
            return;
        };
        let Some(pending_request) = self.requester.pending_segment_request(MediaType::Video) else {
            return;
        };
        let Some(pending_context) = self.segment_request_contexts.get(pending_request.id()) else {
            return;
        };
        if pending_context.requested_media_type() != Some(MediaType::Video) {
            return;
        }

        let Some((_, desired_quality_context)) =
            pl_store.curr_media_playlist_segment_info(MediaType::Video)
        else {
            return;
        };
        let Some(desired_variant_bandwidth) =
            pl_store.curr_variant().map(|variant| variant.bandwidth())
        else {
            return;
        };
        let (Some(pending_quality_context), Some(pending_variant_bandwidth)) = (
            pending_context.quality_context(),
            pending_context.variant_bandwidth(),
        ) else {
            return;
        };
        let segment_duration = pending_context
            .time_info()
            .map(|time_info| time_info.duration());

        if self.adaptive_selector.should_abandon_media_request(
            pending_quality_context,
            &desired_quality_context,
            pending_variant_bandwidth,
            desired_variant_bandwidth,
            pending_request.bytes_loaded(),
            pending_request.bytes_total(),
            pending_request.progress_duration_ms(),
            pending_request.progress_samples(),
            self.media_element_ref.wanted_speed(),
            segment_duration,
            buffer_starvation_delay,
        ) {
            log_info!("Core: Abandoning pending higher-quality video segment request");
            self.abort_segment_requests_with_type(MediaType::Video);
        }
    }

    /// Removes from `self.playlist_refresh_timers` timers for playlist that are not current
    /// anymore and abort their corresponding timers
    fn clean_up_playlist_refresh_timers(&mut self) {
        if let Some(ref pl_store) = self.playlist_store {
            self.playlist_refresh_timers
                .retain(|id| pl_store.is_current_media_playlist(id))
        } else {
            self.playlist_refresh_timers.clear_all_timers();
        }
    }
}

fn sync_media_source_duration(playlist_store: &PlaylistStore) {
    if playlist_store.is_finalized() {
        if let Some(duration) = playlist_store.current_estimated_duration() {
            let _ = jsSetMediaSourceDuration(duration);
        } else {
            log_warn!("Core: Unknown finalized content duration");
        }
    } else {
        let _ = jsSetMediaSourceDuration(u32::MAX as f64);
    }
}
