use super::{
    event_listeners::JsTimeRanges, utils, Dispatcher, JsMemoryBlob, MediaObservation,
    MediaSourceReadyState, PlaybackTickReason, PlayerReadyState, ReadyProbeSegment,
};
use crate::{
    adaptive::PlaybackConditions,
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
    ///
    /// # Arguments
    ///
    /// * `flush` - To set to `true` if you want a potential resulting quality or track
    ///   change to take effect as soon as possible, even if it means a rebuffering period.
    pub(super) fn check_best_variant(&mut self, flush: bool) {
        if let Some((update, allow_fast_quality_switching)) = self.set_optimal_variant() {
            self.handle_variant_update(update, flush, allow_fast_quality_switching);
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
                    self.handle_variant_update(updates, true, true);
                    jsAnnounceVariantLockStatusChange(Some(variant_id));
                }
            }
        }
    }

    /// Remove an HLS variant previously put in place through `lock_variant_core`.
    pub(super) fn unlock_variant_core(&mut self) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            pl_store.unlock_variant();
            self.check_best_variant(false);
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
        let Some(pl_store) = self.playlist_store.as_mut() else {
            return;
        };
        match pl_store.set_audio_track(track_id) {
            SetAudioTrackResponse::NoCompatibleVariant => jsSendOtherError(
                false,
                OtherErrorCode::Unknown,
                "Unable to select the audio track: no compatible variant",
            ),
            SetAudioTrackResponse::VariantSelectionNeeded {
                variant_lock_removed,
            } => {
                // First, update the playlist store with a compatible variant
                let variant_update = self.set_optimal_variant();

                // Second, now send event and do the actual dispatcher logic
                self.announce_current_audio_track();
                if let Some((update, allow_fast_switching)) = variant_update {
                    self.handle_variant_update(update, true, allow_fast_switching);
                }
                if variant_lock_removed {
                    jsAnnounceVariantLockStatusChange(None);
                }
            }
            SetAudioTrackResponse::AudioMediaUpdate => {
                let variant_update = self.set_optimal_variant();
                self.announce_current_audio_track();
                if let Some((update, allow_fast_switching)) = variant_update {
                    self.handle_variant_and_audio_track_update(
                        update,
                        false,
                        allow_fast_switching,
                        true,
                    );
                } else {
                    self.handle_media_playlist_update(&[MediaType::Audio], true, true);
                }
            }
            SetAudioTrackResponse::NoUpdate => self.check_best_variant(false),
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
                if let Some(playlist_data) = data.obtain() {
                    self.on_playlist_fetch_success(pl_info, playlist_data, final_url);
                } else {
                    jsSendOtherError(
                        true,
                        OtherErrorCode::Unknown,
                        "Playlist response data was missing from JS memory",
                    );
                    self.stop_current_content();
                }
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

    /// Check the optimal variant under the last known network/playback conditions.
    ///
    /// If it changed, set it on the `PlaylistStore` and return a tuple indicating what is the
    /// result of this update and whether "fast-quality switching" is enabled in this iteration.
    ///
    /// Returns `None` if switching variant is not possible at this time (current variant is locked,
    /// no `PlaylistStore`...).
    fn set_optimal_variant(&mut self) -> Option<(VariantUpdateResult, bool)> {
        if self.playlist_store.as_ref()?.is_variant_locked() {
            return None;
        }
        let (variant_id, allow_fast_switching) = self.compute_optimal_variant()?;
        let update = self
            .playlist_store
            .as_mut()?
            .update_adaptive_variant(variant_id);
        Some((update, allow_fast_switching))
    }

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

    /// Ask our adaptive logic what is the optimal variant to play according to current conditions
    /// and available variants under current constraints (tracks selected, codec compatibility
    /// etc.).
    ///
    /// Returns `None` if this cannot be computed right now. Returns a tuple with the selected
    /// variant id and whether "fast quality switching" is enabled if a variant update can be done.
    fn compute_optimal_variant(&self) -> Option<(u32, bool)> {
        let pl_store = self.playlist_store.as_ref()?;
        let compatible_variants = pl_store.compatible_variants();
        let current_variant_id = pl_store.current_variant_id();
        let selection = self.adaptive_selector.select_variant(
            &compatible_variants,
            current_variant_id,
            &self.playback_conditions(pl_store),
        )?;
        let has_replacement = [MediaType::Audio, MediaType::Video]
            .into_iter()
            .any(|media_type| {
                pl_store
                    .variant_segment_quality_context(selection.safe_variant_id, media_type)
                    .is_some_and(|context| {
                        self.segment_selectors.has_fast_quality_switch_candidate(
                            media_type,
                            &context,
                            self.media_element_ref.inventory(media_type),
                        )
                    })
            });

        if has_replacement {
            Some((selection.safe_variant_id, true))
        } else {
            // The additive recommendation can also replace buffered media only when both
            // recommendations identify the same variant.
            Some((
                selection.best_variant_id,
                selection.best_variant_id == selection.safe_variant_id,
            ))
        }
    }

    /// Generate the `PlaybackConditions` object needed by the adaptive code according to current
    /// playback conditions.
    fn playback_conditions(&self, pl_store: &PlaylistStore) -> PlaybackConditions {
        let media_type = if pl_store.has_distinct_media_type(MediaType::Video) {
            MediaType::Video
        } else {
            MediaType::Audio
        };
        PlaybackConditions {
            buffer_level: self.media_element_ref.buffer_ahead_for(media_type),
            buffer_goal: self.buffer_goal,
            playback_speed: self.media_element_ref.wanted_speed(),
            abr_reference_segment_duration: pl_store.max_observed_target_duration(),
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
            log_error!("Core: asked to do segment inspection without having a PlaylistStore",);
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
                match PlaylistStore::try_new(pl) {
                    Ok(mut pl_store) => {
                        self.apply_initial_audio_track_selection(&mut pl_store);
                        if let Some(selection) = self.adaptive_selector.select_variant(
                            &pl_store.compatible_variants(),
                            pl_store.current_variant_id(),
                            &self.playback_conditions(&pl_store),
                        ) {
                            pl_store.update_adaptive_variant(selection.best_variant_id);
                        }
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

        self.check_best_variant(false);
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
    fn handle_variant_update(
        &mut self,
        result: VariantUpdateResult,
        flush: bool,
        allow_fast_quality_switching: bool,
    ) {
        self.handle_variant_and_audio_track_update(
            result,
            flush,
            allow_fast_quality_switching,
            false,
        );
    }

    /// Apply a variant change and an optional audio track change together, before requesting
    /// segments. A track change always flushes audio, independently of the video policy.
    fn handle_variant_and_audio_track_update(
        &mut self,
        result: VariantUpdateResult,
        flush: bool,
        allow_fast_quality_switching: bool,
        audio_track_changed: bool,
    ) {
        // The recommendation's permitted use can change even when the variant ID does not.
        for media_type in [MediaType::Audio, MediaType::Video] {
            self.segment_selectors
                .set_fast_quality_switching(media_type, allow_fast_quality_switching);
        }

        let variant_changed = !matches!(&result, VariantUpdateResult::Unchanged);
        let (mut changed_media_types, has_worsened) = match result {
            VariantUpdateResult::Improved(mt) => (mt, false),
            VariantUpdateResult::EqualOrUnknown(mt) => (mt, false),
            VariantUpdateResult::Worsened(mt) => (mt, true),
            VariantUpdateResult::Unchanged if audio_track_changed => (vec![], false),
            VariantUpdateResult::Unchanged => return,
        };

        if audio_track_changed && !changed_media_types.contains(&MediaType::Audio) {
            changed_media_types.insert(0, MediaType::Audio);
        }
        self.handle_media_playlist_update_with_policy(&changed_media_types, |media_type| {
            let flush_media = flush || (audio_track_changed && media_type == MediaType::Audio);
            (flush_media || has_worsened, flush_media)
        });
        if variant_changed {
            if let Some(pl_store) = self.playlist_store.as_ref() {
                jsAnnounceVariantUpdate(pl_store.current_variant_id());
            }
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
        self.handle_media_playlist_update_with_policy(changed_media_types, |_| (abort_prev, flush));
    }

    /// Apply each media type's cancellation/flush policy and schedule segments once after all
    /// media changes have been handled.
    fn handle_media_playlist_update_with_policy(
        &mut self,
        changed_media_types: &[MediaType],
        policy: impl Fn(MediaType) -> (bool, bool),
    ) {
        if self.playlist_store.is_none() {
            return;
        }

        for mt in changed_media_types.iter().copied() {
            let (abort_prev, flush) = policy(mt);
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

        // Check next segment BEFORE actually pushing, as the pushing operation could take in the
        // tens of ms or even in the hundreds depending on segment size and platform performance.
        //
        // We still announce the incoming segment first to ensure the `MediaElementReference`'s
        // inventory is up-to-date.
        self.check_best_variant(false);
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

        self.check_best_variant(false);
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

#[cfg(test)]
mod tests {
    use super::Dispatcher;
    use crate::{
        bindings::MediaType,
        dispatcher::{JsTimeRanges, MediaObservation, PlaybackTickReason},
        parser::TopLevelPlaylist,
        playlist_store::PlaylistStore,
        utils::url::Url,
    };

    fn selected_playlist(manifest: &[u8]) -> PlaylistStore {
        let playlist = TopLevelPlaylist::parse(
            manifest,
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let mut pl_store = PlaylistStore::try_new(playlist).unwrap();
        let variant_id = pl_store.available_variants()[0].id();
        pl_store.set_variant(variant_id);
        pl_store
    }

    fn dispatcher_with_buffer_observation(video: Option<Vec<f64>>) -> Dispatcher {
        let mut dispatcher = Dispatcher::new(3_125_000.);
        dispatcher
            .media_element_ref
            .on_observation(MediaObservation::new(
                PlaybackTickReason::RegularInterval,
                10.,
                4,
                JsTimeRanges::new(vec![0., 15.]),
                false,
                false,
                false,
                f64::MAX,
                Some(JsTimeRanges::new(vec![0., 15.])),
                video.map(JsTimeRanges::new),
            ));
        dispatcher
    }

    fn dispatcher_with_abr_playlists(high_duration: Option<u32>, buffer_level: f64) -> Dispatcher {
        let mut pl_store = selected_playlist(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E\"\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS=\"avc1.42E01E\"\nmedium.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,CODECS=\"avc1.42E01E\"\nhigh.m3u8\n",
        );
        let variant_ids: Vec<_> = pl_store
            .available_variants()
            .iter()
            .map(|v| v.id())
            .collect();
        for (variant_id, duration) in variant_ids.iter().zip([Some(4), Some(4), high_duration]) {
            if let Some(duration) = duration {
                pl_store.set_variant(*variant_id);
                let id = *pl_store.media_playlist_id_for(MediaType::Video).unwrap();
                let media = format!(
                    "#EXTM3U\n#EXT-X-TARGETDURATION:{duration}\n#EXTINF:{duration},\nseg.ts\n#EXT-X-ENDLIST\n"
                );
                pl_store
                    .update_media_playlist(
                        &id,
                        media.as_bytes(),
                        Url::new("https://example.com/media.m3u8".to_string()),
                    )
                    .unwrap();
            }
        }
        pl_store.set_variant(variant_ids[0]);
        let mut dispatcher = dispatcher_with_buffer_observation(Some(vec![0., 10. + buffer_level]));
        dispatcher.adaptive_selector = crate::adaptive::AdaptiveQualitySelector::new(1_875_000.);
        dispatcher.playlist_store = Some(pl_store);
        dispatcher
    }

    #[test]
    fn adaptive_selection_stays_stable_while_the_selected_playlist_loads() {
        let mut dispatcher = dispatcher_with_abr_playlists(None, 30.);
        let high_id = dispatcher
            .playlist_store
            .as_ref()
            .unwrap()
            .available_variants()[2]
            .id();

        for _ in 0..6 {
            let (variant_id, allow_fast_switching) = dispatcher.compute_optimal_variant().unwrap();
            assert_eq!(variant_id, high_id);
            assert!(!allow_fast_switching);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.update_adaptive_variant(variant_id);
            assert!(!pl_store.has_loaded_media_playlist(MediaType::Video));
            assert_eq!(pl_store.segment_target_duration(), None);
            assert_eq!(pl_store.max_observed_target_duration(), Some(4.));
        }
    }

    #[test]
    fn adaptive_selection_stays_stable_with_different_observed_target_durations() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(6), 16.5);
        let pl_store = dispatcher.playlist_store.as_ref().unwrap();
        let medium_id = pl_store.available_variants()[1].id();
        assert_eq!(pl_store.segment_target_duration(), Some(4.));
        assert_eq!(pl_store.max_observed_target_duration(), Some(6.));

        for _ in 0..6 {
            let (variant_id, _) = dispatcher.compute_optimal_variant().unwrap();
            assert_eq!(variant_id, medium_id);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.update_adaptive_variant(variant_id);
            assert_eq!(pl_store.segment_target_duration(), Some(4.));
            assert_eq!(pl_store.max_observed_target_duration(), Some(6.));
        }
    }

    #[test]
    fn playback_conditions_use_video_source_buffer_for_video_variants() {
        for manifest in [
            "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"aud\"\nvideo.m3u8\n",
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\"\nmuxed.m3u8\n",
        ] {
            let pl_store = selected_playlist(manifest.as_bytes());
            let dispatcher = dispatcher_with_buffer_observation(Some(vec![0., 40.]));

            assert_eq!(dispatcher.playback_conditions(&pl_store).buffer_level, Some(30.));
        }
    }

    #[test]
    fn playback_conditions_use_audio_source_buffer_for_audio_only_variants() {
        let pl_store = selected_playlist(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"mp4a.40.2\"\naudio.m3u8\n",
        );
        let dispatcher = dispatcher_with_buffer_observation(Some(vec![0., 40.]));

        assert_eq!(
            dispatcher.playback_conditions(&pl_store).buffer_level,
            Some(5.)
        );
    }

    #[test]
    fn playback_conditions_do_not_fall_back_to_audio_when_video_ranges_are_missing() {
        let pl_store = selected_playlist(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E\"\nvideo.m3u8\n",
        );
        for video in [None, Some(vec![])] {
            let expected = video.as_ref().map(|_| 0.);
            let dispatcher = dispatcher_with_buffer_observation(video);

            assert_eq!(
                dispatcher.playback_conditions(&pl_store).buffer_level,
                expected
            );
        }
    }
}
