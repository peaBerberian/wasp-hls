use crate::{
    bindings::{
        DataSource,
        MediaType,
        jsStartObservingPlayback,
        jsStopObservingPlayback,
        jsRemoveMediaSource,
        RequestId,
        SourceBufferId,
    },
    Logger,
    content::{MediaPlaylistPermanentId, WaspHlsContent},
    parser::MultiVariantPlaylist,
    requester::{
        PlaylistFileType,
        SegmentRequestInfo,
        PlaylistRequestInfo,
        FinishedRequestType,
    },
    source_buffer::{PushMetadata, SourceBufferCreationError},
    utils::url::Url,
};
use super::{
    MediaSourceReadyState,
    WaspHlsPlayer,
    WaspHlsPlayerReadyState,
};

mod segment_queues;

pub use segment_queues::SegmentQueues;

impl WaspHlsPlayer {
    pub(crate) fn on_request_succeeded(&mut self,
        request_id: RequestId,
        data: DataSource,
        final_url: Url
    ) {
        match self.requester.remove_pending_request(request_id) {
            Some(FinishedRequestType::Segment(seg_info)) =>
                self.on_segment_fetch_success(seg_info, data),
            Some(FinishedRequestType::Playlist(pl_info)) =>
                match data {
                    DataSource::Raw(d) => self.on_playlist_fetch_success(pl_info, d, final_url),
                    _ => {
                        self.fail_on_error("Unexpected data format for the Playlist file");
                    }
                }
            _ => Logger::warn("Unknown request finished"),
        }
    }

    pub(crate) fn on_request_failed(&mut self,
        request_id: RequestId,
    ) {
        // TODO retry and whatnot
        match self.requester.remove_pending_request(request_id) {
            Some(FinishedRequestType::Segment(_seg_info)) =>
                self.fail_on_error("A segment request failed."),
            Some(FinishedRequestType::Playlist(_pl_info)) =>
                self.fail_on_error("A Playlist request failed."),
            _ => Logger::warn("Unknown request finished"),
        }
    }

    pub(super) fn fail_on_error(&mut self, error: &str) {
        Logger::error(error);
        self.internal_stop();
    }

