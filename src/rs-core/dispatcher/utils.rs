use super::{segment_request_contexts::PendingSegmentRequest, StartingPosition};
use crate::{
    bindings::{
        jsSendContentCompatibilityError, jsSendMultivariantPlaylistParsingError,
        jsSendSegmentParsingError, ContentCompatibilityErrorCode, MediaType,
        MultivariantPlaylistParsingErrorCode, StartingPositionType,
    },
    playlist_store::{
        PlaylistStore, PlaylistStoreError, ProbeSegmentContext, ProbeSegmentMetadata,
    },
};

pub(super) fn was_last_segment(
    playlist_store: Option<&PlaylistStore>,
    media_type: MediaType,
    seg_start: f64,
) -> bool {
    playlist_store.is_some_and(|c| c.is_last_media_segment(media_type, seg_start))
}

pub(super) fn has_playlist_store_media_type(
    playlist_store: Option<&PlaylistStore>,
) -> Option<MediaType> {
    let playlist_store = playlist_store?;
    if playlist_store.has_distinct_media_type(MediaType::Video) {
        Some(MediaType::Video)
    } else if playlist_store.has_distinct_media_type(MediaType::Audio) {
        Some(MediaType::Audio)
    } else {
        None
    }
}

/// Returns the expected position at which we want to start playback, in seconds, according to
/// both configuration and playlist metadata.
pub(super) fn get_initial_position(
    playlist_store: &PlaylistStore,
    starting_position: Option<StartingPosition>,
) -> f64 {
    if let Some(starting_pos) = starting_position {
        match starting_pos.start_type {
            StartingPositionType::Absolute => starting_pos.position,
            StartingPositionType::FromBeginning => {
                playlist_store
                    .current_estimated_minimum_position()
                    .unwrap_or(0.)
                    + starting_pos.position
            }
            StartingPositionType::FromEnd => playlist_store
                .current_estimated_maximum_position()
                .map(|max| max - starting_pos.position)
                .unwrap_or(playlist_store.expected_start_time()),
        }
    } else {
        playlist_store.expected_start_time()
    }
}

pub(super) fn is_stale_segment_request_context(
    playlist_store: Option<&PlaylistStore>,
    context: &PendingSegmentRequest,
) -> bool {
    match context {
        PendingSegmentRequest::Media {
            playlist_id,
            sequence_number,
            ..
        } => playlist_store.as_ref().is_some_and(|pl_store| {
            pl_store
                .media_playlist_by_id(playlist_id)
                .is_some_and(|playlist| !playlist.contains_sequence(*sequence_number))
        }),

        PendingSegmentRequest::Probe {
            requested_media_type,
            probe_segment:
                ProbeSegmentMetadata {
                    context: ProbeSegmentContext::Media { sequence, .. },
                    ..
                },
        } => match requested_media_type {
            Some(media_type) => playlist_store.is_some_and(|pl_store| {
                pl_store.has_loaded_media_playlist(*media_type)
                    && !pl_store.loaded_playlist_contains_sequence(*media_type, *sequence)
            }),
            None => playlist_store
                .and_then(|pl_store| pl_store.direct_media_playlist())
                .is_some_and(|(_, playlist)| !playlist.contains_sequence(*sequence)),
        },
        _ => false,
    }
}

pub(super) fn handle_playlist_store_error(err: PlaylistStoreError) {
    match err {
        PlaylistStoreError::NoSupportedVariant => jsSendContentCompatibilityError(
            true,
            ContentCompatibilityErrorCode::NoSupportedVariant,
            &err.to_string(),
        ),
        PlaylistStoreError::NoInitialVariant => jsSendMultivariantPlaylistParsingError(
            true,
            MultivariantPlaylistParsingErrorCode::MultivariantPlaylistWithoutVariant,
            &err.to_string(),
        ),
        PlaylistStoreError::NoProbeSegment => jsSendSegmentParsingError(
            true,
            crate::bindings::SegmentParsingErrorCode::UnknownError,
            None,
            &err.to_string(),
        ),
        PlaylistStoreError::UnsupportedStartupStream => jsSendContentCompatibilityError(
            true,
            ContentCompatibilityErrorCode::NoSupportedVariant,
            &err.to_string(),
        ),
    }
}
