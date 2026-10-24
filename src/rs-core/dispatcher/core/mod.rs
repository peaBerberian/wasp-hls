use super::{
    event_listeners::JsTimeRanges, utils, Dispatcher, JsMemoryBlob, MediaObservation,
    MediaSourceReadyState, PlaybackTickReason, PlayerReadyState, ReadyProbeSegment,
};
use crate::{
    adaptive::{should_interrupt_download, AdaptiveVariantSelection, PlaybackConditions},
    bindings::{
        formatters::{format_source_buffer_creation_err_for_js, DIRECT_MEDIA_VARIANT_ID},
        jsAnnounceTrackUpdate, jsAnnounceVariantLockStatusChange, jsAnnounceVariantUpdate,
        jsInspectSegment, jsRequestPlaybackObservation, jsSendMediaPlaylistParsingError,
        jsSendMediaPlaylistRequestError, jsSendMultivariantPlaylistRequestError, jsSendOtherError,
        jsSendPushedSegmentError, jsSendRemoveBufferError, jsSendSegmentParsingError,
        jsSendSegmentRequestError, jsSendSourceBufferCreationError, jsSendTrackSelectionError,
        jsSetMediaSourceDuration, jsStopObservingPlayback, jsUpdateContentInfo,
        AddSourceBufferErrorCode, MediaType, OtherErrorCode, PushedSegmentErrorCode, RequestId,
        SourceBufferId, TimerId, TrackSelectionErrorCode,
    },
    dispatcher::segment_request_contexts::PendingSegmentRequest,
    media_element::{SegmentPushMetadata, SegmentQualityContext},
    parser::{ExternalMediaInfo, PlaylistRefreshInfo, SegmentIdentity},
    playlist_store::{
        IncompatibleTrack, LockVariantResponse, MediaPlaylistPermanentId, PlaylistStore,
        ProbeSegmentMetadata, SelectionChanges, TrackChange, TrackSelectionError,
        VariantLockChange,
    },
    requester::{
        FinishedRequestType, PlaylistFileType, PlaylistRequestInfo, RetryResult, SegmentRequestInfo,
    },
    segment_selector::SegmentSelectionCandidate,
    utils::{logger::*, url::Url},
};

mod startup;

// Select slightly before the playhead to tolerate segment timing differences. Pending-request
// checks must use the same margin, or they can cancel the segment selection just requested.
const SEGMENT_SELECTION_MARGIN: f64 = 0.2;

impl Dispatcher {
    // XXX TODO: Schedule text segments using the shared buffer coverage interface.
    // Count successfully installed empty segments as covered; refresh live playlists, restart
    // on seeks, and invalidate requests/cue batches on selection generation changes.
    // Fetch ahead of playback, never wait for cues to become active before sending them.
    // Subtitle errors/readiness/end must not gate A/V playback.

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
        self.update_min_buffer_time();
        self.ready_probe_segments.clear();
        self.initial_audio_track_selection.clear();
        self.initial_subtitle_track_selection.clear();
        self.last_position = 0.;
        self.playlist_refresh_manager.clear();
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Begin "locking" HLS variant whose `id` is given in argument, meaning that we will keep only
    /// playing that one.
    ///
    /// Suffixed by `_core` because this is not the public API, but should be what the
    /// `lock_variant` API actually calls for its core logic.
    pub(super) fn lock_variant_core(&mut self, variant_id: u32) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            match pl_store.lock_variant(variant_id) {
                LockVariantResponse::AlreadyLocked => {}
                LockVariantResponse::NoVariantWithId => {
                    log_warn!("Core: Locked variant not found");
                    jsSendOtherError(
                        false,
                        OtherErrorCode::UnfoundLockedVariant,
                        &format!("Wanted locked variant \"{variant_id}\" not found"),
                    );
                }
                LockVariantResponse::VariantLocked(changes) => {
                    // XXX TODO: needs_variant_selection does not seem handled
                    // here.
                    // I know currently it would serve no role but dispatcher does not know that...
                    // Or else the return type chosen in PlaylistStore is completely wrong
                    self.announce_track_changes(&changes);
                    if changes.variant_changed {
                        self.announce_current_variant();
                    }
                    self.announce_variant_lock_change(changes.variant_lock_change);
                    let changed_media_types = changes.changed_playlists;
                    if changed_media_types.contains(&MediaType::Text) {
                        self.reset_subtitle_playlist();
                    }

                    // Stop and clean all resources linked to media types whose playlist changed.
                    for media_type in changed_media_types.iter().copied() {
                        log_info!("Core: {} MediaPlaylist changed on variant lock", media_type);
                        self.ready_probe_segments.clear_media_type(media_type);
                        self.abort_segment_requests_with_type(media_type);
                        self.flush_media_buffer(media_type);
                    }

                    // Now iterate on unchanged media types which are in the buffer.
                    // On variant lock, we just do a complete buffer flush today regardless.
                    //
                    // TODO: Avoid flushing an unchanged media type when its buffer already only
                    // contains data from the newly-locked variant. An unchanged playlist
                    // may still have older-quality buffered data, but when it does not this causes
                    // an unnecessary rebuffer and refetch.
                    for media_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
                        if self.media_element_ref.has_buffer(media_type)
                            && !changed_media_types.contains(&media_type)
                        {
                            self.abort_segment_requests_with_type(media_type);
                            self.flush_media_buffer(media_type);
                        }
                    }

                    for media_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
                        self.follow_selected_media_playlist(media_type);
                    }

