use super::{
    event_listeners::JsTimeRanges, utils, Dispatcher, JsMemoryBlob, MediaObservation,
    MediaSourceReadyState, PlaybackTickReason, PlayerReadyState, ReadyProbeSegment,
};
use crate::{
    adaptive::{AdaptiveVariantSelection, PlaybackConditions},
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
    parser::{SegmentIdentity, TopLevelPlaylist, TopLevelPlaylistParsingError},
    playlist_store::{
        AudioTrackChange, LockVariantResponse, MediaPlaylistPermanentId, PlaylistStore,
        ProbeSegmentMetadata, SetAudioTrackResponse,
    },
    requester::{
        FinishedRequestType, PlaylistFileType, PlaylistRequestInfo, RetryResult, SegmentRequestInfo,
    },
    segment_selector::SegmentSelectionCandidate,
    utils::{logger::*, url::Url},
};

mod startup;

/// Describe the strategy that should be taken when switching from a video/text/audio media to
/// another.
#[derive(Clone, Copy)]
enum MediaPlaylistUpdateMode {
    /// Don't do anything with previous request and buffer linked to the previous media of the same
    /// type.
    Seamless,
    /// Abort previous requests for that media type.
    AbortRequests,
    /// Both abort previous requests and flush buffer for that media type, so the change can be
    /// taken into account as soon as possible.
    AbortRequestsAndFlushBuffer,
}