    /// Method called on various events that could lead to start loading segments.
    /// If all conditions are met, the `WaspHlsPlayer` is set to the next
    /// `AwaitingSegments` `WaspHlsPlayerReadyState`, playback observations begin,
    /// and the potential initialization segments are requested.
    pub(super) fn check_ready_to_load_segments(&mut self) {
        if self.ready_state >= WaspHlsPlayerReadyState::AwaitingSegments ||
            self.media_source_state.is_none()
        {
            return;
        }
        let content = if let Some(ctnt) = self.content.as_ref() {
            ctnt
        } else { return; };
        if content.all_curr_media_playlists_ready() {
            self.ready_state = WaspHlsPlayerReadyState::AwaitingSegments;
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Audio) {
                self.fail_on_error(&format!("Error while creating audio SourceBuffer: {:?}", e));
                return;
            }
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Video) {
                self.fail_on_error(&format!("Error while creating video SourceBuffer: {:?}", e));
                return;
            }
            self.request_init_segment(MediaType::Video);
            self.request_init_segment(MediaType::Audio);
            jsStopObservingPlayback(self.id);
            jsStartObservingPlayback(self.id);
        }
    }

    /// Method called as a MultiVariant Playlist is loaded
    pub(super) fn on_multivariant_playlist_loaded(&mut self,
        data: Vec<u8>,
        playlist_url: Url
    ) {

        match MultiVariantPlaylist::parse(data.as_ref(), playlist_url) {
            Err(e) => {
                self.fail_on_error(format!("Error while parsing MultiVariantPlaylist: {:?}", e).as_ref());
                return;
            },
            Ok(pl) => {
                Logger::info("MultiVariant Playlist parsed successfully");
                self.content = Some(WaspHlsContent::new(pl));
                let content = self.content.as_mut().unwrap();
                if content.variants().is_empty() {
                    self.fail_on_error(
                        "Error while parsing MultiVariantPlaylist: no variant found.");
                    return;
                }

                use PlaylistFileType::*;
                let initial_variant_idx = 0; // TODO lowest/latest bandwidth first?
                content.set_curr_variant(initial_variant_idx);
                if let Some((url, id)) = content.curr_media_playlist_request_info(MediaType::Video) {
                    let id = id.clone();
                    let url = url.clone();
                    self.requester.fetch_playlist(url, MediaPlaylist { id, media_type: MediaType::Video });
                }
                if let Some((url, id)) = content.curr_media_playlist_request_info(MediaType::Audio) {
                    let id = id.clone();
                    let url = url.clone();
                    self.requester.fetch_playlist(url, MediaPlaylist { id, media_type: MediaType::Audio });
                }
            },
        }
    }

    /// Method called as a MediaPlaylist Playlist is loaded
    pub(super) fn on_media_playlist_loaded(&mut self,
        playlist_id: MediaPlaylistPermanentId,
        _media_type: MediaType,
        data: Vec<u8>,
        playlist_url: Url
    ) {
        Logger::info(&format!("Media playlist loaded successfully: {}", playlist_url.get_ref()));
        if let Some(ref mut content) = self.content {
            if let Err(e) = content.update_media_playlist(&playlist_id, data.as_ref(), playlist_url) {
                self.fail_on_error(&format!("Failed to parse MediaPlaylist: {:?}", e));
            } else if self.ready_state == WaspHlsPlayerReadyState::Loading {
                self.check_ready_to_load_segments();
            }
        } else { self.fail_on_error("Media playlist loaded but no MultiVariantPlaylist"); }
    }

    fn init_source_buffer(&mut self,
        media_type: MediaType
    ) -> Option<Result<(), SourceBufferCreationError>> {
        // TODO cleaner way than this mess
        if self.source_buffer_store.has(media_type) ||
            !self.source_buffer_store.can_still_create_source_buffer()
        {
            // TODO return Err here
            return None;
        }

        let content = if let Some(c) = &mut self.content { c } else {
            // TODO return Err here
            return None;
        };

        let media_playlist = if let Some(p) = content.curr_media_playlist(media_type) { p } else {
            return None;
        };

        let mime_type = if let Some(m) = media_playlist.mime_type(media_type) { m } else {
            // TODO return Err here
            return None;
        };

        let codecs = match content.curr_variant().map(|v| v.codecs(media_type)) {
            Some(Some(c)) => c,
            _ => {
                // TODO return Err here
                return None;
            },
        };

        Some(self.source_buffer_store.create_source_buffer(media_type, mime_type, &codecs))
    }

    pub(crate) fn internal_on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        Logger::info(&format!("MediaSource state changed: {:?}", state));
        if let Some(ref mut mtdt) = self.media_source_state {
            *mtdt = state;
        } else {
            self.media_source_state = Some(state);
        }
        if state == MediaSourceReadyState::Open {
            self.check_ready_to_load_segments();
        }
    }

    pub(crate) fn internal_on_source_buffer_update(&mut self,
        source_buffer_id: SourceBufferId
    ) {
        if let Some(ref mut sb) = self.source_buffer_store.audio {
            if sb.id == source_buffer_id {
                sb.on_update_end();
                return;
            }
        }
        if let Some(ref mut sb) = self.source_buffer_store.video {
            if sb.id == source_buffer_id {
                sb.on_update_end();
                return;
            }
        }
    }

    pub(crate) fn internal_on_source_buffer_error(&mut self,
        _source_buffer_id: SourceBufferId
    ) {
        // TODO check QuotaExceededError and so on...
        self.fail_on_error("A SourceBuffer emitted an error");
    }

    pub fn on_regular_tick(&mut self, position: f64) {
        Logger::debug(&format!("Tick received: {}", position));
        self.last_position = position;
        self.check_segments_to_request()
    }

    pub fn on_seek(&mut self, position: f64) {
        Logger::debug(&format!("Seeking tick received: {}", position));
        self.segment_queues.reset(position);

        // TODO better logic than aborting everything on seek
        self.requester.abort_segments(|_| true);
        self.check_segments_to_request()
    }

    pub fn check_segments_to_request(&mut self) {
        if !self.requester.segment_requests().iter()
            .any(|s| s.media_type == MediaType::Video)
        {
            self.request_next_segment(MediaType::Video);
        }
        if !self.requester.segment_requests().iter()
            .any(|s| s.media_type == MediaType::Audio)
        {
            self.request_next_segment(MediaType::Audio);
        }
    }

    pub fn internal_stop(&mut self) {
        Logger::info("Stopping current content (if one) and resetting player");
        self.requester.abort_all();
        jsStopObservingPlayback(self.id);
        jsRemoveMediaSource(self.id);
        self.segment_queues.reset(0.);
        self.content = None;
        self.media_source_state = None;
        self.last_position = 0.;
        self.ready_state = WaspHlsPlayerReadyState::Stopped;
    }

    /// Method called once a playlist request ended with success
    pub(super) fn on_playlist_fetch_success(&mut self,
        pl_info: PlaylistRequestInfo,
        result: Vec<u8>,
        final_url: Url
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        if let PlaylistFileType::MediaPlaylist { id, media_type } = playlist_type {
            self.on_media_playlist_loaded(id, media_type, result, final_url);
        } else {
            self.on_multivariant_playlist_loaded(result, final_url);
        }
    }

    pub(super) fn request_init_segment(&mut self, media_type: MediaType) {
        let content = if let Some(c) = self.content.as_ref() {
            c
        } else { return; };
        if let Some(init_seg_uri) = content.curr_init_segment(media_type) {
            self.requester.request_init_segment(media_type, init_seg_uri.clone());
        } else {
            Logger::debug(&format!("No {} initialization segment found", media_type));
        }
    }

    pub(super) fn request_next_segment(&mut self, media_type: MediaType) {
        let content = if let Some(c) = self.content.as_ref() {
            c
        } else { return; };

        let pl = if let Some(p) = content.curr_media_playlist(media_type) {
            p
        } else { return; };

        let segment_queue = self.segment_queues.get_mut(media_type);
        let maximum_position = self.buffer_goal + self.last_position;
        if let Some(seg) = segment_queue.get_next(&pl.segment_list, maximum_position) {
            self.requester.request_media_segment(media_type, seg);
        }
    }

    /// Method called once a segment request ended with success
    pub(super) fn on_segment_fetch_success(&mut self,
        segment_req: SegmentRequestInfo,
        result: DataSource
    ) {
        let media_type = segment_req.media_type;
        Logger::debug(&format!("{} segment request finished, pushing it...", media_type));
        if let Some(sb) = self.source_buffer_store.get_mut(media_type) {
            sb.append_buffer(PushMetadata::new(result));
            self.request_next_segment(media_type);
        }
    }
}