                    self.check_segments_to_request();
                }
            }
        }
    }

    /// Remove an HLS variant previously put in place through `lock_variant_core`.
    pub(super) fn unlock_variant_core(&mut self) {
        if let Some(pl_store) = self.playlist_store.as_mut() {
            let changes = pl_store.unlock_variant();
            // XXX TODO: We may also want to call other change anouncement API here?
            // I know currently it would serve no role but dispatcher does not know that...
            // Or else the return type chosen in PlaylistStore is completely wrong
            self.announce_variant_lock_change(changes.variant_lock_change);
            self.check_requested_segments_still_needed();
            self.check_segments_to_request();
        }
    }

    /// Method to call once a timer for Playlist refresh, started with the jsTimer JavaScript
    /// function, has finished, with the corrsonding `TimerId` as argument.
    pub(super) fn on_playlist_refresh_timer_ended(&mut self, id: TimerId) {
        let Some((playlist_id, media_type)) = self.playlist_refresh_manager.on_timer_ended(id)
        else {
            return;
        };
        self.follow_media_playlist(media_type, playlist_id);
    }

    /// Method to call once a timer for retrying a request, started with the jsTimer JavaScript
    /// function, has finished, with the corresponding `TimerId` as argument.
    // TODO: Before retrying a media playlist, discard it if it became neither selected nor
    // followed during backoff. Failure handling already makes that check.
    pub(super) fn on_retry_request(&mut self, id: TimerId) {
        if let Some(media_type) = self.requester.media_type_for_timer(id) {
            if !self.check_requested_segment_still_needed(media_type) {
                // NOTE: `check_requested_segment_still_needed` already takes care of removing
                // the now unneeded resource from the `requester`
                self.check_segments_to_request();
                return;
            }
        }
        self.requester.on_timer_finished(id);
    }

    /// Set an audio track whose `id` is given in argument.
    /// XXX TODO: That method changes is impossible to review manually.
    /// Ensure **NOTHING** changed in terms of behavior here
    pub(super) fn set_audio_track_core(&mut self, track_id: Option<u32>) {
        let Some(pl_store) = self.playlist_store.as_mut() else {
            return;
        };
        let changes = match pl_store.set_audio_track(track_id) {
            Ok(changes) => changes,
            Err(reason) => {
                if let Some(track_id) = track_id {
                    let (code, message) = match reason {
                        TrackSelectionError::UnknownTrack => (
                            TrackSelectionErrorCode::UnknownTrack,
                            "Unable to select the audio track: unknown track ID",
                        ),
                        TrackSelectionError::NoCompatibleVariant => (
                            TrackSelectionErrorCode::NoCompatibleVariant,
                            "Unable to select the audio track: no compatible variant",
                        ),
                    };
                    jsSendTrackSelectionError(code, MediaType::Audio, track_id, message);
                }
                return;
            }
        };
        if changes.needs_variant_selection || changes.changed_playlists.contains(&MediaType::Audio)
        {
            // Preserve the existing choice of a final variant before requesting playlists.
            let variant_changes = self
                .compute_optimal_variant()
                .and_then(|selection| {
                    self.playlist_store
                        .as_mut()
                        .map(|store| store.set_variant(selection.safe_variant_id))
                })
                .unwrap_or_default();
            self.announce_final_track_changes(&changes, &variant_changes);
            self.announce_current_variant();
            self.announce_variant_lock_change(changes.variant_lock_change);
            let mut changed_media_types = variant_changes.changed_playlists;
            // An explicit audio switch still flushes the old audio buffer.
            if !changed_media_types.contains(&MediaType::Audio) {
                changed_media_types.push(MediaType::Audio);
            }
            let text_changed = changes.changed_playlists.contains(&MediaType::Text)
                || changed_media_types.contains(&MediaType::Text);
            if text_changed {
                self.reset_subtitle_playlist();
            }
            self.reset_on_selected_media_playlists(&changed_media_types);
        } else {
            self.announce_track_changes(&changes);
            if changes.changed_playlists.contains(&MediaType::Text) {
                self.reset_subtitle_playlist();
            }
            self.check_requested_segments_still_needed();
            self.check_segments_to_request();
        }
    }

    fn announce_variant_lock_change(&self, change: VariantLockChange) {
        match change {
            VariantLockChange::Unchanged => {}
            VariantLockChange::Locked(id) => jsAnnounceVariantLockStatusChange(Some(id)),
            VariantLockChange::Unlocked => jsAnnounceVariantLockStatusChange(None),
        }
    }

    fn announce_track_changes(&self, changes: &SelectionChanges) {
        if changes.audio_track_change != TrackChange::Unchanged {
            self.announce_current_audio_track();
        }
        if changes.subtitle_track_change != TrackChange::Unchanged {
            self.announce_current_subtitle_track();
        }
    }

    // XXX TODO: Impossible to understand, compared to announce_track_changes
    fn announce_final_track_changes(&self, choice: &SelectionChanges, variant: &SelectionChanges) {
        if choice.audio_track_change != TrackChange::Unchanged
            || variant.audio_track_change != TrackChange::Unchanged
        {
            self.announce_current_audio_track();
        }
        if choice.subtitle_track_change != TrackChange::Unchanged
            || variant.subtitle_track_change != TrackChange::Unchanged
        {
            self.announce_current_subtitle_track();
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
        // TODO: Also recheck whether a failed segment is still needed; if obsolete, report its
        // failure as nonfatal and skip retrying it/failing on it.
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
                        // We may be on a startup phase here, restart checking it
                        self.recheck_player_state();
                    } else {
                        // TODO: perhaps we could recheck player state in all cases to simplify the
                        // code? I mean here in **all** check_segments cases.
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
                        // TODO: A playlist followed only to inspect replacements can fail while
                        // additions and buffered media remain playable. Keep that failure
                        // nonfatal instead of stopping playback solely because it is followed.
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
        // More playable data does not necessarily cause a media-element event.
        // Request an observation now if it means we could go out of rebuffering faster
        if self.media_element_ref.is_rebuffering() {
            jsRequestPlaybackObservation();
        }
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
                                .restart_from_position(wanted_pos - SEGMENT_SELECTION_MARGIN);
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
        // TODO: Group rebuffering and end-of-stream decisions with freezing
        // detection and possibly gap jumping? Rebuffering currently uses the
        // previous selection result until the next observation.
        self.media_element_ref.on_observation(observation);
        for media_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
            if let Some(buffered) = self.media_element_ref.buffered_ranges_for(media_type) {
                self.segment_selectors
                    .get_mut(media_type)
                    .check_for_removed_data(&buffered);
            }
        }
        match reason {
            PlaybackTickReason::Seeking => self.on_seek(),
            _ => self.on_regular_tick(),
        }
        self.media_element_ref.check_end_of_stream();
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
        for media_type in [MediaType::Video, MediaType::Audio, MediaType::Text] {
            self.schedule_next_segment_for_type(media_type, recommendation);
        }

        // Selection can change while waiting for codecs, without scheduling a segment.
        // The API already filters unchanged variant announcements.
        self.announce_current_variant();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    pub(super) fn set_subtitle_track_core(&mut self, track_id: Option<u32>) {
        let Some(pl_store) = self.playlist_store.as_mut() else {
            return;
        };
        let changes = match pl_store.set_subtitle_track(track_id) {
            Ok(changes) => changes,
            Err(reason) => {
                let (code, message) = match reason {
                    TrackSelectionError::UnknownTrack => (
                        TrackSelectionErrorCode::UnknownTrack,
                        "Unable to select the subtitle track: unknown track ID",
                    ),
                    TrackSelectionError::NoCompatibleVariant => (
                        TrackSelectionErrorCode::NoCompatibleVariant,
                        "Unable to select the subtitle track: no compatible variant",
                    ),
                };
                if let Some(track_id) = track_id {
                    jsSendTrackSelectionError(code, MediaType::Text, track_id, message);
                }
                return;
            }
        };
        let mut variant_changes = SelectionChanges::default();
        if changes.needs_variant_selection {
            if let Some(selection) = self.compute_optimal_variant() {
                let Some(pl_store) = self.playlist_store.as_mut() else {
                    return;
                };
                variant_changes = pl_store.set_variant(selection.safe_variant_id);
            }
        }
        self.announce_final_track_changes(&changes, &variant_changes);
        if changes.needs_variant_selection {
            self.announce_current_variant();
            self.announce_variant_lock_change(changes.variant_lock_change);
        }
        if changes.changed_playlists.contains(&MediaType::Text)
            || variant_changes.changed_playlists.contains(&MediaType::Text)
        {
            self.reset_subtitle_playlist();
        }
        self.reset_on_selected_media_playlists(&variant_changes.changed_playlists);
        self.check_requested_segments_still_needed();
        self.check_segments_to_request();
    }

    pub(super) fn disable_subtitle_track_core(&mut self) {
        let Some(pl_store) = self.playlist_store.as_mut() else {
            return;
        };
        let changes = pl_store.disable_subtitle_track();
        // XXX TODO: We may also want to call other change anouncement API here?
        // I know currently it would serve no role but dispatcher does not know that...
        // Or else the return type chosen in PlaylistStore is completely wrong
        self.announce_track_changes(&changes);
        if changes.changed_playlists.contains(&MediaType::Text) {
            self.reset_subtitle_playlist();
        }
        self.check_segments_to_request();
    }

    /// Discard work belonging to the previous subtitle playlist.
    // XXX TODO: I have no idea what this is for. If audio doesn't have this,
    // we probably don't want it for subtitles
    fn reset_subtitle_playlist(&mut self) {
        self.abort_segment_requests_with_type(MediaType::Text);
        self.requester.abort_playlists_with_type(MediaType::Text);
        self.playlist_refresh_manager
            .forget_media_type(MediaType::Text);
        self.ready_probe_segments.clear_media_type(MediaType::Text);
        self.segment_selectors
            .get_mut(MediaType::Text)
            .restart_from_position(
                self.media_element_ref.wanted_position() - SEGMENT_SELECTION_MARGIN,
            );
        // XXX TODO: Reset retained text and renderer cues for the new selection generation.
    }

    /// Send `trackUpdate` for the selected subtitle track, as for audio.
    fn announce_current_subtitle_track(&self) {
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        jsAnnounceTrackUpdate(MediaType::Text, pl_store.current_subtitle_track_id());
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
        let compatible_variants = pl_store.compatible_variants().collect::<Vec<_>>();
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
        // NOTE: We don't include text here on purpose. It's buffer gap matters less than
        // Audio/Video
        let buffer_level = [MediaType::Audio, MediaType::Video]
            .into_iter()
            .filter_map(|media_type| self.media_element_ref.buffer_ahead_for(media_type))
            .reduce(f64::min);
        PlaybackConditions {
            buffer_level,
            buffer_goal: self.buffer_goal,
            playback_speed: self.media_element_ref.wanted_speed(),
            abr_reference_segment_duration: [MediaType::Audio, MediaType::Video]
                .into_iter()
                .filter_map(|media_type| pl_store.max_observed_target_duration(media_type))
                .reduce(f64::max),
        }
    }

    /// Once a "probe segment" has been loaded, it needs to be inspected so information
    /// can be extracted from it and the state be updated accordingly.
    ///
    /// This is what this method does: inspect the segment and update the playlist state
    /// accordingly.
    fn on_probe_segment_loaded(
        &mut self,
        probe_segment: ProbeSegmentMetadata,
        requested_media_type: Option<MediaType>,
        data: JsMemoryBlob,
    ) {
        // XXX TODO: Find out what to do with text probe segments
        let Some(pl_store) = self.playlist_store.as_mut() else {
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
        let media_info = ExternalMediaInfo {
            mime_type: inspection.mime_type,
            media_type: inspection.media_type,
            codec: inspection.codec,
        };
        let changes = pl_store.set_external_media_info(media_info, considered_media_type);

        // That might have led to more timing-related information
        jsUpdateContentInfo(
            utils::estimated_playback_minimum_position(pl_store),
            utils::estimated_playback_maximum_position(pl_store),
            pl_store.playlist_type(),
            pl_store.is_finalized(),
            pl_store.uses_program_date_time(),
        );
        sync_media_source_duration(pl_store);
        for media_type in changes.changed_playlists {
            self.follow_selected_media_playlist(media_type);
        }

        // Probe segment may have provided information helping us to determine how much to
        // buffer (e.g. media type)
        self.update_min_buffer_time();

        self.ready_probe_segments.insert(ReadyProbeSegment {
            request: probe_segment,
            media_type: considered_media_type,
            data,
        });
        self.recheck_player_state();
    }

    /// Called when a playlist request succeeds.
    ///
    /// # Arguments
    ///
    /// * `pl_info` - Request details, including the URL before redirects.
    /// * `data` - Downloaded playlist bytes.
    /// * `final_url` - Response URL after redirects. Use it to resolve relative URLs.
    fn on_playlist_fetch_success(
        &mut self,
        pl_info: PlaylistRequestInfo,
        data: Vec<u8>,
        final_url: Url,
    ) {
        let is_obsolete = self.is_obsolete_playlist_request(&pl_info.playlist_type);
        let PlaylistRequestInfo {
            playlist_type,
            started_at,
            ..
        } = pl_info;
        match playlist_type {
            PlaylistFileType::TopLevelPlaylist => {
                self.on_top_level_playlist_loaded(data, final_url, started_at)
            }

            PlaylistFileType::MediaPlaylist { id, media_type } => {
                // TODO: extract to method
                log_info!(
                    "Media playlist loaded successfully: {}",
                    final_url.get_ref(),
                );
                let refresh_info = {
                    let Some(pl_store) = self.playlist_store.as_mut() else {
                        jsSendOtherError(
                            true,
                            OtherErrorCode::Unknown,
                            "Media playlist loaded but no top-level playlist",
                        );
                        self.stop_current_content();
                        return;
                    };
                    match pl_store.update_media_playlist(&id, data.as_ref(), final_url) {
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
                        Ok(p) => p.refresh_info(),
                    }
                };
                self.process_parsed_media_playlist(id, refresh_info, started_at);
            }
        }
    }

    /// A media playlist response is obsolete only when its permanent ID is no longer
    /// selected. The same playlist may still be selected through another variant.
    /// TODO: See if just communicating a MediaPlaylistPermanentId could work
    fn is_obsolete_playlist_request(&self, playlist_type: &PlaylistFileType) -> bool {
        match playlist_type {
            PlaylistFileType::TopLevelPlaylist => false,
            PlaylistFileType::MediaPlaylist { id, .. } => {
                let is_current_or_followed = self
                    .playlist_store
                    .as_ref()
                    .is_some_and(|pl_store| pl_store.is_current_media_playlist(id))
                    || self.playlist_refresh_manager.is_following(id);
                !is_current_or_followed
            }
        }
    }

    /// Parse the top-level playlist using its final response URL.
    ///
    /// # Arguments
    ///
    /// * `data` - Downloaded playlist bytes.
    /// * `playlist_url` - Response URL after redirects. Use it to resolve relative URLs.
    /// * `started_at` - Time the request started, in monotonic milliseconds.
    fn on_top_level_playlist_loaded(&mut self, data: Vec<u8>, playlist_url: Url, started_at: f64) {
        log_info!("Core: top-level playlist loaded");
        match PlaylistStore::try_new(data.as_ref(), playlist_url) {
            Ok(mut pl_store) => {
                log_info!("Core: top-level playlist parsed successfully");
                self.apply_initial_audio_track_selection(&mut pl_store);
                self.apply_initial_subtitle_track_selection(&mut pl_store);
                // Inspect the safe variant at startup. Later recommendations go straight
                // to segment selection.
                if let Some(selection) = self.adaptive_selector.select_variant(
                    &pl_store.compatible_variants().collect::<Vec<_>>(),
                    None,
                    &self.playback_conditions(&pl_store),
                ) {
                    pl_store.set_variant(selection.safe_variant_id);
                }

                // A top-level playlist may be a media playlist directly.
                let direct_media_refresh = pl_store
                    .direct_media_playlist()
                    .map(|(id, playlist)| (*id, playlist.refresh_info()));

                self.playlist_store = Some(pl_store);
                if let Some((playlist_id, refresh_info)) = direct_media_refresh {
                    self.process_parsed_media_playlist(playlist_id, refresh_info, started_at);
                }
                self.recheck_player_state();
            }
            Err(err) => {
                utils::handle_playlist_store_error(err);
                self.stop_current_content();
            }
        }
    }

    fn apply_initial_audio_track_selection(&mut self, pl_store: &mut PlaylistStore) {
        let initial_audio_track_selection = std::mem::take(&mut self.initial_audio_track_selection);
        for selection in initial_audio_track_selection {
            let matching_ids: Vec<_> = pl_store
                .audio_tracks()
                .iter()
                .filter(|track| selection.matches(track))
                .map(|track| track.id())
                .collect();

            // In all matching, keep the first actually possible one
            for id in matching_ids {
                if pl_store.set_audio_track(Some(id)).is_ok() {
                    return;
                }
            }
        }
    }

    fn apply_initial_subtitle_track_selection(&mut self, pl_store: &mut PlaylistStore) {
        let selections = std::mem::take(&mut self.initial_subtitle_track_selection);
        for selection in selections {
            let Some(selection) = selection else {
                pl_store.disable_subtitle_track();
                return;
            };
            let matching_ids: Vec<_> = pl_store
                .subtitle_tracks()
                .iter()
                .filter(|track| selection.matches(track))
                .map(|track| track.id())
                .collect();
            for id in matching_ids {
                if pl_store.set_subtitle_track(Some(id)).is_ok() {
                    return;
                }
            }
        }
    }

    /// Keep playback's resume threshold reachable by the segment scheduler.
    pub(super) fn update_min_buffer_time(&mut self) {
        // TODO: account for low-latency playlists.
        let normal_threshold = self
            .playlist_store
            .as_ref()
            .and_then(|pl_store| {
                pl_store
                    .current_media_playlists()
                    .filter(|(media_type, _)| {
                        matches!(media_type, MediaType::Audio | MediaType::Video)
                    })
                    .map(|(_, playlist)| playlist.target_duration())
                    // XXX TODO: Why max?
                    .reduce(f64::max)
            })
            .map(|duration| (duration - 1.).clamp(2., 8.))
            .unwrap_or(5.);
        let min_buffer_time = normal_threshold.min(self.buffer_goal.max(0.));
        log_debug!("Core: Updating min_buffer_time: {min_buffer_time}");
        self.media_element_ref
            .update_min_buffer_time(min_buffer_time);
    }

    /// Method called once a Media Playlist was parsed with success
    fn process_parsed_media_playlist(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        refresh_info: Option<PlaylistRefreshInfo>,
        started_at: f64,
    ) {
        self.playlist_refresh_manager
            .on_playlist_loaded(playlist_id, refresh_info, started_at);

        self.update_min_buffer_time();

        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };

        // That might have led to more timing-related information
        jsUpdateContentInfo(
            utils::estimated_playback_minimum_position(pl_store),
            utils::estimated_playback_maximum_position(pl_store),
            pl_store.playlist_type(),
            pl_store.is_finalized(),
            pl_store.uses_program_date_time(),
        );
        sync_media_source_duration(pl_store);
        self.recheck_player_state();
        self.media_element_ref.check_end_of_stream();
    }

    /// This media player is based on a regular clock.
    ///
    /// That clock can "tick" for various reasons: specific media events like seeking, regular
    /// interval etc.
    ///
    /// That method is intended for the default fallback case, it re-checks the complete
    /// current media state assuming continuous playback.
    fn on_regular_tick(&mut self) {
        let wanted_pos = self.media_element_ref.wanted_position();
        self.last_position = wanted_pos;

        // Lock `Requester`, so it only do new segment requests when every
        // wanted segments is scheduled - for better priorization
        let was_already_locked = self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.segment_selectors
            .advance_position(wanted_pos - SEGMENT_SELECTION_MARGIN);

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
            .restart_from_position(wanted_pos - SEGMENT_SELECTION_MARGIN);

        let was_already_locked = self.requester.lock_segment_requests();
        self.requester.update_base_position(Some(wanted_pos));
        self.check_requested_segments_still_needed();
        self.check_segments_to_request();
        if !was_already_locked {
            self.requester.unlock_segment_requests();
        }
    }

    /// Check that all pending initialization and media requests still correspond to the most
    /// needed segments.
    ///
    /// Always cancel media requests outside the wanted buffer window. For useful requests,
    /// only an urgent adaptive switch may replace a fetch in progress. Queued requests and
    /// requests waiting for retry are reconsidered for every switch.
    ///
    /// Reconsider pending requests when playback conditions change, including seeks and
    /// regular ticks. Both candidates come from the same ABR recommendation.
    pub(super) fn check_requested_segments_still_needed(&mut self) {
        for media_type in [MediaType::Audio, MediaType::Video, MediaType::Text] {
            self.check_requested_segment_still_needed(media_type);
        }
    }

    /// Return whether the pending request survives selection. Reconsider queued requests and
    /// requests waiting for retry even when a switch is nonurgent.
    fn check_requested_segment_still_needed(&mut self, media_type: MediaType) -> bool {
        if !self.requester.has_segment_request_pending(media_type) {
            return false;
        }
        if self.segment_request_contexts.has(|context| {
            matches!(
                context,
                PendingSegmentRequest::Probe {
                    requested_media_type: Some(probe_type),
                    ..
                } if *probe_type == media_type
            )
        }) {
            // Probes establish the metadata needed to resume ordinary selection.
            return true;
        }

        let Some(pl_store) = self.playlist_store.as_ref() else {
            self.abort_segment_requests_with_type(media_type);
            return false;
        };

        // Interrupt requests out of the wanted buffer window
        // TODO: requester method like `retain_requests` or something taking a callback instead?
        let wanted_position = self.media_element_ref.wanted_position();
        if self
            .requester
            .pending_segment_time_info(media_type)
            .is_some_and(|time_info| {
                time_info.end() <= wanted_position - SEGMENT_SELECTION_MARGIN
                    || time_info.start() >= wanted_position + self.buffer_goal.max(0.)
            })
        {
            self.abort_segment_requests_with_type(media_type);
            return false;
        }

        // Direct media has no variant choice; only the range check above applies.
        if pl_store.direct_media_playlist().is_some() {
            return true;
        }

        let Some(recommendation) = self.compute_optimal_variant() else {
            return true;
        };
        // TODO: A slow replacement of the final segment can delay end-of-stream even when
        // lower-quality data already covers the end. We could cancel that optional
        // download near the end, without preventing reloads of data that is actually missing.
        if self.requester.has_segment_in_flight(media_type)
            && !self.should_interrupt_download(media_type, recommendation)
        {
            return true;
        }
        let Some((addition_candidate, replacement_candidate)) =
            segment_selection_candidates(pl_store, Some(recommendation), media_type)
        else {
            return true;
        };

        // Now just check that the current requests are still the ones we want
        let selected = self
            .segment_selectors
            .get_mut(media_type)
            .most_needed_segment(
                addition_candidate,
                replacement_candidate,
                self.media_element_ref.inventory(media_type),
            );
        let Some(needed) = selected.needed_segments() else {
            let refresh_id = selected.playlist_to_refresh();
            let waiting_for_playlist = pl_store.media_playlist(&refresh_id).is_none();
            self.follow_media_playlist(media_type, refresh_id);
            // Keep useful progress while loading the target playlist. Once it is loaded,
            // an urgent switch can cancel even when it contains no suitable segment yet.
            // TODO: consider staleness?
            if !waiting_for_playlist {
                self.abort_segment_requests_with_type(media_type);
            }
            return waiting_for_playlist;
        };
        let still_needed = {
            if let Some(init) = needed.init_segment() {
                self.requester
                    .is_requesting_segment(media_type, init.url(), init.byte_range())
            } else if let Some((media, _)) = needed.media_segment() {
                self.requester
                    .is_requesting_segment(media_type, media.url(), media.byte_range())
            } else {
                false
            }
        };
        if !still_needed {
            self.abort_segment_requests_with_type(media_type);
        }
        still_needed
    }

    // If `true`, the current pending request for the given `MediaType` should be interrupted.
    // Takes the `MediaType` and the current adaptive logic estimates of what the current variants
    // to target are.
    fn should_interrupt_download(
        &self,
        media_type: MediaType,
        recommendation: AdaptiveVariantSelection,
    ) -> bool {
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return false;
        };
        if pl_store.is_variant_locked() {
            return false;
        }
        let playback = self.playback_conditions(pl_store);
        self.segment_request_contexts.has(|context| {
            // TODO: Should we cancel init requests more often? They don't add playable media,
            // but they're small, so maybe just finishing them is better? The variant's bitrate
            // doesn't help much here. Also check seeks needing another init: init requests have
            // no timing, so the buffer-window check won't cancel them.
            let variant_id = match context {
                PendingSegmentRequest::Init {
                    media_type: request_type,
                    variant_id,
                    ..
                }
                | PendingSegmentRequest::Media {
                    media_type: request_type,
                    variant_id,
                    ..
                } if *request_type == media_type => *variant_id,
                _ => return false,
            };
            match (
                pl_store.variant(variant_id),
                pl_store.variant(recommendation.best_variant_id),
                pl_store.variant(recommendation.safe_variant_id),
            ) {
                (Some(downloading), Some(best), Some(safe)) => {
                    should_interrupt_download(downloading, best, safe, &playback)
                }
                _ => false,
            }
        })
    }

    /// Select and schedule the next initialization or media segment for `media_type`, using
    /// the variant recommendation and current buffer inventory. Skip selection if a segment
    /// request is already pending for this type.
    ///
    /// Update the active variant when needed and follow the playlist chosen for refreshing,
    /// which may differ from the playlist supplying the selected segment. Playlist tracking
    /// can change even when no segment is scheduled.
    fn schedule_next_segment_for_type(
        &mut self,
        media_type: MediaType,
        recommendation: Option<AdaptiveVariantSelection>,
    ) {
        if self.requester.has_segment_request_pending(media_type) {
            self.media_element_ref.set_end_when_done(media_type, false);
            return;
        }
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        let Some((addition_candidate, replacement_candidate)) =
            segment_selection_candidates(pl_store, recommendation, media_type)
        else {
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
        self.media_element_ref
            .set_end_when_done(media_type, selected.loading_complete());
        // Own the proposed metadata before following, which may request another playlist.
        // Only one resource is loaded at a time: initialization first, otherwise media.
        let needed = selected.needed_segments().map(|needed| {
            let (init, media) = if let Some(init) = needed.init_segment() {
                (Some(init.clone()), None)
            } else {
                (
                    None,
                    needed
                        .media_segment()
                        .map(|(media, init_segment_id)| (media.clone(), init_segment_id)),
                )
            };
            (needed.variant_id(), init, media)
        });
        self.follow_media_playlist(media_type, refresh_id);
        let Some((variant_id, init, media)) = needed else {
            self.ready_probe_segments.clear_media_type(media_type);
            return;
        };
        let (playlist_id, quality_context) = if variant_id == addition_variant_id {
            (addition_playlist_id, addition_context)
        } else {
            (replacement_playlist_id, replacement_context)
        };

        let Some(pl_store) = self.playlist_store.as_mut() else {
            return;
        };
        if pl_store.is_variant_locked() && pl_store.current_variant_id() != Some(variant_id) {
            // TODO: A recommendation should not contradict the lock. Report an internal error if
            // this becomes reachable instead of repeatedly skipping segment scheduling.
            log_warn!("Core: Segment selection contradicted the variant lock");
            return;
        }

        // We could target a different variant but rely on the same playlist for the current media
        // type.
        // In that case, we technically could stay in the same variant, we don't need to change it
        // from the PlaylistStore POV for now, that would be disruptive for nothing.
        if Some(&playlist_id) != pl_store.current_media_playlist_id(media_type)
            && Some(variant_id) != pl_store.current_variant_id()
        {
            // TODO: Verify transitions between separate and muxed audio in both directions.
            let changes = pl_store.set_variant(variant_id);
            // XXX TODO: We may also want to call other change anouncement API here?
            // I know currently it would serve no role but dispatcher does not know that...
            // Or else the return type chosen in PlaylistStore is completely wrong
            self.announce_track_changes(&changes);
            let changed_media_types = changes.changed_playlists;
            if changed_media_types.contains(&MediaType::Text) {
                self.reset_subtitle_playlist();
            }
            for changed_type in changed_media_types {
                self.ready_probe_segments.clear_media_type(changed_type);
                if recommendation.is_some_and(|selection| {
                    self.should_interrupt_download(changed_type, selection)
                }) {
                    self.abort_segment_requests_with_type(changed_type);
                }
                // Selection already chose this type's refresh target. It may differ from
                // the playlist supplying the segment; only retarget the other type here.
                if changed_type != media_type {
                    self.follow_selected_media_playlist(changed_type);
                }
            }
        }

        if !self.ready_to_load_segments() {
            return;
        }

        if let Some(init) = init {
            if let Some(data) =
                self.take_probe_segment_data(media_type, init.url(), init.byte_range())
            {
                self.on_init_segment_loaded(data, media_type, init.id());
                return;
            }
            let req_id = self
                .segment_request_contexts
                .insert(PendingSegmentRequest::Init {
                    variant_id,
                    media_type,
                    init_segment_id: init.id(),
                });
            self.requester
                .request_init_segment(media_type, init.url(), init.byte_range(), req_id);
        } else if let Some((media, init_segment_id)) = media {
            if let Some(data) =
                self.take_probe_segment_data(media_type, media.url(), media.byte_range())
            {
                self.on_media_segment_loaded(SegmentPushMetadata {
                    data,
                    media_type,
                    init_segment_id,
                    time_info: media.time_info().clone(),
                    context: quality_context,
                    sequence_number: media.sequence(),
                    discontinuity_sequence: media.discontinuity_sequence(),
                });
                return;
            }
            let req_id = self
                .segment_request_contexts
                .insert(PendingSegmentRequest::Media {
                    variant_id,
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

    /// Follow the playback playlist (the one we're loading segments for) when its configuration
    /// changes.
    fn follow_selected_media_playlist(&mut self, media_type: MediaType) {
        let playlist_id = self
            .playlist_store
            .as_ref()
            .and_then(|pl_store| pl_store.current_media_playlist_id(media_type).copied());
        if let Some(playlist_id) = playlist_id {
            self.follow_media_playlist(media_type, playlist_id);
        } else {
            self.playlist_refresh_manager.forget_media_type(media_type);
        }
    }

    /// Set which playlist to refresh for this media type.
    /// Wait until its next reload time, or request it now if that time has passed.
    /// Finished playlists and requests already in progress need no new request.
    fn follow_media_playlist(&mut self, media_type: MediaType, id: MediaPlaylistPermanentId) {
        if !self.playlist_refresh_manager.follow(media_type, id) {
            return;
        }
        let Some(pl_store) = self.playlist_store.as_ref() else {
            return;
        };
        let needs_reload = pl_store
            .media_playlist(&id)
            .is_none_or(|playlist| playlist.may_be_refreshed());

        if !needs_reload {
            return;
        }
        let Some(url) = pl_store.media_playlist_url(&id) else {
            return;
        };
        let playlist_type = PlaylistFileType::MediaPlaylist { id, media_type };
        if !self.requester.is_requesting_playlist(url, &playlist_type) {
            self.requester.fetch_playlist(url.clone(), playlist_type);
        }
    }

    /// Restart buffering from the selected playlists and types from scratch:
    /// Discard previous probes, abort segment requests, and flush buffered data for each given
    /// media type, even if its selected playlist has not changed.
    /// Might thus lead to rebuffering.
    ///
    /// You might want to call this method when you want a direct and clean restart. E.g. at audio
    /// track switch.
    fn reset_on_selected_media_playlists(&mut self, media_types: &[MediaType]) {
        if self.playlist_store.is_none() {
            return;
        }

        for media_type in media_types.iter().copied() {
            log_info!("Core: Resetting selected {} MediaPlaylist", media_type);
            self.ready_probe_segments.clear_media_type(media_type);

            self.follow_selected_media_playlist(media_type);
            self.abort_segment_requests_with_type(media_type);
            self.flush_media_buffer(media_type);
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
            .restart_from_position(
                self.media_element_ref.wanted_position() - SEGMENT_SELECTION_MARGIN,
            );
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

        let Some(req_ctxt) = self.segment_request_contexts.take(segment_req.id()) else {
            log_warn!("Loaded segment with unknown pending context.");
            return;
        };

        let media_type = segment_req.media_type();
        match req_ctxt {
            PendingSegmentRequest::Media {
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
                if req_media_type == MediaType::Text {
                    // XXX TODO: Parse text in Rust here or in MediaElementRef?
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
                ..
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
                self.on_probe_segment_loaded(probe_segment, requested_media_type, result);
            }
        }
    }

    /// Announce the variant chosen for loading, independently of playlist and codec readiness.
    fn announce_current_variant(&self) {
        let Some(playlist_store) = self.playlist_store.as_ref() else {
            return;
        };
        let variant_id = if playlist_store.direct_media_playlist().is_some() {
            Some(DIRECT_MEDIA_VARIANT_ID)
        } else {
            playlist_store.current_variant_id()
        };
        jsAnnounceVariantUpdate(variant_id);
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
            segment_start,
            segment_end,
        );
        self.check_segments_to_request();
        // Preceding check could have led to a fatal error
        if matches!(self.ready_state, PlayerReadyState::Stopped) {
            return;
        }

        if let Err(x) = self
            .media_element_ref
            .push_media_segment(media_type, prepared_data)
        {
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
    }

    fn on_init_segment_loaded(&mut self, data: JsMemoryBlob, media_type: MediaType, init_id: f64) {
        if media_type == MediaType::Text {
            // XXX TODO: To think about what to do with text init segment
        }

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

    /// Abort pending and queued requests and remove their contexts.
    fn abort_segment_requests_with_type(&mut self, media_type: MediaType) {
        let aborted_reqs = self.requester.abort_segments_with_type(media_type);
        for req_id in aborted_reqs {
            self.segment_request_contexts.take(req_id);
        }
    }
}

fn segment_selection_candidates(
    playlist_store: &PlaylistStore,
    recommendation: Option<AdaptiveVariantSelection>,
    media_type: MediaType,
) -> Option<(SegmentSelectionCandidate<'_>, SegmentSelectionCandidate<'_>)> {
    let recommendation = normalize_abr_selection(playlist_store, recommendation, media_type);
    Some((
        segment_selection_candidate_for_variant(
            recommendation.map(|selection| selection.best_variant_id),
            playlist_store,
            media_type,
        )?,
        segment_selection_candidate_for_variant(
            recommendation.map(|selection| selection.safe_variant_id),
            playlist_store,
            media_type,
        )?,
    ))
}

fn segment_selection_candidate_for_variant(
    variant_id: Option<u32>,
    pl_store: &PlaylistStore,
    media_type: MediaType,
) -> Option<SegmentSelectionCandidate<'_>> {
    if let Some(variant_id) = variant_id {
        let variant = pl_store.variant(variant_id)?;
        let playlist_id = match pl_store.resolve_playlist_id_for_variant(variant, media_type) {
            Ok(Some(playlist_id)) => playlist_id,
            Ok(None) => return None,
            Err(IncompatibleTrack) => return None,
        };
        let context = SegmentQualityContext::new(
            variant.score().unwrap_or(variant.bandwidth() as f64),
            playlist_id,
        );
        Some(SegmentSelectionCandidate {
            variant_id,
            context,
            is_ended: pl_store
                .media_playlist(&context.playlist_id())
                .is_some_and(|playlist| playlist.is_ended()),
            playlist: pl_store
                .media_playlist(&context.playlist_id())
                .map(|playlist| playlist.segment_list()),
        })
    } else {
        let (_, playlist) = pl_store.direct_media_playlist()?;
        let playlist_id = *pl_store.current_media_playlist_id(media_type)?;
        let context = SegmentQualityContext::new(0., playlist_id);
        Some(SegmentSelectionCandidate {
            variant_id: DIRECT_MEDIA_VARIANT_ID,
            context,
            playlist: Some(playlist.segment_list()),
            is_ended: playlist.is_ended(),
        })
    }
}

/// XXX TODO WHAT ABOUT TEXT?
/// Keep audio and video variant choices compatible whether segment selection uses Adaptive's
/// "best" or "safe" candidate for any `media_type`.
///
/// If any of those candidates for one `media_type` leads to the need to select a different
/// playlist for an other media type, use the safe variant also for "best".
///
/// This allows to ensure a single logical variant is considered at once for all media types,
/// keeping the dispatcher's choices within declared HLS combinations and letting other modules
/// track a single selected variant.
///
/// TODO: Abort in-flight requests for incompatible variant if not compatible with a new
/// considered-one? Or maybe not, to ponder.
fn normalize_abr_selection(
    pl_store: &PlaylistStore,
    recommendation: Option<AdaptiveVariantSelection>,
    media_type: MediaType,
) -> Option<AdaptiveVariantSelection> {
    let mut selection = recommendation?;
    let other_type = match media_type {
        MediaType::Video => MediaType::Audio,
        MediaType::Audio => MediaType::Video,
        // XXX TODO Adapt here instead
        MediaType::Text => return None,
    };
    let safe_variant = pl_store.variant(selection.safe_variant_id)?;
    let safe_playlist_id = match pl_store.resolve_playlist_id_for_variant(safe_variant, other_type)
    {
        Ok(playlist_id) => playlist_id,
        Err(IncompatibleTrack) => return None,
    };
    let best_variant = pl_store.variant(selection.best_variant_id)?;
    match pl_store.resolve_playlist_id_for_variant(best_variant, other_type) {
        Ok(playlist_id) => {
            if playlist_id != safe_playlist_id {
                selection.best_variant_id = selection.safe_variant_id;
            }
        }
        Err(IncompatibleTrack) => selection.best_variant_id = selection.safe_variant_id,
    }
    Some(selection)
}

/// Set the current MediaSource's duration according to the content stored in the given
/// PlaylistStore.
///
/// If the content is EVENT/LIVE, set a very large value instead.
fn sync_media_source_duration(pl_store: &PlaylistStore) {
    let mut playback_playlists = pl_store
        .current_media_playlists()
        .filter(|(media_type, _)| matches!(media_type, MediaType::Audio | MediaType::Video))
        .peekable();
    let playback_finalized = playback_playlists.peek().is_some()
        && playback_playlists.all(|(_, playlist)| !playlist.may_be_refreshed());
    if playback_finalized {
        if let Some(duration) = utils::estimated_playback_maximum_position(pl_store) {
            let _ = jsSetMediaSourceDuration(duration);
        } else {
            log_warn!("Core: Unknown finalized content duration");
        }
    } else {
        // NOTE: u32::MAX selected because of large history of poor support of
        // Infinity or high numbers for various devices.
        // TODO: To perhaps isolate in compatibility related-code to make it clearer.
        let _ = jsSetMediaSourceDuration(u32::MAX as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_abr_selection, segment_selection_candidate_for_variant, Dispatcher,
        PlayerReadyState,
    };
    use crate::{
        bindings::MediaType,
        dispatcher::{JsTimeRanges, MediaObservation, PlaybackTickReason},
        playlist_store::PlaylistStore,
        utils::url::Url,
    };

    #[test]
    fn initial_audio_preferences_skip_unused_groups() {
        for mode in 0..3 {
            let mut pl_store = PlaylistStore::try_new(
                br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="Unused",LANGUAGE="fr",ASSOC-LANGUAGE="en",URI="unused.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="Usable",LANGUAGE="en",ASSOC-LANGUAGE="en",URI="audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=100000,AUDIO="audio"
video.m3u8
"#
                .as_slice(),
                Url::new("https://example.com/master.m3u8".into()),
            )
            .unwrap();
            let mut dispatcher = Dispatcher::new(100000.);
            dispatcher.initial_audio_track_selection = if mode == 1 {
                vec![
                    crate::dispatcher::InitialAudioTrackSelection {
                        language: Some("fr".into()),
                        ..Default::default()
                    },
                    crate::dispatcher::InitialAudioTrackSelection {
                        language: Some("en".into()),
                        ..Default::default()
                    },
                ]
            } else {
                vec![crate::dispatcher::InitialAudioTrackSelection {
                    language: (mode == 2).then(|| "fr".into()),
                    assoc_language: Some("en".into()),
                    ..Default::default()
                }]
            };
            dispatcher.apply_initial_audio_track_selection(&mut pl_store);
            let usable = pl_store
                .audio_tracks()
                .iter()
                .find(|track| track.name() == Some("Usable"))
                .unwrap()
                .id();
            assert_eq!(
                pl_store.fixed_audio_track_id(),
                (mode != 2).then_some(usable)
            );
            assert_eq!(pl_store.compatible_variants().count(), 1);
            assert!(dispatcher.initial_audio_track_selection.is_empty());
        }
    }

    fn selected_playlist(manifest: &[u8]) -> PlaylistStore {
        let mut pl_store = PlaylistStore::try_new(
            manifest,
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let variant_id = pl_store.available_variants().next().unwrap().id();
        pl_store.set_variant(variant_id);
        pl_store
    }

    #[test]
    fn initial_subtitle_preferences_skip_tracks_incompatible_with_selected_audio() {
        let mut pl_store = selected_playlist(b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"en\",NAME=\"English\",LANGUAGE=\"en\",URI=\"en.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"fr\",NAME=\"French\",LANGUAGE=\"fr\",URI=\"fr.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"a\",NAME=\"English\",LANGUAGE=\"en\",URI=\"en-subs.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"b\",NAME=\"French\",LANGUAGE=\"fr\",URI=\"fr-subs.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=100000,AUDIO=\"en\",SUBTITLES=\"a\"\na.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=200000,AUDIO=\"fr\",SUBTITLES=\"b\"\nb.m3u8\n");
        let audio_id = pl_store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("en"))
            .unwrap()
            .id();
        pl_store.set_audio_track(Some(audio_id)).unwrap();
        let mut dispatcher = Dispatcher::new(100000.);
        dispatcher.initial_subtitle_track_selection = vec![
            Some(crate::dispatcher::InitialSubtitleTrackSelection {
                language: Some("fr".into()),
                forced: Some(false),
                ..Default::default()
            }),
            Some(crate::dispatcher::InitialSubtitleTrackSelection {
                language: Some("en".into()),
                forced: Some(false),
                ..Default::default()
            }),
        ];
        let french = pl_store
            .subtitle_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        assert_eq!(
            pl_store.set_subtitle_track(Some(french)),
            Err(crate::playlist_store::TrackSelectionError::NoCompatibleVariant)
        );
        dispatcher.apply_initial_subtitle_track_selection(&mut pl_store);
        let english_subtitle = pl_store
            .subtitle_tracks()
            .iter()
            .find(|track| track.language() == Some("en"))
            .unwrap()
            .id();
        assert_eq!(pl_store.fixed_audio_track_id(), Some(audio_id));
        assert_eq!(pl_store.fixed_subtitle_track_id(), Some(english_subtitle));
        assert!(dispatcher.initial_subtitle_track_selection.is_empty());
    }

    #[test]
    fn changing_subtitle_selection_cancels_only_text_requests_and_contexts() {
        let mut pl_store = selected_playlist(b"#EXTM3U\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"English\",DEFAULT=YES,URI=\"subs.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=100000,CODECS=\"avc1.42E01E\",SUBTITLES=\"subs\"\nvideo.m3u8\n");
        let text_id = *pl_store.current_media_playlist_id(MediaType::Text).unwrap();
        let media = pl_store
            .update_media_playlist(
                &text_id,
                &b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.vtt\n"[..],
                Url::new("https://example.com/subs.m3u8".into()),
            )
            .unwrap()
            .segment_list()
            .media()[0]
            .clone();
        let mut dispatcher = Dispatcher::new(100000.);
        dispatcher.playlist_store = Some(pl_store);
        dispatcher.requester.lock_segment_requests();
        for media_type in [MediaType::Text, MediaType::Video] {
            let context = dispatcher.segment_request_contexts.insert(
                crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
                    variant_id: 0,
                    media_type,
                    init_segment_id: None,
                    quality_context: crate::media_element::SegmentQualityContext::new(0., text_id),
                    sequence_number: 0,
                    discontinuity_sequence: 0,
                },
            );
            dispatcher
                .requester
                .request_media_segment(media_type, &media, context);
        }
        dispatcher.disable_subtitle_track_core();
        assert!(!dispatcher
            .requester
            .has_segment_request_pending(MediaType::Text));
        assert!(dispatcher
            .requester
            .has_segment_request_pending(MediaType::Video));
        assert!(!dispatcher.segment_request_contexts.has(|context| matches!(
            context,
            crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
                media_type: MediaType::Text,
                ..
            }
        )));
        assert!(dispatcher.segment_request_contexts.has(|context| matches!(
            context,
            crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
                media_type: MediaType::Video,
                ..
            }
        )));
    }

    #[test]
    fn additions_use_safe_when_the_other_playlist_differs() {
        for shared_audio in [false, true] {
            let high_group = if shared_audio { "low" } else { "high" };
            let manifest = format!(
                "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"low\",NAME=\"English\",DEFAULT=YES,URI=\"low-audio.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"high\",NAME=\"English\",DEFAULT=YES,URI=\"high-audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"low\"\nlow-video.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"{high_group}\"\nhigh-video.m3u8\n"
            );
            let mut pl_store = selected_playlist(manifest.as_bytes());
            let low = pl_store.available_variants().next().unwrap().id();
            let high = pl_store.available_variants().nth(1).unwrap().id();
            let recommendation = Some(crate::adaptive::AdaptiveVariantSelection {
                best_variant_id: high,
                safe_variant_id: low,
            });
            let video_selection =
                normalize_abr_selection(&pl_store, recommendation, MediaType::Video).unwrap();
            assert_eq!(video_selection.safe_variant_id, low);
            assert_eq!(
                video_selection.best_variant_id,
                if shared_audio { high } else { low }
            );
            let safe = segment_selection_candidate_for_variant(
                Some(video_selection.safe_variant_id),
                &pl_store,
                MediaType::Video,
            )
            .unwrap();
            // A missing playlist stays eligible and can be refreshed by the caller.
            assert!(safe.playlist.is_none());
            let audio_selection =
                normalize_abr_selection(&pl_store, recommendation, MediaType::Audio).unwrap();
            assert_eq!(audio_selection.safe_variant_id, low);
            assert_eq!(audio_selection.best_variant_id, low);
            let old_audio = *pl_store
                .current_media_playlist_id(MediaType::Audio)
                .unwrap();
            let changed = pl_store.set_variant(high);
            assert_eq!(pl_store.current_variant_id(), Some(high));
            assert!(changed.changed_playlists.contains(&MediaType::Video));
            assert_eq!(
                changed.changed_playlists.contains(&MediaType::Audio),
                !shared_audio
            );
            assert_eq!(
                pl_store.current_media_playlist_id(MediaType::Audio) == Some(&old_audio),
                shared_audio
            );
        }
    }

    #[test]
    fn shared_video_does_not_select_safe_variant_before_best_audio() {
        for distinct_subtitle_tracks in [false, true] {
            let manifest = "#EXTM3U\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"low-subs\",NAME=\"English\",DEFAULT=YES,URI=\"low-subs.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"high-subs\",NAME=\"English\",DEFAULT=YES,URI=\"high-subs.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"low\",NAME=\"English\",URI=\"low-audio.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"high\",NAME=\"English\",URI=\"high-audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"low\",SUBTITLES=\"low-subs\"\nvideo.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"high\",SUBTITLES=\"high-subs\"\nvideo.m3u8\n";
            let manifest = if distinct_subtitle_tracks {
                manifest.replace(
                    "GROUP-ID=\"high-subs\",NAME=\"English\"",
                    "GROUP-ID=\"high-subs\",NAME=\"French\"",
                )
            } else {
                manifest.to_owned()
            };
            let mut pl_store = selected_playlist(manifest.as_bytes());
            let low = pl_store.available_variants().next().unwrap().id();
            let high = pl_store.available_variants().nth(1).unwrap().id();
            for media_type in [MediaType::Video, MediaType::Audio, MediaType::Text] {
                for variant_id in [low, high] {
                    let id = pl_store
                        .resolve_playlist_id_for_variant(
                            pl_store.variant(variant_id).unwrap(),
                            media_type,
                        )
                        .unwrap()
                        .unwrap();
                    let manifest = format!(
                    "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n{variant_id}-{media_type}.ts\n#EXT-X-ENDLIST\n"
                );
                    pl_store
                        .update_media_playlist(
                            &id,
                            manifest.as_bytes(),
                            Url::new("https://example.com/media.m3u8".into()),
                        )
                        .unwrap();
                }
            }
            let high_audio_id = pl_store
                .resolve_playlist_id_for_variant(pl_store.variant(high).unwrap(), MediaType::Audio)
                .unwrap()
                .unwrap();
            let high_audio_segment = pl_store
                .media_playlist(&high_audio_id)
                .unwrap()
                .segment_list()
                .media()[0]
                .clone();
            pl_store.assume_variant_supported_for_test(low);
            pl_store.assume_variant_supported_for_test(high);
            pl_store.set_variant(low);
            let recommendation = Some(crate::adaptive::AdaptiveVariantSelection {
                best_variant_id: high,
                safe_variant_id: low,
            });
            let mut dispatcher = Dispatcher::new(3_125_000.);
            dispatcher.ready_state = PlayerReadyState::AwaitingSegments;
            dispatcher.playlist_store = Some(pl_store);
            dispatcher.requester.lock_segment_requests();
            dispatcher.announce_current_subtitle_track();
            crate::bindings::take_selection_announcements();

            dispatcher.schedule_next_segment_for_type(MediaType::Video, recommendation);
            assert_eq!(
                dispatcher
                    .playlist_store
                    .as_ref()
                    .unwrap()
                    .current_variant_id(),
                Some(low)
            );
            assert!(crate::bindings::take_selection_announcements().is_empty());
            dispatcher.check_segments_to_request();
            let mut expected = Vec::new();
            if distinct_subtitle_tracks {
                expected.push((
                    Some(MediaType::Text),
                    dispatcher
                        .playlist_store
                        .as_ref()
                        .unwrap()
                        .current_subtitle_track_id(),
                ));
            }
            expected.push((None, Some(high)));
            assert_eq!(crate::bindings::take_selection_announcements(), expected);
            assert_eq!(
                dispatcher
                    .playlist_store
                    .as_ref()
                    .unwrap()
                    .current_variant_id(),
                Some(high)
            );
            assert!(dispatcher.requester.is_requesting_segment(
                MediaType::Audio,
                high_audio_segment.url(),
                high_audio_segment.byte_range()
            ));
        }
    }

    fn dispatcher_with_buffer_observation(
        audio: Option<Vec<f64>>,
        video: Option<Vec<f64>>,
    ) -> Dispatcher {
        let mut dispatcher = Dispatcher::new(3_125_000.);
        dispatcher.ready_state = PlayerReadyState::AwaitingSegments;
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
        let variant_ids: Vec<_> = pl_store.available_variants().map(|v| v.id()).collect();
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
    fn adaptive_switches_replace_queued_requests_and_position_jumps_cancel_obsolete_requests() {
        for (buffer, alternative_available) in [(1., true), (1., false), (8.1, true)] {
            let mut dispatcher = dispatcher_with_abr_playlists(Some(4), buffer);
            let playlist_store = dispatcher.playlist_store.as_mut().unwrap();
            let high = playlist_store.available_variants().nth(2).unwrap().id();
            playlist_store.set_variant(high);
            let playlist_id = *playlist_store
                .current_media_playlist_id(MediaType::Video)
                .unwrap();
            playlist_store.update_media_playlist(
                &playlist_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nhigh-0.ts\n#EXTINF:4,\nhigh-1.ts\n#EXTINF:4,\nhigh-2.ts\n#EXTINF:4,\nhigh-3.ts\n#EXT-X-ENDLIST\n".as_slice(),
                Url::new("https://example.com/high.m3u8".into()),
            ).unwrap();
            let segment = playlist_store
                .media_playlist(&playlist_id)
                .unwrap()
                .segment_list()
                .media()[3]
                .clone();
            dispatcher.requester.lock_segment_requests();
            dispatcher
                .requester
                .request_media_segment(MediaType::Video, &segment, 123);
            assert!(!dispatcher.requester.has_segment_in_flight(MediaType::Video));
            let recommendation = dispatcher.compute_optimal_variant().unwrap();
            if !alternative_available {
                let playlist_store = dispatcher.playlist_store.as_mut().unwrap();
                let replacement_id = playlist_store
                    .resolve_playlist_id_for_variant(
                        playlist_store
                            .variant(recommendation.best_variant_id)
                            .unwrap(),
                        MediaType::Video,
                    )
                    .unwrap()
                    .unwrap();
                playlist_store
                    .update_media_playlist(
                        &replacement_id,
                        b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-ENDLIST\n".as_slice(),
                        Url::new("https://example.com/no-segment.m3u8".into()),
                    )
                    .unwrap();
            }
            dispatcher.check_requested_segments_still_needed();
            assert!(!dispatcher
                .requester
                .has_segment_request_pending(MediaType::Video));
            if buffer == 8.1 {
                for (position, buffer_goal) in [(50., 30.), (0., 5.)] {
                    if !dispatcher
                        .requester
                        .has_segment_request_pending(MediaType::Video)
                    {
                        dispatcher
                            .requester
                            .request_media_segment(MediaType::Video, &segment, 123);
                    }
                    dispatcher.buffer_goal = buffer_goal;
                    dispatcher
                        .media_element_ref
                        .on_observation(MediaObservation::new(
                            PlaybackTickReason::RegularInterval,
                            position,
                            4,
                            JsTimeRanges::new(vec![0., 70.]),
                            false,
                            false,
                            false,
                            f64::MAX,
                            None,
                            Some(JsTimeRanges::new(vec![0., 70.])),
                        ));
                    // No seek handler or cursor restart, and no urgent quality switch.
                    dispatcher.check_requested_segments_still_needed();
                    assert!(!dispatcher
                        .requester
                        .has_segment_request_pending(MediaType::Video));
                }
            }
        }
    }

    #[test]
    fn coordinated_switch_cancels_only_urgent_requests() {
        for (buffer, origin, urgent) in [(8.1, 1, false), (1., 1, true), (1., 0, false)] {
            let mut playlist_store = selected_playlist(
                b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"low\",NAME=\"English\",DEFAULT=YES,URI=\"low-audio.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"high\",NAME=\"English\",DEFAULT=YES,URI=\"high-audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"low\"\nlow-video.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"high\"\nhigh-video.m3u8\n",
            );
            let variant_ids: Vec<_> = playlist_store
                .available_variants()
                .map(|variant| variant.id())
                .collect();
            for variant_id in variant_ids.iter().copied() {
                for media_type in [MediaType::Video, MediaType::Audio] {
                    let playlist_id = playlist_store
                        .resolve_playlist_id_for_variant(
                            playlist_store.variant(variant_id).unwrap(),
                            media_type,
                        )
                        .unwrap()
                        .unwrap();
                    let manifest = format!("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\n{variant_id}-{media_type}.ts\n#EXT-X-ENDLIST\n");
                    playlist_store
                        .update_media_playlist(
                            &playlist_id,
                            manifest.as_bytes(),
                            Url::new("https://example.com/media.m3u8".into()),
                        )
                        .unwrap();
                }
                playlist_store.assume_variant_supported_for_test(variant_id);
            }
            playlist_store.set_variant(variant_ids[1]);
            let audio_id = playlist_store
                .resolve_playlist_id_for_variant(
                    playlist_store.variant(variant_ids[origin]).unwrap(),
                    MediaType::Audio,
                )
                .unwrap()
                .unwrap();
            let old_audio = playlist_store
                .media_playlist(&audio_id)
                .unwrap()
                .segment_list()
                .media()[0]
                .clone();
            let quality_context = segment_selection_candidate_for_variant(
                Some(variant_ids[origin]),
                &playlist_store,
                MediaType::Audio,
            )
            .unwrap()
            .context;
            let mut dispatcher =
                dispatcher_with_buffer_observation(None, Some(vec![0., 10. + buffer]));
            dispatcher.ready_state = PlayerReadyState::AwaitingSegments;
            dispatcher.playlist_store = Some(playlist_store);
            dispatcher.requester.lock_segment_requests();
            let request_id = dispatcher.segment_request_contexts.insert(
                crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
                    variant_id: variant_ids[origin],
                    media_type: MediaType::Audio,
                    init_segment_id: None,
                    quality_context,
                    sequence_number: old_audio.sequence(),
                    discontinuity_sequence: old_audio.discontinuity_sequence(),
                },
            );
            dispatcher
                .requester
                .request_media_segment(MediaType::Audio, &old_audio, request_id);
            dispatcher.schedule_next_segment_for_type(
                MediaType::Video,
                Some(crate::adaptive::AdaptiveVariantSelection {
                    best_variant_id: variant_ids[0],
                    safe_variant_id: variant_ids[0],
                }),
            );
            assert_eq!(
                dispatcher
                    .playlist_store
                    .as_ref()
                    .unwrap()
                    .current_variant_id(),
                Some(variant_ids[0])
            );
            assert_eq!(
                dispatcher.requester.is_requesting_segment(
                    MediaType::Audio,
                    old_audio.url(),
                    old_audio.byte_range()
                ),
                !urgent
            );
            assert!(dispatcher
                .requester
                .has_segment_request_pending(MediaType::Video));
        }
    }

    #[test]
    fn seek_keeps_the_segment_selected_by_the_look_back_margin() {
        for (index, survives) in [(18, false), (19, true)] {
            let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 0.);
            let playlist_store = dispatcher.playlist_store.as_mut().unwrap();
            let id = *playlist_store
                .current_media_playlist_id(MediaType::Video)
                .unwrap();
            let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:2\n");
            for number in 0..30 {
                text.push_str(&format!("#EXTINF:2,\nseg-{number}.m4s\n"));
            }
            text.push_str("#EXT-X-ENDLIST\n");
            playlist_store
                .update_media_playlist(
                    &id,
                    text.as_bytes(),
                    Url::new("https://example.com/low.m3u8".into()),
                )
                .unwrap();
            let segment = playlist_store
                .media_playlist(&id)
                .unwrap()
                .segment_list()
                .media()[index]
                .clone();
            dispatcher
                .media_element_ref
                .on_observation(MediaObservation::new(
                    PlaybackTickReason::Seeking,
                    40.,
                    1,
                    JsTimeRanges::new(vec![0., 22.]),
                    true,
                    false,
                    true,
                    60.,
                    None,
                    Some(JsTimeRanges::new(vec![0., 22.])),
                ));
            dispatcher.segment_selectors.restart_from_position(39.8);
            dispatcher.requester.lock_segment_requests();
            dispatcher
                .requester
                .request_media_segment(MediaType::Video, &segment, 123);
            assert_eq!(
                dispatcher.check_requested_segment_still_needed(MediaType::Video),
                survives
            );
            if survives {
                // Repeated ticks must not keep cancelling and reselecting the 38–40 segment.
                assert!(dispatcher.check_requested_segment_still_needed(MediaType::Video));
                assert!(dispatcher.requester.is_requesting_segment(
                    MediaType::Video,
                    segment.url(),
                    segment.byte_range(),
                ));
            }
        }
    }

    #[test]
    fn regular_playback_replaces_obsolete_requests_with_media_near_playback() {
        let mut text = String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nseg-{number}.m4s\n"));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 8.1);
        let pl_store = dispatcher.playlist_store.as_mut().unwrap();
        let variant_ids: Vec<_> = pl_store.available_variants().map(|v| v.id()).collect();
        for variant_id in variant_ids {
            let id = pl_store
                .resolve_playlist_id_for_variant(
                    pl_store.variant(variant_id).unwrap(),
                    MediaType::Video,
                )
                .unwrap()
                .unwrap();
            pl_store
                .update_media_playlist(
                    &id,
                    text.as_bytes(),
                    Url::new("https://example.com/media.m3u8".into()),
                )
                .unwrap();
        }
        dispatcher.requester.lock_segment_requests();

        for (position, expected_start) in [(0., 0.), (12., 8.), (12., 8.), (16.2, 16.)] {
            dispatcher
                .media_element_ref
                .on_observation(MediaObservation::new(
                    PlaybackTickReason::RegularInterval,
                    position,
                    4,
                    JsTimeRanges::new(vec![0., 40.]),
                    false,
                    false,
                    false,
                    40.,
                    None,
                    Some(JsTimeRanges::new(vec![0., 40.])),
                ));
            // Run the tick's selection steps without browser-only variant notifications.
            dispatcher
                .segment_selectors
                .advance_position(position - super::SEGMENT_SELECTION_MARGIN);
            dispatcher.check_requested_segments_still_needed();
            let recommendation = dispatcher.compute_optimal_variant();
            dispatcher.schedule_next_segment_for_type(MediaType::Video, recommendation);
            assert_eq!(
                dispatcher
                    .requester
                    .pending_segment_time_info(MediaType::Video)
                    .unwrap()
                    .start(),
                expected_start
            );
            assert!(dispatcher.check_requested_segment_still_needed(MediaType::Video));
        }
    }

    #[test]
    fn direct_media_preserves_useful_requests_without_adaptive_reconsideration() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 8.1);
        let playlist_store = dispatcher.playlist_store.as_mut().unwrap();
        *playlist_store = PlaylistStore::try_new(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n#EXT-X-ENDLIST\n",
            Url::new("https://example.com/direct.m3u8".into()),
        )
        .unwrap();
        playlist_store.set_external_media_info(
            crate::parser::ExternalMediaInfo {
                mime_type: "video/mp4".into(),
                codec: "avc1.42E01E".into(),
                media_type: MediaType::Video,
            },
            MediaType::Video,
        );
        let segment = playlist_store
            .direct_media_playlist()
            .unwrap()
            .1
            .segment_list()
            .media()[0]
            .clone();
        dispatcher
            .media_element_ref
            .on_observation(MediaObservation::new(
                PlaybackTickReason::RegularInterval,
                0.,
                4,
                JsTimeRanges::new(vec![0., 20.]),
                false,
                false,
                false,
                f64::MAX,
                None,
                Some(JsTimeRanges::new(vec![0., 20.])),
            ));
        dispatcher.requester.lock_segment_requests();
        dispatcher
            .requester
            .request_media_segment(MediaType::Video, &segment, 123);
        assert!(dispatcher.compute_optimal_variant().is_none());
        dispatcher.check_requested_segments_still_needed();
        assert!(dispatcher.check_requested_segment_still_needed(MediaType::Video));
        assert!(dispatcher
            .requester
            .has_segment_request_pending(MediaType::Video));
    }

    #[test]
    fn refused_variant_choice_does_not_schedule_a_segment() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        let pl_store = dispatcher.playlist_store.as_mut().unwrap();
        let current = pl_store.current_variant_id().unwrap();
        assert_ne!(recommendation.best_variant_id, current);
        assert!(matches!(
            pl_store.lock_variant(current),
            crate::playlist_store::LockVariantResponse::VariantLocked(_)
        ));
        // A previously computed recommendation must not bypass a newer lock.
        dispatcher.schedule_next_segment_for_type(MediaType::Video, Some(recommendation));
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
        dispatcher.schedule_next_segment_for_type(MediaType::Video, None);
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
        let pl_store = dispatcher.playlist_store.as_ref().unwrap();
        let current = pl_store.current_variant_id().unwrap();
        let playlist_id = *pl_store
            .current_media_playlist_id(MediaType::Video)
            .unwrap();
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        assert_ne!(recommendation.best_variant_id, current);
        assert_eq!(pl_store.current_variant_id(), Some(current));
        assert_eq!(
            pl_store.current_media_playlist_id(MediaType::Video),
            Some(&playlist_id)
        );
    }

    #[test]
    fn scheduling_and_pending_checks_use_the_recommended_best() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let recommendation = dispatcher.compute_optimal_variant().unwrap();
        let pl_store = dispatcher.playlist_store.as_mut().unwrap();
        let previous_variant = pl_store.current_variant_id().unwrap();
        let best_id = pl_store
            .resolve_playlist_id_for_variant(
                pl_store.variant(recommendation.best_variant_id).unwrap(),
                MediaType::Video,
            )
            .unwrap()
            .unwrap();
        pl_store
            .update_media_playlist(
                &best_id,
                b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nbest-seg.ts\n#EXT-X-ENDLIST\n"
                    .as_slice(),
                Url::new("https://example.com/best.m3u8".into()),
            )
            .unwrap();
        let expected_url = Url::new("https://example.com/best-seg.ts".into());
        dispatcher.requester.lock_segment_requests();
        dispatcher.schedule_next_segment_for_type(MediaType::Video, Some(recommendation));
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
        // A pending check must use the ABR pair, even if the stored choice differs.
        dispatcher
            .playlist_store
            .as_mut()
            .unwrap()
            .set_variant(previous_variant);
        dispatcher
            .media_element_ref
            .on_observation(MediaObservation::new(
                PlaybackTickReason::RegularInterval,
                0.,
                4,
                JsTimeRanges::new(vec![0., 40.]),
                false,
                false,
                false,
                f64::MAX,
                None,
                Some(JsTimeRanges::new(vec![0., 40.])),
            ));
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
        let pl_store = dispatcher.playlist_store.as_mut().unwrap();
        let ids: Vec<_> = pl_store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        let safe_variant = ids[1];
        let best_variant = ids[2];
        let safe_id = pl_store
            .resolve_playlist_id_for_variant(
                pl_store.variant(safe_variant).unwrap(),
                MediaType::Video,
            )
            .unwrap()
            .unwrap();
        let mut text =
            String::from("#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"safe-init.mp4\"\n");
        for number in 0..10 {
            text.push_str(&format!("#EXTINF:4,\nsegment-{number}.m4s\n"));
        }
        pl_store
            .update_media_playlist(
                &safe_id,
                text.as_bytes(),
                Url::new("https://example.com/safe.m3u8".into()),
            )
            .unwrap();
        pl_store.set_variant(best_variant);
        let best =
            segment_selection_candidate_for_variant(Some(best_variant), pl_store, MediaType::Video)
                .unwrap();
        let safe =
            segment_selection_candidate_for_variant(Some(safe_variant), pl_store, MediaType::Video)
                .unwrap();
        let low_context =
            segment_selection_candidate_for_variant(Some(ids[0]), &pl_store, MediaType::Video)
                .unwrap()
                .context;
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
            .get_mut(crate::bindings::MediaType::Video)
            .most_needed_segment(best, safe, &inventory);
        let needed = selected.needed_segments().unwrap();
        assert_eq!(needed.variant_id(), safe_variant);
        assert_eq!(needed.media_segment().unwrap().0.start(), 8.);
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
        let pl_store = dispatcher.playlist_store.as_ref().unwrap();
        let safe_variant = pl_store.available_variants().nth(1).unwrap().id();
        let id = pl_store
            .resolve_playlist_id_for_variant(
                pl_store.variant(safe_variant).unwrap(),
                MediaType::Video,
            )
            .unwrap()
            .unwrap();
        let request = crate::requester::PlaylistFileType::MediaPlaylist {
            id,
            media_type: MediaType::Video,
        };
        assert!(dispatcher.is_obsolete_playlist_request(&request));
        dispatcher
            .playlist_refresh_manager
            .follow(MediaType::Video, id);
        assert!(!dispatcher.is_obsolete_playlist_request(&request));
    }

    #[test]
    fn segment_failure_checks_its_own_playlist_after_best_changes() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(4), 30.);
        let pl_store = dispatcher.playlist_store.as_mut().unwrap();
        let ids: Vec<_> = pl_store
            .available_variants()
            .map(|variant| variant.id())
            .collect();
        let safe_id = pl_store
            .resolve_playlist_id_for_variant(pl_store.variant(ids[1]).unwrap(), MediaType::Video)
            .unwrap()
            .unwrap();
        pl_store.update_media_playlist(&safe_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:100\n#EXTINF:4,\nsegment-100.ts\n".as_slice(),
            Url::new("https://example.com/safe.m3u8".into())).unwrap();
        pl_store.set_variant(ids[2]);
        let context = crate::dispatcher::segment_request_contexts::PendingSegmentRequest::Media {
            variant_id: ids[1],
            media_type: MediaType::Video,
            init_segment_id: None,
            quality_context: segment_selection_candidate_for_variant(
                Some(ids[1]),
                &pl_store,
                MediaType::Video,
            )
            .unwrap()
            .context,
            sequence_number: 100,
            discontinuity_sequence: 0,
        };
        assert!(!crate::dispatcher::utils::is_stale_segment_request_context(
            Some(pl_store),
            &context
        ));
        pl_store.update_media_playlist(&safe_id,
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:101\n#EXTINF:4,\nsegment-101.ts\n".as_slice(),
            Url::new("https://example.com/safe.m3u8".into())).unwrap();
        assert!(crate::dispatcher::utils::is_stale_segment_request_context(
            Some(pl_store),
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
            .available_variants()
            .nth(2)
            .unwrap()
            .id();

        for _ in 0..6 {
            let selection = dispatcher.compute_optimal_variant().unwrap();
            let variant_id = selection.best_variant_id;
            assert_eq!(variant_id, high_id);
            assert_ne!(variant_id, selection.safe_variant_id);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.set_variant(variant_id);
            assert!(!pl_store.has_loaded_media_playlist(MediaType::Video));
            assert_eq!(
                pl_store
                    .current_media_playlists()
                    .find_map(|(media_type, playlist)| {
                        (media_type == MediaType::Video).then(|| playlist.target_duration())
                    }),
                None
            );
            assert_eq!(
                pl_store.max_observed_target_duration(MediaType::Video),
                Some(4.)
            );
        }
    }

    #[test]
    fn adaptive_selection_stays_stable_with_different_observed_target_durations() {
        let mut dispatcher = dispatcher_with_abr_playlists(Some(6), 16.5);
        let pl_store = dispatcher.playlist_store.as_ref().unwrap();
        let medium_id = pl_store.available_variants().nth(1).unwrap().id();
        assert_eq!(
            pl_store
                .current_media_playlists()
                .find_map(|(media_type, playlist)| {
                    (media_type == MediaType::Video).then(|| playlist.target_duration())
                }),
            Some(4.)
        );
        assert_eq!(
            pl_store.max_observed_target_duration(MediaType::Video),
            Some(6.)
        );

        for _ in 0..6 {
            let variant_id = dispatcher
                .compute_optimal_variant()
                .unwrap()
                .best_variant_id;
            assert_eq!(variant_id, medium_id);
            let pl_store = dispatcher.playlist_store.as_mut().unwrap();
            pl_store.set_variant(variant_id);
            assert_eq!(
                pl_store
                    .current_media_playlists()
                    .find_map(|(media_type, playlist)| {
                        (media_type == MediaType::Video).then(|| playlist.target_duration())
                    }),
                Some(4.)
            );
            assert_eq!(
                pl_store.max_observed_target_duration(MediaType::Video),
                Some(6.)
            );
        }
    }

    #[test]
    fn abr_reference_duration_combines_audio_video_and_excludes_text() {
        let mut store = selected_playlist(
            b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Main\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"text\",NAME=\"Main\",DEFAULT=YES,AUTOSELECT=YES,URI=\"text.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"audio\",SUBTITLES=\"text\"\nvideo.m3u8\n",
        );
        for (media_type, duration) in [
            (MediaType::Audio, 6),
            (MediaType::Video, 4),
            (MediaType::Text, 100),
        ] {
            let id = *store.current_media_playlist_id(media_type).unwrap();
            let media = format!("#EXTM3U\n#EXT-X-TARGETDURATION:{duration}\n#EXTINF:{duration},\nsegment\n#EXT-X-ENDLIST\n");
            store
                .update_media_playlist(
                    &id,
                    media.as_bytes(),
                    Url::new("https://example.com/media.m3u8".into()),
                )
                .unwrap();
        }
        let dispatcher = dispatcher_with_buffer_observation(None, Some(vec![0., 10.]));
        assert_eq!(
            store.max_observed_target_duration(MediaType::Text),
            Some(100.)
        );
        assert_eq!(
            dispatcher
                .playback_conditions(&store)
                .abr_reference_segment_duration,
            Some(6.)
        );
    }

    #[test]
    fn playback_bounds_intersect_loaded_audio_video_and_ignore_text() {
        use crate::dispatcher::utils::{
            estimated_playback_maximum_position, estimated_playback_minimum_position,
        };
        let mut store = selected_playlist(
            b"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Main\",DEFAULT=YES,URI=\"audio.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"text\",NAME=\"Main\",DEFAULT=YES,AUTOSELECT=YES,URI=\"text.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS=\"avc1.42E01E,mp4a.40.2\",AUDIO=\"audio\",SUBTITLES=\"text\"\nvideo.m3u8\n",
        );
        assert_eq!(estimated_playback_minimum_position(&store), None);
        assert_eq!(estimated_playback_maximum_position(&store), None);
        let base = 1_704_164_640.; // 2024-01-02T03:04:00Z
        for (media_type, start, duration, expected) in [
            (MediaType::Text, 9, 1, None),
            (MediaType::Audio, 7, 12, Some((7., 19.))),
            (MediaType::Video, 5, 10, Some((7., 15.))),
        ] {
            let id = *store.current_media_playlist_id(media_type).unwrap();
            let media = format!("#EXTM3U\n#EXT-X-TARGETDURATION:{duration}\n#EXT-X-PROGRAM-DATE-TIME:2024-01-02T03:04:{start:02}.000Z\n#EXTINF:{duration},\nsegment\n#EXT-X-ENDLIST\n");
            store
                .update_media_playlist(
                    &id,
                    media.as_bytes(),
                    Url::new("https://example.com/media.m3u8".into()),
                )
                .unwrap();
            assert_eq!(
                estimated_playback_minimum_position(&store),
                expected.map(|(start, _)| base + start)
            );
            assert_eq!(
                estimated_playback_maximum_position(&store),
                expected.map(|(_, end)| base + end)
            );
        }
    }

    #[test]
    fn abr_normalization_rejects_incompatible_audio_instead_of_treating_it_as_absent() {
        let mut store = selected_playlist(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="en"
en-video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="fr"
fr-video.m3u8
"#,
        );
        let variants: Vec<_> = store.available_variants().map(|v| v.id()).collect();
        let french = store
            .audio_tracks()
            .iter()
            .find(|track| track.language() == Some("fr"))
            .unwrap()
            .id();
        store.set_audio_track(Some(french)).unwrap();
        store.set_variant(variants[1]);
        let recommendation = crate::adaptive::AdaptiveVariantSelection {
            best_variant_id: variants[0],
            safe_variant_id: variants[1],
        };
        let selection =
            normalize_abr_selection(&store, Some(recommendation), MediaType::Video).unwrap();
        assert_eq!(selection.best_variant_id, variants[1]);
        let invalid_safe = crate::adaptive::AdaptiveVariantSelection {
            best_variant_id: variants[1],
            safe_variant_id: variants[0],
        };
        assert!(normalize_abr_selection(&store, Some(invalid_safe), MediaType::Video).is_none());
        assert!(segment_selection_candidate_for_variant(
            Some(variants[0]),
            &store,
            MediaType::Audio
        )
        .is_none());
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