/// Describe a change of media playlist for a given media type (audio/video/text) and how the
/// player should handle the transition.
struct MediaPlaylistUpdate {
    /// Type concerned by the change.
    media_type: MediaType,
    /// How the player should handle that transition.
    mode: MediaPlaylistUpdateMode,
}

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
        self.loaded_variant_id = None;
        self.ready_probe_segments.clear();
        self.initial_audio_track_selection.clear();
        self.last_position = 0.;
        self.clean_up_playlist_refresh_timers();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Begin "locking" HLS variant whose `id` is given in argument, meaning that we will keep only
    /// playing that one.
    pub(super) fn lock_variant_core(&mut self, variant_id: u32) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            match pl_store.lock_variant(variant_id) {
                LockVariantResponse::AlreadyLocked => {}
                LockVariantResponse::NoVariantWithId => {
                    log_warn!("Core: Locked variant not found");
                    jsSendOtherError(
                        false,
                        crate::bindings::OtherErrorCode::UnfoundLockedVariant,
                        &format!("Wanted locked variant \"{variant_id}\" not found"),
                    );
                }
                LockVariantResponse::VariantLocked {
                    changed_media_types,
                    audio_track_change,
                } => {
                    // XXX TODO: risks?
                    self.playlist_refresh_timers.clear_refresh_targets();
                    if let AudioTrackChange::Changed(track_id) = audio_track_change {
                        let is_audio_track_selected = pl_store.fixed_audio_track_id().is_some();
                        jsAnnounceTrackUpdate(MediaType::Audio, track_id, is_audio_track_selected);
                    }
                    for media_type in changed_media_types.iter().copied() {
                        log_info!("Core: {} MediaPlaylist changed", media_type);
                        self.ready_probe_segments.clear_media_type(media_type);
                        self.abort_segment_requests_with_type(media_type);
                        self.flush_media_buffer(media_type);
                        self.request_current_media_playlist_if_needed(media_type);
                    }
                    // TODO: Avoid flushing an unchanged media type when its buffer already only
                    // contains data from the newly-locked variant. An unchanged playlist
                    // may still have older-quality buffered data, but when it does not this causes
                    // an unnecessary rebuffer and refetch.
                    for media_type in [MediaType::Audio, MediaType::Video] {
                        if self.media_element_ref.has_buffer(media_type)
                            && !changed_media_types.contains(&media_type)
                        {
                            self.abort_segment_requests_with_type(media_type);
                            self.flush_media_buffer(media_type);
                        }
                    }
                    if !changed_media_types.is_empty() {
                        self.clean_up_playlist_refresh_timers();
                    }
                    self.check_segments_to_request();
                    // XXX TODO: No variantUpdate?
                    jsAnnounceVariantLockStatusChange(Some(variant_id));
                }
            }
        }
    }

    /// Remove an HLS variant previously put in place through `lock_variant_core`.
    pub(super) fn unlock_variant_core(&mut self) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            pl_store.unlock_variant();
            // TODO: Did I forget to emit a lock status change here? To check.
            self.check_requested_segments_still_needed();
            self.check_segments_to_request();
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

        let media_type = self.playlist_refresh_timers.refresh_media_type(
            &playlist_id,
            playlist_store.selected_playlist_media_type(&playlist_id),
        );
        let (Some(url), Some(media_type)) =
            (playlist_store.media_playlist_url(&playlist_id), media_type)
        else {
            return;
        };
        let playlist_type = PlaylistFileType::MediaPlaylist {
            id: playlist_id,
            media_type,
        };
        if !self.requester.is_requesting_playlist(url, &playlist_type) {
            self.requester.fetch_playlist(url.clone(), playlist_type);
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
                // Choose a compatible path to inspect without relying on the old track's buffer.
                let variant_update = self.compute_optimal_variant().and_then(|selection| {
                    let store = self.playlist_store.as_mut()?;
                    Some(store.set_variant(selection.safe_variant_id))
                });

                // Second, now send event and do the actual dispatcher logic
                self.announce_current_audio_track();
                if let Some(changed_media_types) = variant_update {
                    self.handle_media_playlist_update(
                        &changed_media_types,
                        MediaPlaylistUpdateMode::AbortRequestsAndFlushBuffer,
                    );
                }
                if variant_lock_removed {
                    jsAnnounceVariantLockStatusChange(None);
                }
            }

            SetAudioTrackResponse::AudioMediaUpdate => {
                self.announce_current_audio_track();
                self.check_requested_segments_still_needed();
                // We want to abort and flush audio here, independently of the variant update, as
                // we're in an audio-track-setting context.
                self.handle_media_playlist_update(
                    &[MediaType::Audio],
                    MediaPlaylistUpdateMode::AbortRequestsAndFlushBuffer,
                );
            }
            SetAudioTrackResponse::NoUpdate => {
                self.check_requested_segments_still_needed();
                self.check_segments_to_request();
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
                if let Some(playlist_data) = data.obtain() {
                    self.on_playlist_fetch_success(pl_info, playlist_data, final_url);
                } else if !self.is_obsolete_playlist_request(&pl_info.playlist_type) {
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
        let is_obsolete_playlist = self
            .requester
            .pending_playlist_type(request_id)
            .is_some_and(|playlist_type| self.is_obsolete_playlist_request(playlist_type));

        match self.requester.on_pending_request_failure(
            request_id,
            has_timeouted,
            status,
            !is_obsolete_playlist,
        ) {
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

                // TODO: On terminal 404/410 for a still-listed cached segment, wait for the
                // requested playlist's refresh before reporting a fatal failure. If its range is
                // still playable, abandon the upgrade and exclude it from replacement selection
                // until an explicit restart, so the same failed upgrade is not retried endlessly.
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
                if is_obsolete_playlist {
                    log_info!("Core: Ignoring failure of a deselected media playlist");
                    return;
                }
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

        let recommendation = self.compute_optimal_variant();
        let was_already_locked = self.requester.lock_segment_requests();
        [MediaType::Video, MediaType::Audio]
            .into_iter()
            .for_each(|mt| {
                self.check_segment_to_request_for_type(mt, recommendation);
            });
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    /// Send `trackUpdate` event for the audio track currently-set on the `PlaylistStore`.
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
    /// Returns `None` if this cannot be computed right now. Otherwise returns the variant
    /// recommendations for additive loading and safe replacements.
    fn compute_optimal_variant(&self) -> Option<AdaptiveVariantSelection> {
        let pl_store = self.playlist_store.as_ref()?;
        if pl_store.is_variant_locked() {
            let variant_id = pl_store.current_variant_id()?;
            return Some(AdaptiveVariantSelection {
                best_variant_id: variant_id,
                safe_variant_id: variant_id,
            });
        }
        let compatible_variants = pl_store.compatible_variants();
        let current_variant_id = pl_store.current_variant_id();
        self.adaptive_selector.select_variant(
            &compatible_variants,
            current_variant_id,
            &self.playback_conditions(pl_store),
        )
    }

    /// Generate the `PlaybackConditions` object needed by the adaptive code according to current
    /// playback conditions.
    fn playback_conditions(&self, pl_store: &PlaylistStore) -> PlaybackConditions {
        // TODO: Do not count buffer that a track or variant change is about to flush.
        // The last playback observation may also still report it after a flush is queued.
        // TODO: If we introduce per-type BOLA, coordinate choices as a playable variant pair
        // and account for their combined bandwidth. For now both selectors share one ABR result.
        let buffer_level = [MediaType::Audio, MediaType::Video]
            .into_iter()
            .filter_map(|media_type| self.media_element_ref.buffer_ahead_for(media_type))
            .reduce(f64::min);
        PlaybackConditions {
            buffer_level,
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
        let is_obsolete = self.is_obsolete_playlist_request(&pl_info.playlist_type);
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
                            if is_obsolete {
                                log_info!(
                                    "Core: Ignoring parse failure of a deselected media playlist"
                                );
                                return;
                            }
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

    /// A media playlist response is obsolete only when its permanent ID is no longer
    /// selected. The same playlist may still be selected through another variant.
    fn is_obsolete_playlist_request(&self, playlist_type: &PlaylistFileType) -> bool {
        match playlist_type {
            PlaylistFileType::TopLevelPlaylist => false,
            PlaylistFileType::MediaPlaylist { id, .. } => {
                !self
                    .playlist_store
                    .as_ref()
                    .is_some_and(|store| store.is_current_media_playlist(id))
                    && !self.playlist_refresh_timers.is_following(id)
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
                        // Inspect the safe variant at startup. Later recommendations go straight
                        // to segment selection.
                        if let Some(selection) = self.adaptive_selector.select_variant(
                            &pl_store.compatible_variants(),
                            None,
                            &self.playback_conditions(&pl_store),
                        ) {
                            pl_store.set_variant(selection.safe_variant_id);
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

        self.check_requested_segments_still_needed();
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
    /// Reconsider pending requests when playback conditions change, including seeks and
    /// regular ticks. Both candidates come from the same ABR recommendation.
    pub(super) fn check_requested_segments_still_needed(&mut self) {
        let recommendation = self.compute_optimal_variant();
        for media_type in [MediaType::Audio, MediaType::Video] {
            if !self.requester.has_segment_request_pending(media_type)
                || self.segment_request_contexts.has(|context| {
                    matches!(
                        context,
                        PendingSegmentRequest::Probe {
                            requested_media_type: Some(probe_type),
                            ..
                        } if *probe_type == media_type
                    )
                })
            {
                // Probes establish the metadata needed to resume ordinary selection.
                continue;
            }
            let Some(store) = self.playlist_store.as_ref() else {
                self.abort_segment_requests_with_type(media_type);
                continue;
            };
            let recommendation = segment_selection_for_type(store, recommendation, media_type);
            let Some(addition_candidate) = segment_candidate(
                store,
                recommendation.map(|selection| selection.best_variant_id),
                media_type,
            ) else {
                self.abort_segment_requests_with_type(media_type);
                continue;
            };
            let Some(replacement_candidate) = segment_candidate(
                store,
                recommendation.map(|selection| selection.safe_variant_id),
                media_type,
            ) else {
                self.abort_segment_requests_with_type(media_type);
                continue;
            };
            let selected = self
                .segment_selectors
                .get_mut(media_type)
                .most_needed_segment(
                    addition_candidate,
                    replacement_candidate,
                    self.media_element_ref.inventory(media_type),
                );
            let still_needed = selected.needed_segments().is_some_and(|needed| {
                if let Some(init) = needed.init_segment() {
                    self.requester
                        .is_requesting_segment(media_type, init.url(), init.byte_range())
                } else if let Some(media) = needed.media_segment() {
                    self.requester.is_requesting_segment(
                        media_type,
                        media.url(),
                        media.byte_range(),
                    )
                } else {
                    false
                }
            });
            if !still_needed {
                self.abort_segment_requests_with_type(media_type);
            }
        }
    }

    fn check_segment_to_request_for_type(
        &mut self,
        media_type: MediaType,
        recommendation: Option<AdaptiveVariantSelection>,
    ) {
        if self.requester.has_segment_request_pending(media_type) {
            return;
        }
        let Some(store) = self.playlist_store.as_ref() else {
            return;
        };
        let recommendation = segment_selection_for_type(store, recommendation, media_type);
        let Some(addition_candidate) = segment_candidate(
            store,
            recommendation.map(|selection| selection.best_variant_id),
            media_type,
        ) else {
            return;
        };
        let Some(replacement_candidate) = segment_candidate(
            store,
            recommendation.map(|selection| selection.safe_variant_id),
            media_type,
        ) else {
            return;
        };
        let addition_playlist_id = addition_candidate.context.playlist_id();
        let replacement_playlist_id = replacement_candidate.context.playlist_id();
        let addition_variant_id = addition_candidate.variant_id;
        let addition_context = addition_candidate.context;
        let replacement_context = replacement_candidate.context;
        let selected = self
            .segment_selectors
            .get_mut(media_type)
            .most_needed_segment(
                addition_candidate,
                replacement_candidate,
                self.media_element_ref.inventory(media_type),
            );
        let refresh_id = selected.playlist_to_refresh();
        let Some(needed) = selected.needed_segments() else {
            self.follow_media_playlist(media_type, refresh_id);
            return;
        };
        let variant_id = needed.variant_id();
        let (playlist_id, quality_context) = if variant_id == addition_variant_id {
            (addition_playlist_id, addition_context)
        } else {
            (replacement_playlist_id, replacement_context)
        };

        // Own just the proposed segment metadata so the store can record this choice and
        // check its codecs before a request is scheduled.
        let init = needed.init_segment().cloned();
        let media = needed.media_segment().cloned();
        let init_segment_id = media.as_ref().and_then(|media| {
            store
                .media_playlist(&playlist_id)
                .and_then(|playlist| playlist.segment_list().init_for(media))
                .map(|init| init.id())
        });

        // NOTE: The "active variant" (which is the one enabled in the `PlaylistStore` that we're
        // currently loading segments from) might be different from the "refreshed playlist".
        // This is based on segment-selection heuristics (which may e.g. indicate that we can load a
        // segment of a quality A while waiting for the request of a playlist B).
        let Some(store) = self.playlist_store.as_mut() else {
            return;
        };
        if store.is_variant_locked() && store.current_variant_id() != Some(variant_id) {
            // TODO: A recommendation should not contradict the lock. Report an internal error if
            // this becomes reachable instead of repeatedly skipping segment scheduling.
            log_warn!("Core: Segment selection contradicted the variant lock");
            return;
        }
        // Shared audio does not choose a different video variant. When a playlist really changes,
        // select its whole variant and handle both media types together.
        let choose_variant = media_type == MediaType::Video
            || !store.has_distinct_media_type(MediaType::Video)
            || store.current_media_playlist_id(media_type) != Some(&playlist_id);
        if choose_variant && store.current_variant_id() != Some(variant_id) {
            let changed_media_types = store.set_variant(variant_id);
            for changed_type in changed_media_types {
                self.ready_probe_segments.clear_media_type(changed_type);
                self.abort_segment_requests_with_type(changed_type);
                self.playlist_refresh_timers.forget_media_type(changed_type);
                self.request_current_media_playlist_if_needed(changed_type);
            }
        }
        self.follow_media_playlist(media_type, refresh_id);

        if !self.ready_to_load_segments() {
            return;
        }
        if let Some(init) = init {
            let req_id = self
                .segment_request_contexts
                .insert(PendingSegmentRequest::Init {
                    media_type,
                    init_segment_id: init.id(),
                });
            self.requester.request_init_segment(
                media_type,
                init.url().clone(),
                init.byte_range(),
                req_id,
            );
        } else if let Some(media) = media {
            let req_id = self
                .segment_request_contexts
                .insert(PendingSegmentRequest::Media {
                    variant_id,
                    playlist_id,
                    media_type,
                    init_segment_id,
                    quality_context,
                    sequence_number: media.sequence(),
                    discontinuity_sequence: media.discontinuity_sequence(),
                });
            self.requester
                .request_media_segment(media_type, &media, req_id);
        }
    }

    /// Resume refreshes alongside cached segment requests. A retained timer is also a reload
    /// cooldown: switching candidates must not make us reload the same list too frequently.
    fn follow_media_playlist(&mut self, media_type: MediaType, id: MediaPlaylistPermanentId) {
        self.playlist_refresh_timers.follow(media_type, id);
        let Some(store) = self.playlist_store.as_ref() else {
            return;
        };
        let needs_reload = store
            .media_playlist(&id)
            .is_none_or(|playlist| playlist.may_be_refreshed());
        if !needs_reload || self.playlist_refresh_timers.has_timer(&id) {
            return;
        }
        let Some(url) = store.media_playlist_url(&id) else {
            return;
        };
        let playlist_type = PlaylistFileType::MediaPlaylist { id, media_type };
        if !self.requester.is_requesting_playlist(url, &playlist_type) {
            self.requester.fetch_playlist(url.clone(), playlist_type);
        }
    }

    /// Perform all actions that should be commonly taken after one or multiple of the current Media
    /// Playlists change.
    fn handle_media_playlist_update(
        &mut self,
        changed_media_types: &[MediaType],
        mode: MediaPlaylistUpdateMode,
    ) {
        let updates = changed_media_types
            .iter()
            .copied()
            .map(|media_type| MediaPlaylistUpdate { media_type, mode });
        self.handle_media_playlist_updates(updates);
    }

    /// Apply a batch of Media Playlist updates and schedule segments once after all changes have
    /// been handled.
    fn handle_media_playlist_updates(
        &mut self,
        updates: impl IntoIterator<Item = MediaPlaylistUpdate>,
    ) {
        if self.playlist_store.is_none() {
            return;
        }

        let mut has_updates = false;
        for MediaPlaylistUpdate { media_type, mode } in updates {
            has_updates = true;
            log_info!("Core: {} MediaPlaylist changed", media_type);
            self.ready_probe_segments.clear_media_type(media_type);

            if !matches!(mode, MediaPlaylistUpdateMode::Seamless) {
                self.playlist_refresh_timers.forget_media_type(media_type);
                self.abort_segment_requests_with_type(media_type);
            }
            if matches!(mode, MediaPlaylistUpdateMode::AbortRequestsAndFlushBuffer) {
                self.flush_media_buffer(media_type);
            }
            self.request_current_media_playlist_if_needed(media_type);
        }

        if has_updates {
            self.clean_up_playlist_refresh_timers();
        }
        self.check_segments_to_request();
    }

    fn flush_media_buffer(&mut self, media_type: MediaType) {
        if let Err(e) = self.media_element_ref.flush(media_type) {
            log_warn!(
                "Could not remove data from the previous {media_type} buffer: {}",
                e
            );
        }
        self.segment_selectors
            .get_mut(media_type)
            .restart_from_position(self.media_element_ref.wanted_position() - 0.2);
    }

    fn request_current_media_playlist_if_needed(&mut self, media_type: MediaType) {
        let playlist_to_fetch = self.playlist_store.as_ref().and_then(|pl_store| {
            if pl_store.has_loaded_media_playlist(media_type) {
                None
            } else {
                let id = *pl_store.current_media_playlist_id(media_type)?;
                let url = pl_store.media_playlist_url(&id)?.clone();
                Some((id, url))
            }
        });

        if let Some((id, url)) = playlist_to_fetch {
            use PlaylistFileType::*;
            log_debug!("Core: Media changed, requesting its media playlist");
            let playlist_type = MediaPlaylist { id, media_type };
            if !self.requester.is_requesting_playlist(&url, &playlist_type) {
                self.requester.fetch_playlist(url, playlist_type);
            }
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
                variant_id,
                media_type: req_media_type,
                init_segment_id,
                quality_context,
                sequence_number,
                discontinuity_sequence,
                ..
            } => {
                let time_info = segment_req
                    .time_info()
                    .expect("Media segment requests have timing information")
                    .clone();
                if Some(req_media_type) != media_type {
                    log_warn!("Loaded media segment with mismatched media type context.");
                }
                self.announce_loaded_variant(variant_id, req_media_type);
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

    /// Report the loaded video variant, or the loaded audio variant for audio-only content.
    /// Shared audio cannot identify a video variant and must not change its announcement.
    fn announce_loaded_variant(&mut self, variant_id: u32, media_type: MediaType) {
        if media_type == MediaType::Audio
            && self
                .playlist_store
                .as_ref()
                .is_some_and(|store| store.has_distinct_media_type(MediaType::Video))
        {
            return;
        }
        if self.loaded_variant_id != Some(variant_id) {
            self.loaded_variant_id = Some(variant_id);
            jsAnnounceVariantUpdate(Some(variant_id));
        }
    }

    fn on_media_segment_loaded(&mut self, push_md: SegmentPushMetadata) {
        let segment_start = push_md.time_info.start();
        let segment_end = push_md.time_info.end();
        let media_type = push_md.media_type;
        let playlist_id = push_md.context.playlist_id();
        let identity =
            SegmentIdentity::new(push_md.sequence_number, push_md.discontinuity_sequence);
        let prepared_data = self
            .media_element_ref
            .announce_incoming_media_segment(push_md);

        // Check next segment BEFORE actually pushing, as the pushing operation could take in the
        // tens of ms or even in the hundreds depending on segment size and platform performance.
        //
        // We still announce the incoming segment first to ensure the `MediaElementReference`'s
        // inventory is up-to-date.
        self.segment_selectors.get_mut(media_type).validate_media(
            playlist_id,
            identity,
            segment_end,
        );
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

    /// Remove timers for unknown playlists. Inactive playlists keep their timer as a cooldown;
    /// when it expires, the callback only fetches playlists that are still being followed.
    fn clean_up_playlist_refresh_timers(&mut self) {
        if let Some(ref pl_store) = self.playlist_store {
            self.playlist_refresh_timers
                .retain(|id| pl_store.media_playlist_url(id).is_some())
        } else {
            self.playlist_refresh_timers.clear_all_timers();
        }
    }
}

fn segment_candidate(
    store: &PlaylistStore,
    variant_id: Option<u32>,
    media_type: MediaType,
) -> Option<SegmentSelectionCandidate<'_>> {
    if let Some(variant_id) = variant_id {
        let id = store.media_playlist_id_for_variant(variant_id, media_type)?;
        let context = store.variant_segment_quality_context(variant_id, media_type)?;
        Some(SegmentSelectionCandidate {
            variant_id,
            context,
            playlist: store
                .media_playlist(&id)
                .map(|playlist| playlist.segment_list()),
        })
    } else {
        store.direct_media_playlist()?;
        let (list, context) = store.loaded_media_playlist_segment_info(media_type)?;
        Some(SegmentSelectionCandidate {
            variant_id: crate::bindings::formatters::DIRECT_MEDIA_VARIANT_ID,
            context,
            playlist: Some(list),
        })
    }
}

/// Use safe for additions too when best would require changing the other media playlist.
/// Safe can serve both purposes; best is only suitable for additions.
fn segment_selection_for_type(
    store: &PlaylistStore,
    recommendation: Option<AdaptiveVariantSelection>,
    media_type: MediaType,
) -> Option<AdaptiveVariantSelection> {
    let mut selection = recommendation?;
    let other_type = match media_type {
        MediaType::Video => MediaType::Audio,
        MediaType::Audio => MediaType::Video,
    };
    if store.media_playlist_id_for_variant(selection.best_variant_id, other_type)
        != store.media_playlist_id_for_variant(selection.safe_variant_id, other_type)
    {
        selection.best_variant_id = selection.safe_variant_id;
    }
    Some(selection)
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
    use super::{segment_candidate, segment_selection_for_type, Dispatcher};
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

    #[test]
    fn additions_use_safe_when_the_other_playlist_differs() {
        for shared_audio in [false, true] {
            let high_group = if shared_audio { "low" } else { "high" };
            let manifest = format!(
                "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"low\",NAME=\"English\",DEFAULT=YES,URI=\"low-audio.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"high\",NAME=\"English\",DEFAULT=YES,URI=\"high-audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"low\"\nlow-video.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"{high_group}\"\nhigh-video.m3u8\n"
            );
            let mut store = selected_playlist(manifest.as_bytes());
            let low = store.available_variants()[0].id();
            let high = store.available_variants()[1].id();
            let recommendation = Some(crate::adaptive::AdaptiveVariantSelection {
                best_variant_id: high,
                safe_variant_id: low,
            });
            let video_selection =
                segment_selection_for_type(&store, recommendation, MediaType::Video).unwrap();
            assert_eq!(video_selection.safe_variant_id, low);
            assert_eq!(
                video_selection.best_variant_id,
                if shared_audio { high } else { low }
            );
            let safe = segment_candidate(
                &store,
                Some(video_selection.safe_variant_id),
                MediaType::Video,
            )
            .unwrap();
            // A missing playlist stays eligible and can be refreshed by the caller.
            assert!(safe.playlist.is_none());
            let audio_selection =
                segment_selection_for_type(&store, recommendation, MediaType::Audio).unwrap();
            assert_eq!(audio_selection.safe_variant_id, low);
            assert_eq!(audio_selection.best_variant_id, low);
            let old_audio = *store.current_media_playlist_id(MediaType::Audio).unwrap();
            let changed = store.set_variant(high);
            assert_eq!(store.current_variant_id(), Some(high));
            assert!(changed.contains(&MediaType::Video));
            assert_eq!(changed.contains(&MediaType::Audio), !shared_audio);
            assert_eq!(
                store.current_media_playlist_id(MediaType::Audio) == Some(&old_audio),
                shared_audio
            );
        }
    }

    #[test]
    fn shared_audio_completion_does_not_announce_another_video_variant() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 20.);
        let variants = dispatcher
            .playlist_store
            .as_ref()
            .unwrap()
            .available_variants();
        let low = variants[0].id();
        let high = variants[2].id();
        dispatcher.loaded_variant_id = Some(high);
        dispatcher.announce_loaded_variant(low, MediaType::Audio);
        assert_eq!(dispatcher.loaded_variant_id, Some(high));
    }

    fn dispatcher_with_buffer_observation(
        audio: Option<Vec<f64>>,
        video: Option<Vec<f64>>,
    ) -> Dispatcher {
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
                audio.map(JsTimeRanges::new),
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
                let id = *pl_store
                    .current_media_playlist_id(MediaType::Video)
                    .unwrap();
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
        for variant_id in &variant_ids {
            pl_store.assume_variant_supported_for_test(*variant_id);
        }
        pl_store.set_variant(variant_ids[0]);
        let mut dispatcher =
            dispatcher_with_buffer_observation(None, Some(vec![0., 10. + buffer_level]));
        dispatcher.adaptive_selector = crate::adaptive::AdaptiveQualitySelector::new(1_875_000.);
        dispatcher.playlist_store = Some(pl_store);
        dispatcher
    }

    #[test]
    fn refused_variant_choice_does_not_schedule_a_segment() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        let store = dispatcher.playlist_store.as_mut().unwrap();
        let current = store.current_variant_id().unwrap();
        assert_ne!(recommendation.best_variant_id, current);
        assert!(matches!(
            store.lock_variant(current),
            crate::playlist_store::LockVariantResponse::VariantLocked { .. }
        ));
        // A previously computed recommendation must not bypass a newer lock.
        dispatcher.check_segment_to_request_for_type(MediaType::Video, Some(recommendation));
        assert!(!dispatcher
            .requester
            .has_segment_request_pending(MediaType::Video));
        assert!(!dispatcher
            .segment_request_contexts
            .has(|context| context.is_media() || context.is_init()));
        assert_eq!(
            dispatcher
                .playlist_store
                .as_ref()
                .unwrap()
                .current_variant_id(),
            Some(current)
        );
    }

    #[test]
    fn missing_recommendation_does_not_fall_back_to_a_stored_variant() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let current = dispatcher
            .playlist_store
            .as_ref()
            .unwrap()
            .current_variant_id();
        dispatcher.check_segment_to_request_for_type(MediaType::Video, None);
        assert!(!dispatcher
            .requester
            .has_segment_request_pending(MediaType::Video));
        assert_eq!(
            dispatcher
                .playlist_store
                .as_ref()
                .unwrap()
                .current_variant_id(),
            current
        );
    }

    #[test]
    fn recommendations_do_not_change_the_current_choice() {
        let dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let store = dispatcher.playlist_store.as_ref().unwrap();
        let current = store.current_variant_id().unwrap();
        let playlist_id = *store.current_media_playlist_id(MediaType::Video).unwrap();
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        assert_ne!(recommendation.best_variant_id, current);
        assert_eq!(store.current_variant_id(), Some(current));
        assert_eq!(
            store.current_media_playlist_id(MediaType::Video),
            Some(&playlist_id)
        );
    }

    #[test]
    fn scheduling_and_pending_checks_use_the_recommended_best() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        let store = dispatcher.playlist_store.as_mut().unwrap();
        let previous_variant = store.current_variant_id().unwrap();
        let best_id = store
            .media_playlist_id_for_variant(recommendation.best_variant_id, MediaType::Video)
            .unwrap();
        store
            .update_media_playlist(
                &best_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nbest-seg.ts\n#EXT-X-ENDLIST\n"
                    .as_slice(),
                Url::new("https://example.com/best.m3u8".into()),
            )
            .unwrap();
        let expected_url = Url::new("https://example.com/best-seg.ts".into());
        dispatcher.requester.lock_segment_requests();
        dispatcher.check_segment_to_request_for_type(MediaType::Video, Some(recommendation));
        assert!(dispatcher
            .requester
            .is_requesting_segment(MediaType::Video, &expected_url, None));
        assert_eq!(
            dispatcher
                .playlist_store
                .as_ref()
                .unwrap()
                .current_variant_id(),
            Some(recommendation.best_variant_id)
        );
        // Scheduling a request must not announce its variant as loaded.
        assert_eq!(dispatcher.loaded_variant_id, None);

        // A pending check must use the ABR pair, even if the stored choice differs.
        dispatcher
            .playlist_store
            .as_mut()
            .unwrap()
            .set_variant(previous_variant);
        dispatcher.check_requested_segments_still_needed();
        assert!(dispatcher
            .requester
            .is_requesting_segment(MediaType::Video, &expected_url, None));
        assert_eq!(
            dispatcher
                .playlist_store
                .as_ref()
                .unwrap()
                .current_variant_id(),
            Some(previous_variant)
        );
    }

    #[test]
    fn safe_candidate_keeps_its_playlist_and_init_while_best_is_selected() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let store = dispatcher.playlist_store.as_mut().unwrap();
        let ids: Vec<_> = store
            .available_variants()
            .iter()
            .map(|variant| variant.id())
            .collect();
        let safe_variant = ids[1];
        let best_variant = ids[2];
        let safe_id = store
            .media_playlist_id_for_variant(safe_variant, MediaType::Video)
            .unwrap();
        let mut text =
            String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"safe-init.mp4\"\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nsegment-{number}.m4s\n"));
        }
        store
            .update_media_playlist(
                &safe_id,
                text.as_bytes(),
                Url::new("https://example.com/safe.m3u8".into()),
            )
            .unwrap();
        store.set_variant(best_variant);
        let best = segment_candidate(store, Some(best_variant), MediaType::Video).unwrap();
        let safe = segment_candidate(store, Some(safe_variant), MediaType::Video).unwrap();
        let low_context = store
            .variant_segment_quality_context(ids[0], MediaType::Video)
            .unwrap();
        let inventory: Vec<_> = (0..6)
            .map(|number| {
                crate::media_element::BufferedChunk::new_for_test(
                    number as f64 * 4.,
                    (number + 1) as f64 * 4.,
                    low_context,
                )
            })
            .collect();
        let selected = dispatcher
            .segment_selectors
            .get_mut(MediaType::Video)
            .most_needed_segment(best, safe, &inventory);
        let needed = selected.needed_segments().unwrap();
        assert_eq!(needed.variant_id(), safe_variant);
        assert_eq!(needed.media_segment().unwrap().start(), 8.);
        assert_eq!(
            needed.init_segment().unwrap().url().get_ref(),
            "https://example.com/safe-init.mp4"
        );
        assert_eq!(
            dispatcher
                .playlist_store
                .as_ref()
                .unwrap()
                .current_variant_id(),
            Some(best_variant)
        );
    }

    #[test]
    fn followed_safe_playlist_response_is_not_obsolete() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let store = dispatcher.playlist_store.as_ref().unwrap();
        let safe_variant = store.available_variants()[1].id();
        let id = store
            .media_playlist_id_for_variant(safe_variant, MediaType::Video)
            .unwrap();
        let request = crate::requester::PlaylistFileType::MediaPlaylist {
            id,
            media_type: MediaType::Video,
        };
        assert!(dispatcher.is_obsolete_playlist_request(&request));
        dispatcher
            .playlist_refresh_timers
            .follow(MediaType::Video, id);
        assert!(!dispatcher.is_obsolete_playlist_request(&request));
    }

    #[test]
    fn segment_failure_checks_its_own_playlist_after_best_changes() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let store = dispatcher.playlist_store.as_mut().unwrap();
        let ids: Vec<_> = store
            .available_variants()
            .iter()
            .map(|variant| variant.id())
            .collect();
        let safe_id = store
            .media_playlist_id_for_variant(ids[1], MediaType::Video)
            .unwrap();
        store.update_media_playlist(&safe_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:100\n#EXTINF:4,\nsegment-100.ts\n".as_slice(),
            Url::new("https://example.com/safe.m3u8".into())).unwrap();
        store.set_variant(ids[2]);
        let context = crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
            variant_id: ids[1],
            playlist_id: safe_id,
            media_type: MediaType::Video,
            init_segment_id: None,
            quality_context: store
                .variant_segment_quality_context(ids[1], MediaType::Video)
                .unwrap(),
            sequence_number: 100,
            discontinuity_sequence: 0,
        };
        assert!(!crate::dispatcher::utils::is_stale_segment_request_context(
            Some(store),
            &context
        ));
        store.update_media_playlist(&safe_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:101\n#EXTINF:4,\nsegment-101.ts\n".as_slice(),
            Url::new("https://example.com/safe.m3u8".into())).unwrap();
        assert!(crate::dispatcher::utils::is_stale_segment_request_context(
            Some(store),
            &context
        ));
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
            let selection = dispatcher.compute_optimal_variant().unwrap();
            let variant_id = selection.best_variant_id;
            assert_eq!(variant_id, high_id);
            assert_ne!(variant_id, selection.safe_variant_id);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.set_variant(variant_id);
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
            let variant_id = dispatcher
                .compute_optimal_variant()
                .unwrap()
                .best_variant_id;
            assert_eq!(variant_id, medium_id);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.set_variant(variant_id);
            assert_eq!(pl_store.segment_target_duration(), Some(4.));
            assert_eq!(pl_store.max_observed_target_duration(), Some(6.));
        }
    }

    #[test]
    fn playback_conditions_use_minimum_known_required_buffer() {
        for (manifest, audio, expected_buffer_level) in [
            ("#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"aud\"\nvideo.m3u8\n", Some(vec![0., 15.]), 5.),
            ("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\"\nmuxed.m3u8\n", None, 30.),
        ] {
            let pl_store = selected_playlist(manifest.as_bytes());
            let dispatcher = dispatcher_with_buffer_observation(audio, Some(vec![0., 40.]));

            assert_eq!(dispatcher.playback_conditions(&pl_store).buffer_level, Some(expected_buffer_level));
        }
    }

    #[test]
    fn playback_conditions_use_audio_source_buffer_for_audio_only_variants() {
        let pl_store = selected_playlist(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"mp4a.40.2\"\naudio.m3u8\n",
        );
        let dispatcher = dispatcher_with_buffer_observation(Some(vec![0., 15.]), None);

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
            let dispatcher = dispatcher_with_buffer_observation(None, video);

            assert_eq!(
                dispatcher.playback_conditions(&pl_store).buffer_level,
                expected
            );
        }
    }
}
