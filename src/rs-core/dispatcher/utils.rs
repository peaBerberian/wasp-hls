use super::{segment_request_contexts::PendingSegmentRequest, StartingPosition};
use crate::{
    bindings::{
        jsSendContentCompatibilityError, jsSendMediaPlaylistParsingError,
        jsSendMultivariantPlaylistParsingError, jsSendSegmentParsingError,
        ContentCompatibilityErrorCode, MediaType, MultivariantPlaylistParsingErrorCode,
        StartingPositionType,
    },
    parser::TopLevelPlaylistParsingError,
    playlist_store::{
        PlaylistStore, PlaylistStoreError, ProbeSegmentContext, ProbeSegmentMetadata,
    },
};

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

/// Estimated playback start: the latest beginning among selected, loaded audio/video playlists.
pub(super) fn estimated_playback_minimum_position(playlist_store: &PlaylistStore) -> Option<f64> {
    playlist_store
        .current_media_playlists()
        .filter(|(media_type, _)| matches!(media_type, MediaType::Audio | MediaType::Video))
        .filter_map(|(_, playlist)| playlist.beginning())
        .reduce(f64::max)
}

/// Estimated playback end: the earliest ending among selected, loaded audio/video playlists.
pub(super) fn estimated_playback_maximum_position(playlist_store: &PlaylistStore) -> Option<f64> {
    playlist_store
        .current_media_playlists()
        .filter(|(media_type, _)| matches!(media_type, MediaType::Audio | MediaType::Video))
        .filter_map(|(_, playlist)| playlist.ending())
        .reduce(f64::min)
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
                estimated_playback_minimum_position(playlist_store).unwrap_or(0.)
                    + starting_pos.position
            }
            StartingPositionType::FromEnd => estimated_playback_maximum_position(playlist_store)
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
            quality_context,
            sequence_number,
            ..
        } => playlist_store.as_ref().is_some_and(|pl_store| {
            pl_store
                .media_playlist(&quality_context.playlist_id())
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
        PlaylistStoreError::Parsing(err) => {
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
        }
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
        PlaylistStoreError::UnsupportedStartupStream | PlaylistStoreError::TextOnlyMainContent => {
            jsSendContentCompatibilityError(
                true,
                ContentCompatibilityErrorCode::NoSupportedVariant,
                &err.to_string(),
            )
        }
    }
}
