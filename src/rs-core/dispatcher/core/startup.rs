use super::super::segment_request_contexts::PendingSegmentRequest;
use super::{
    sync_media_source_duration, utils, Dispatcher, MediaPlaylistUpdate, MediaPlaylistUpdateMode,
    PlayerReadyState, ReadyProbeSegment,
};
use crate::media_element::SegmentPushMetadata;
use crate::playlist_store::ProbeSegmentMetadata;
use crate::{
    bindings::{
        formatters::{
            format_audio_tracks_for_js, format_direct_media_audio_tracks_for_js,
            format_direct_media_variants_info_for_js, format_source_buffer_creation_err_for_js,
            format_variants_info_for_js, DIRECT_MEDIA_AUDIO_TRACK_ID, DIRECT_MEDIA_VARIANT_ID,
        },
        jsAnnounceFetchedContent, jsAnnounceTrackUpdate, jsAnnounceVariantUpdate, jsSendOtherError,
        jsSendSegmentParsingError, jsSendSourceBufferCreationError, jsStartObservingPlayback,
        MediaSourceReadyState, MediaType, OtherErrorCode, PlaylistType,
    },
    media_element::SourceBufferCreationError,
    playlist_store::{ProbeSegmentContext, StartupStatus},
    requester::{PlaylistFileType, RequestLaneTag},
};

impl Dispatcher {
    /// Look if progress can be made in playing the current content by checking the full state
    pub(super) fn recheck_player_state(&mut self) {
        match self.ready_state {
            PlayerReadyState::Stopped => {}
            PlayerReadyState::AwaitingPlaylistInfo { .. } => {
                advance_awaiting_playlist_info_state(self)
            }
            PlayerReadyState::AwaitingMediaSource { .. } => {
                advance_awaiting_media_source_state(self)
            }
            PlayerReadyState::AwaitingSegments | PlayerReadyState::Playing => {
                self.check_segments_to_request();
            }
        };
    }

    pub(super) fn ready_to_load_segments(&mut self) -> bool {
        ensure_ready_playlists(self, self.media_element_ref.wanted_position())
    }
}

fn ensure_ready_playlists(dispatcher: &mut Dispatcher, wanted_position: f64) -> bool {
    let Some(playlist_store) = dispatcher.playlist_store.as_mut() else {
        return false;
    };
    let status = match playlist_store.startup_status(wanted_position) {
        Ok(status) => status,
        Err(err) => {
            utils::handle_playlist_store_error(err);
            dispatcher.stop_current_content();
            return false;
        }
    };

    match status {
        StartupStatus::Ready => true,
        StartupStatus::AwaitingPlaylists => false,
        StartupStatus::AwaitingSupportCheck => false,
        StartupStatus::VariantSelectionNeeded => {
            let Some((variant_id, allow_fast_switching)) = dispatcher.compute_optimal_variant()
            else {
                // NOTE: Should normally never happen, we should have ensured this was not possible
                // before
                jsSendOtherError(
                    true,
                    OtherErrorCode::Unknown,
                    "Unable to select a variant during startup",
                );
                dispatcher.stop_current_content();
                return false;
            };
            let Some(playlist_store) = dispatcher.playlist_store.as_mut() else {
                return false;
            };
            let changed_media_types = playlist_store.set_variant(variant_id);
            for media_type in [MediaType::Audio, MediaType::Video] {
                dispatcher
                    .segment_selectors
                    .set_fast_quality_switching(media_type, allow_fast_switching);
            }
            jsAnnounceVariantUpdate(Some(variant_id));
            let updates =
                changed_media_types
                    .iter()
                    .copied()
                    .map(|media_type| MediaPlaylistUpdate {
                        media_type,
                        mode: MediaPlaylistUpdateMode::Seamless,
                    });
            dispatcher.handle_media_playlist_updates(updates);
            false
        }
        StartupStatus::NeedsProbes(probe_metadata) => {
            let error_media_type = if probe_metadata.len() == 1 {
                probe_metadata[0].1
            } else {
                None
            };

            if probe_metadata
                .into_iter()
                .all(|probe_request| start_probe_segment_request(dispatcher, probe_request))
            {
                false
            } else {
                jsSendSegmentParsingError(
                    true,
                    crate::bindings::SegmentParsingErrorCode::UnknownError,
                    error_media_type,
                    "No probe segment was available to determine startup metadata",
                );
                dispatcher.stop_current_content();
                false
            }
        }
    }
}

