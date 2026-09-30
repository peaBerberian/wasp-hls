use crate::{
    adaptive::AdaptiveQualitySelector,
    bindings::MediaType,
    dispatcher::playlist_refresh_timers::PlaylistRefreshTimers,
    media_element::MediaElementReference,
    parser::AudioTrack,
    playlist_store::{PlaylistStore, ProbeSegmentMetadata},
    requester::Requester,
    segment_selector::NextSegmentSelectors,
};

mod api;
mod core;
mod event_listeners;
mod playlist_refresh_timers;
mod segment_request_contexts;
mod utils;

use segment_request_contexts::SegmentRequestContexts;

pub(crate) use crate::bindings::{MediaSourceReadyState, PlaybackTickReason, StartingPositionType};
pub(crate) use event_listeners::{JsMemoryBlob, JsTimeRanges, MediaObservation};

/// The `Dispatcher` is the player Interface exported to the JavaScript-side,
/// providing an API to load contents and influence various parameters about playback.
pub struct Dispatcher {
    /// Current `PlayerReadyState` the `Dispatcher` is in.
    ready_state: PlayerReadyState,

    /// Allows to perform actions related to the HTMLMediaElement on the page, like buffering media,
    /// pausing, seeking etc.
    media_element_ref: MediaElementReference,

    /// Struct allowing to obtain estimate of the optimal variants to play,
    /// mostly based on network metrics.
    adaptive_selector: AdaptiveQualitySelector,

    /// Store the "Top level Playlist" (structure which describes the currently
    /// loaded content) alongside some state to keep track of the chosen... tracks.
    /// (More technically of variants and media streams).
    ///
    /// `None` if no "Top level Playlist" has been loaded yet.
    playlist_store: Option<PlaylistStore>,

    /// Abstraction allowing to perform playlist and segment requests, while
    /// easily monitoring requests that are pending.
    requester: Requester,

    /// Amount of buffer, ahead of the current position we want to build in seconds.
    /// Once we reached that point, we won't try to load load new segments.
    ///
    /// This can for example be used to limit memory and network bandwidth usage.
    buffer_goal: f64,

    /// The last known current position stored.
    /// Changes periodically and immediately on various time-changing events (such as seeks, stops
    /// etc.)
    last_position: f64,

    /// Abstraction allowing to know which is the next segment to request.
    segment_selectors: NextSegmentSelectors,

    /// Current set-up timers to notify about a needed playlist refresh, associated to the playlist
    /// that needs to be refreshed.
    playlist_refresh_timers: PlaylistRefreshTimers,

    /// Stores data on pending requests linked to init or media segments.
    /// Allowing to retreive them once finished.
    segment_request_contexts: SegmentRequestContexts,

    /// Startup probe segments that have already been fetched and inspected and now only wait for
    /// the regular buffering pipeline to be ready before being pushed.
    ready_probe_segments: ReadyProbeSegments,

    /// Preferred criteria to resolve the initial audio track selection for the
    /// next content being loaded.
    initial_audio_track_selection: Vec<InitialAudioTrackSelection>,
}

#[derive(Clone, Debug, Default)]
pub struct InitialAudioTrackSelection {
    pub language: Option<String>,
    pub assoc_language: Option<String>,
    pub name: Option<String>,
    pub characteristics: Vec<String>,
    pub channels: Option<u32>,
}

impl InitialAudioTrackSelection {
    fn is_empty(&self) -> bool {
        self.language.is_none()
            && self.assoc_language.is_none()
            && self.name.is_none()
            && self.characteristics.is_empty()
            && self.channels.is_none()
    }

    fn matches(&self, track: &AudioTrack) -> bool {
        self.language
            .as_deref()
            .map(|language| track.language() == Some(language))
            .unwrap_or(true)
            && self
                .assoc_language
                .as_deref()
                .map(|assoc_language| track.assoc_language() == Some(assoc_language))
                .unwrap_or(true)
            && self
                .name
                .as_deref()
                .map(|name| track.name() == name)
                .unwrap_or(true)
            && self
                .channels
                .map(|channels| track.channels() == Some(channels))
                .unwrap_or(true)
            && self.characteristics.iter().all(|characteristic| {
                track
                    .characteristics()
                    .iter()
                    .any(|track_characteristic| track_characteristic == characteristic)
            })
    }
}

/// Identify the playback-related state the `Dispatcher` is in.
#[derive(Clone, Debug)]
enum PlayerReadyState {
    /// No content is currently loaded.
    Stopped,

    /// We're preparing a content's playlist base information
    /// Appears after `Stopped` and before `AwaitingMediaSource`.
    AwaitingPlaylistInfo {
        /// The position we should start at once playback begins.
        starting_position: Option<StartingPosition>,
        /// Set to `true` once the callback linked to transmiting the current playlist
        /// information (audio tracks, playlist metadata) has been called. Prevent sending
        /// it more than once.
        lifecycle_announced: bool,
    },

    /// We're creating a `MediaSource` and the corresponding buffers.
    /// Appears after `AwaitingPlaylistInfo` and before `AwaitingSegments`.
    AwaitingMediaSource {
        /// The position we should start at once playback begins.
        starting_position: Option<StartingPosition>,
    },

    /// The SourceBuffers are all ready but currently awaiting segments before
    /// being aple to play.
    /// Appears after `AwaitingMediaSource` and before `Playing`.
    AwaitingSegments,

    /// The content has enough segments to play.
    /// Note that this does not mean the media element is currently playing content:
    /// it can still be paused or at a `0` playback rate.
    /// Appears after `AwaitingSegments`.
    Playing,
}

#[derive(Clone, Copy, Debug)]
pub struct StartingPosition {
    start_type: StartingPositionType,
    position: f64,
}

impl StartingPosition {
    pub fn new(start_type: StartingPositionType, position: f64) -> Self {
        Self {
            start_type,
            position,
        }
    }
}

#[derive(Debug)]
struct ReadyProbeSegment {
    request: ProbeSegmentMetadata,
    media_type: MediaType,
    data: event_listeners::JsMemoryBlob,
}

#[derive(Default)]
struct ReadyProbeSegments {
    audio: Option<ReadyProbeSegment>,
    video: Option<ReadyProbeSegment>,
}

impl ReadyProbeSegments {
    fn get(&self, media_type: MediaType) -> Option<&ReadyProbeSegment> {
        match media_type {
            MediaType::Audio => self.audio.as_ref(),
            MediaType::Video => self.video.as_ref(),
        }
    }

    fn insert(&mut self, probe: ReadyProbeSegment) {
        match probe.media_type {
            MediaType::Audio => self.audio = Some(probe),
            MediaType::Video => self.video = Some(probe),
        }
    }

    fn take(&mut self, media_type: MediaType) -> Option<ReadyProbeSegment> {
        match media_type {
            MediaType::Audio => self.audio.take(),
            MediaType::Video => self.video.take(),
        }
    }

    fn clear_media_type(&mut self, media_type: MediaType) {
        match media_type {
            MediaType::Audio => self.audio = None,
            MediaType::Video => self.video = None,
        }
    }

    fn clear(&mut self) {
        self.audio = None;
        self.video = None;
    }
}

#[cfg(test)]
mod tests {
    use super::InitialAudioTrackSelection;
    use crate::{parser::TopLevelPlaylist, utils::url::Url};

    #[test]
    fn initial_audio_track_selection_matches_language() {
        let playlist = TopLevelPlaylist::parse(
            r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French",LANGUAGE="fr",AUTOSELECT=YES,URI="audio-fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#
            .as_bytes(),
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let TopLevelPlaylist::Multivariant(playlist) = playlist else {
            unreachable!();
        };

        let selection = InitialAudioTrackSelection {
            language: Some("fr".to_string()),
            ..Default::default()
        };

        let matched = playlist
            .audio_tracks()
            .iter()
            .find(|track| selection.matches(track))
            .unwrap();
        assert_eq!(matched.language(), Some("fr"));
    }

    #[test]
    fn initial_audio_track_selection_matches_characteristics_and_channels() {
        let playlist = TopLevelPlaylist::parse(
            r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French AD",LANGUAGE="fr",ASSOC-LANGUAGE="fr",AUTOSELECT=YES,CHANNELS="6",CHARACTERISTICS="public.accessibility.describes-video",URI="audio-fr-ad.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#
            .as_bytes(),
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let TopLevelPlaylist::Multivariant(playlist) = playlist else {
            unreachable!();
        };

        let selection = InitialAudioTrackSelection {
            language: Some("fr".to_string()),
            assoc_language: Some("fr".to_string()),
            characteristics: vec!["public.accessibility.describes-video".to_string()],
            channels: Some(6),
            ..Default::default()
        };

        let matched = playlist
            .audio_tracks()
            .iter()
            .find(|track| selection.matches(track))
            .unwrap();
        assert_eq!(matched.name(), "French AD");
    }

    #[test]
    fn initial_audio_track_selection_prefers_first_matching_entry() {
        let playlist = TopLevelPlaylist::parse(
            r#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French stereo",LANGUAGE="fr",CHANNELS="2",AUTOSELECT=YES,URI="audio-fr.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aud",NAME="French surround",LANGUAGE="fr",CHANNELS="6",AUTOSELECT=YES,URI="audio-fr-6.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,AUDIO="aud"
video.m3u8
"#
            .as_bytes(),
            Url::new("https://example.com/master.m3u8".to_string()),
        )
        .unwrap();
        let TopLevelPlaylist::Multivariant(playlist) = playlist else {
            unreachable!();
        };

        let selections = [
            InitialAudioTrackSelection {
                language: Some("fr".to_string()),
                channels: Some(8),
                ..Default::default()
            },
            InitialAudioTrackSelection {
                language: Some("fr".to_string()),
                channels: Some(6),
                ..Default::default()
            },
        ];

        let matched = selections
            .iter()
            .find_map(|selection| {
                playlist
                    .audio_tracks()
                    .iter()
                    .find(|track| selection.matches(track))
            })
            .unwrap();
        assert_eq!(matched.name(), "French surround");
    }
}