/// Try to progress `ready_state` when in the `AwaitingPlaylistInfo` state
fn advance_awaiting_playlist_info_state(dispatcher: &mut Dispatcher) {
    let (starting_position, lifecycle_announced) = match &dispatcher.ready_state {
        PlayerReadyState::AwaitingPlaylistInfo {
            starting_position,
            lifecycle_announced,
        } => (*starting_position, *lifecycle_announced),
        _ => return,
    };

    let wanted_position = {
        let Some(playlist_store) = dispatcher.playlist_store.as_mut() else {
            return;
        };
        utils::get_initial_position(playlist_store, starting_position)
    };

    // Start fetching initial media playlists if needed
    {
        let Some(playlist_store) = dispatcher.playlist_store.as_mut() else {
            return;
        };
        for mt in [MediaType::Video, MediaType::Audio] {
            if playlist_store.has_loaded_media_playlist(mt) {
                continue; // Already fetched
            }
            if let Some(id) = playlist_store.media_playlist_id_for(mt) {
                if let Some(url) = playlist_store.media_playlist_url(id) {
                    let url = url.clone();
                    let playlist_type = PlaylistFileType::MediaPlaylist {
                        id: *id,
                        media_type: mt,
                    };
                    if !dispatcher
                        .requester
                        .is_requesting_playlist(&url, &playlist_type)
                    {
                        dispatcher.requester.fetch_playlist(url, playlist_type);
                    }
                }
            }
        }
    }

    if !lifecycle_announced {
        let (
            playlist_kind,
            variants_info,
            audio_tracks_info,
            curr_variant,
            curr_audio_track,
            is_fixed,
        ) = {
            let Some(playlist_store) = dispatcher.playlist_store.as_mut() else {
                return;
            };

            // SAFETY: The following lines are unsafe because they may actually define raw pointers
            // to point to Rust's heap memory and put it in the returned values.
            //
            // However, we're calling the JS binding function it is communicated to directly
            // after and thus before the corresponding underlying data had a chance to be
            // dropped.
            //
            // Because one of the rules of those bindings is to copy all pointed data
            // synchronously on call, we should not encounter any issue.
            let (variants_info, audio_tracks_info) = if playlist_store.playlist_kind()
                == PlaylistType::MediaPlaylist
            {
                let has_audio_track = playlist_store.has_distinct_media_type(MediaType::Audio);
                (
                    unsafe { format_direct_media_variants_info_for_js() },
                    unsafe { format_direct_media_audio_tracks_for_js(has_audio_track) },
                )
            } else {
                (
                    unsafe {
                        format_variants_info_for_js(playlist_store.available_variants().as_slice())
                    },
                    unsafe { format_audio_tracks_for_js(playlist_store.audio_tracks()) },
                )
            };
            let fixed_audio_track = playlist_store.fixed_audio_track_id();
            let is_fixed = fixed_audio_track.is_some();
            let curr_audio_track = if let Some(fixed) = fixed_audio_track {
                Some(fixed)
            } else if playlist_store.playlist_kind() == PlaylistType::MediaPlaylist
                && playlist_store.has_distinct_media_type(MediaType::Audio)
            {
                Some(DIRECT_MEDIA_AUDIO_TRACK_ID)
            } else {
                playlist_store.current_audio_track_id()
            };
            let curr_variant = if playlist_store.playlist_kind() == PlaylistType::MediaPlaylist {
                Some(DIRECT_MEDIA_VARIANT_ID)
            } else {
                playlist_store.current_variant_id()
            };

            (
                playlist_store.playlist_kind(),
                variants_info,
                audio_tracks_info,
                curr_variant,
                curr_audio_track,
                is_fixed,
            )
        };

        jsAnnounceFetchedContent(playlist_kind, variants_info, audio_tracks_info);
        jsAnnounceVariantUpdate(curr_variant);
        jsAnnounceTrackUpdate(MediaType::Audio, curr_audio_track, is_fixed);

        dispatcher.ready_state = PlayerReadyState::AwaitingPlaylistInfo {
            starting_position,
            lifecycle_announced: true,
        };
    }

    if !ensure_ready_playlists(dispatcher, wanted_position) {
        return;
    }

    dispatcher.ready_state = PlayerReadyState::AwaitingMediaSource { starting_position };
    dispatcher.recheck_player_state();
}

/// Try to progress `ready_state` when in the `AwaitingMediaSource` state
fn advance_awaiting_media_source_state(dispatcher: &mut Dispatcher) {
    let starting_pos = match (
        &dispatcher.ready_state,
        dispatcher.media_element_ref.media_source_ready_state(),
    ) {
        (_, Some(MediaSourceReadyState::Closed) | None) => {
            return;
        }
        (PlayerReadyState::AwaitingMediaSource { starting_position }, _) => *starting_position,
        _ => {
            return;
        }
    };

    let Some(playlist_store) = dispatcher.playlist_store.as_ref() else {
        return;
    };

    let wanted_start = utils::get_initial_position(playlist_store, starting_pos);
    if wanted_start > 0. {
        dispatcher.media_element_ref.seek(wanted_start);
    }

    dispatcher.ready_state = PlayerReadyState::AwaitingSegments;
    sync_media_source_duration(playlist_store);

    if let Some(Err(e)) = init_source_buffer(dispatcher, MediaType::Audio) {
        let (code, msg) = format_source_buffer_creation_err_for_js(e);
        jsSendSourceBufferCreationError(true, code, MediaType::Audio, &msg);
        dispatcher.stop_current_content();
        return;
    }
    if let Some(Err(e)) = init_source_buffer(dispatcher, MediaType::Video) {
        let (code, msg) = format_source_buffer_creation_err_for_js(e);
        jsSendSourceBufferCreationError(true, code, MediaType::Video, &msg);
        dispatcher.stop_current_content();
        return;
    }
    jsStartObservingPlayback();
    consume_probe_segments(dispatcher);
}

fn start_probe_segment_request(
    dispatcher: &mut Dispatcher,
    probe_request: (ProbeSegmentMetadata, Option<MediaType>),
) -> bool {
    let (probe_segment, requested_media_type) = probe_request;
    if requested_media_type.is_some_and(|mt| dispatcher.ready_probe_segments.get(mt).is_some())
        || dispatcher.segment_request_contexts.has(|context| {
            matches!(
                context,
                PendingSegmentRequest::Probe {
                    requested_media_type: context_media_type,
                    ..
                } if *context_media_type == requested_media_type
            )
        })
    {
        return true;
    }

    let lane_tag = requested_media_type
        .map(RequestLaneTag::from_media_type)
        .unwrap_or(RequestLaneTag::Probe);
    let req_id = dispatcher
        .segment_request_contexts
        .insert(PendingSegmentRequest::Probe {
            probe_segment: probe_segment.clone(),
            requested_media_type,
        });
    match &probe_segment.context {
        ProbeSegmentContext::Init { .. } => dispatcher.requester.request_segment_immediately(
            lane_tag,
            &probe_segment.url,
            probe_segment.byte_range.as_ref(),
            None,
            req_id,
        ),
        ProbeSegmentContext::Media { time_info, .. } => {
            dispatcher.requester.request_segment_immediately(
                lane_tag,
                &probe_segment.url,
                probe_segment.byte_range.as_ref(),
                Some(time_info.clone()),
                req_id,
            )
        }
    }
    true
}

fn init_source_buffer(
    dispatcher: &mut Dispatcher,
    media_type: MediaType,
) -> Option<Result<(), SourceBufferCreationError>> {
    let content = dispatcher.playlist_store.as_mut()?;
    if !content.has_loaded_media_playlist(media_type) {
        return None;
    }
    let mime_type = content.current_mime_type(media_type)?;
    let codec = content.current_codec(media_type)?;
    Some(
        dispatcher
            .media_element_ref
            .create_source_buffer(media_type, &mime_type, &codec),
    )
}

/// In certain conditions, a "probe segment" may be fetched initially to gather more
/// metadata on the content to play.
///
/// As it is usable data and should anyway correspond to the initial segment to load, this
/// method allows to both reset that state and to push it to the buffers.
fn consume_probe_segments(dispatcher: &mut Dispatcher) {
    for media_type in [MediaType::Audio, MediaType::Video] {
        let Some(ReadyProbeSegment {
            request,
            media_type,
            data,
        }) = dispatcher.ready_probe_segments.take(media_type)
        else {
            continue;
        };

        match request.context {
            ProbeSegmentContext::Init { id } => {
                dispatcher.on_init_segment_loaded(data, media_type, id);
            }
            ProbeSegmentContext::Media {
                sequence,
                discontinuity_sequence,
                time_info,
            } => {
                let Some((segment_list, context)) = dispatcher
                    .playlist_store
                    .as_ref()
                    .and_then(|store| store.loaded_media_playlist_segment_info(media_type))
                else {
                    jsSendOtherError(
                        true,
                        OtherErrorCode::Unknown,
                        "No media context could be resolved after probing",
                    );
                    dispatcher.stop_current_content();
                    return;
                };
                let init_segment_id = segment_list
                    .media()
                    .iter()
                    .find(|seg| seg.sequence() == sequence)
                    .and_then(|seg| segment_list.init_for(seg))
                    .map(|init| init.id());
                dispatcher.on_media_segment_loaded(SegmentPushMetadata {
                    data,
                    media_type,
                    time_info,
                    context,
                    init_segment_id,
                    sequence_number: sequence,
                    discontinuity_sequence,
                });
            }
        }
    }
}
