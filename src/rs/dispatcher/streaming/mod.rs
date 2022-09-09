use crate::{
    bindings::{
        DataSource,
        MediaType,
        jsStartObservingPlayback,
        jsStopObservingPlayback,
        jsRemoveMediaSource,
        RequestId,
        SourceBufferId,
        jsSetMediaSourceDuration,
        MediaObservation,
    },
    Logger,
    content_tracker::{MediaPlaylistPermanentId, ContentTracker},
    parser::MultiVariantPlaylist,
    requester::{
        PlaylistFileType,
        SegmentRequestInfo,
        PlaylistRequestInfo,
        FinishedRequestType,
    },
    media_element::{PushMetadata, SourceBufferCreationError},
    utils::url::Url,
    segment_selector::NextSegmentInfo,
};
use super::{
    MediaSourceReadyState,
    Dispatcher,
    PlayerReadyState,
};

impl Dispatcher {
    pub(crate) fn on_request_succeeded(&mut self,
        request_id: RequestId,
        data: DataSource,
        final_url: Url,
        resource_size: u32,
        duration_ms: f64,
    ) {
        match self.requester.remove_pending_request(request_id) {
            Some(FinishedRequestType::Segment(seg_info)) =>
                self.on_segment_fetch_success(seg_info, data, resource_size, duration_ms),
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
    /// If all conditions are met, the `Dispatcher` is set to the next
    /// `AwaitingSegments` `PlayerReadyState`, playback observations begin,
    /// and the potential initialization segments are requested.
    pub(super) fn check_ready_to_load_segments(&mut self) {
        if self.ready_state >= PlayerReadyState::AwaitingSegments {
            return;
        }

        match self.media_element_ref.buffers() {
            None => { return; },
            Some(x) => if x.ready_state() == MediaSourceReadyState::Closed {
                return;
            }
        }

        let content_tracker = if let Some(ctnt) = self.content_tracker.as_ref() {
            ctnt
        } else { return; };
        if content_tracker.all_curr_media_playlists_ready() {
            self.ready_state = PlayerReadyState::AwaitingSegments;
            let start_time = content_tracker.get_expected_start_time();
            self.media_element_ref.seek_once_ready(start_time);
            if let Some(duration) = content_tracker.curr_duration() {
                jsSetMediaSourceDuration(duration);
            } else {
                Logger::warn("Unknown content duration");
            }

            if let Some(Err(e)) = self.init_source_buffer(MediaType::Audio) {
                self.fail_on_error(&format!("Error while creating audio SourceBuffer: {:?}", e));
                return;
            }
            if let Some(Err(e)) = self.init_source_buffer(MediaType::Video) {
                self.fail_on_error(&format!("Error while creating video SourceBuffer: {:?}", e));
                return;
            }
            jsStopObservingPlayback();
            jsStartObservingPlayback();
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
            },
            Ok(pl) => {
                Logger::info("MultiVariant Playlist parsed successfully");
                self.content_tracker = Some(ContentTracker::new(pl));
                let content_tracker = self.content_tracker.as_mut().unwrap();
                if content_tracker.variants().is_empty() {
                    self.fail_on_error(
                        "Error while parsing MultiVariantPlaylist: no variant found.");
                    return;
                }

                use PlaylistFileType::*;
                // TODO lowest/latest bandwidth first?
                content_tracker.update_curr_bandwidth(2_000_000.);
                if let Some((url, id)) = content_tracker.curr_media_playlist_request_info(MediaType::Video) {
                    let id = id.clone();
                    let url = url.clone();
                    self.requester.fetch_playlist(url, MediaPlaylist { id, media_type: MediaType::Video });
                }
                if let Some((url, id)) = content_tracker.curr_media_playlist_request_info(MediaType::Audio) {
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
        data: Vec<u8>,
        playlist_url: Url
    ) {
        Logger::info(&format!("Media playlist loaded successfully: {}", playlist_url.get_ref()));
        if let Some(ref mut content_tracker) = self.content_tracker.as_mut() {
            if let Err(e) = content_tracker.update_media_playlist(&playlist_id, data.as_ref(), playlist_url) {
                self.fail_on_error(&format!("Failed to parse MediaPlaylist: {:?}", e));
            } else if self.ready_state == PlayerReadyState::Loading {
                self.check_ready_to_load_segments();
            }
        } else { self.fail_on_error("Media playlist loaded but no MultiVariantPlaylist"); }
    }

    fn init_source_buffer(&mut self,
        media_type: MediaType
    ) -> Option<Result<(), SourceBufferCreationError>> {
        match self.media_element_ref.buffers_mut() {
            None => Some(Err(SourceBufferCreationError::NoMediaSourceAttached)),
            Some(buffers) => {
                if buffers.has(media_type) || !buffers.can_still_create_source_buffer() {
                    // TODO return Err here
                    return None;
                }

                let content = self.content_tracker.as_mut()?;
                let media_playlist = content.curr_media_playlist(media_type)?;
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

                Some(buffers.create_source_buffer(media_type, mime_type, &codecs))
            }
        }
    }

    pub(crate) fn internal_on_media_source_state_change(&mut self, state: MediaSourceReadyState) {
        match self.media_element_ref.buffers_mut() {
            None => {},
            Some(buffers) => {
                Logger::info(&format!("MediaSource state changed: {:?}", state));
                buffers.update_ready_state(state);
                if state == MediaSourceReadyState::Open {
                    self.check_ready_to_load_segments();
                }
            }
        }
    }

    pub(crate) fn internal_on_source_buffer_update(&mut self,
        source_buffer_id: SourceBufferId
    ) {
        match self.media_element_ref.buffers_mut() {
            None => {},
            Some(buffers) => buffers.on_source_buffer_update(source_buffer_id),
        }
    }

    pub(crate) fn internal_on_source_buffer_error(&mut self,
        _source_buffer_id: SourceBufferId
    ) {
        // TODO check QuotaExceededError and so on...
        self.fail_on_error("A SourceBuffer emitted an error");
    }

    pub fn on_regular_tick(&mut self, observation: MediaObservation) {
        let position = observation.current_time();
        Logger::debug(&format!("Tick received: {}", position));
        self.last_position = position;
        self.segment_selectors.update_base_position(position);
        self.check_segments_to_request()
    }

    pub fn on_seek(&mut self, observation: MediaObservation) {
        let position = observation.current_time();
        Logger::debug(&format!("Seeking tick received: {}", position));
        self.segment_selectors.reset_position(position);

        // TODO better logic than aborting everything on seek
        self.requester.abort_segments(|_| true);
        self.check_segments_to_request()
    }

    pub fn check_segments_to_request(&mut self) {
        match self.content_tracker.as_ref() {
            None => {},
            Some(ctnt) => {
                [MediaType::Video, MediaType::Audio].iter().for_each(|mt| {
                    let mt = *mt;
                    if !self.requester.has_segment_request_pending(mt) {
                        if let Some(pl) = ctnt.curr_media_playlist(mt) {
                            match self.segment_selectors.get_mut(mt).get_next_segment_info(pl) {
                                NextSegmentInfo::None => {},
                                NextSegmentInfo::InitSegment(i) =>
                                    self.requester.request_init_segment(mt, i.uri.clone()),
                                NextSegmentInfo::MediaSegment(seg) =>
                                    self.requester.request_media_segment(mt, seg),
                            }
                        }
                    }
                });
            },
        }
    }

    pub fn internal_stop(&mut self) {
        Logger::info("Stopping current content (if one) and resetting player");
        self.requester.abort_all();
        jsStopObservingPlayback();
        jsRemoveMediaSource();
        self.segment_selectors.reset_position(0.);
        self.content_tracker = None;
        self.media_element_ref.reset();
        self.last_position = 0.;
        self.ready_state = PlayerReadyState::Stopped;
    }

    /// Method called once a playlist request ended with success
    pub(super) fn on_playlist_fetch_success(&mut self,
        pl_info: PlaylistRequestInfo,
        result: Vec<u8>,
        final_url: Url
    ) {
        let PlaylistRequestInfo { playlist_type, .. } = pl_info;
        if let PlaylistFileType::MediaPlaylist { id, .. } = playlist_type {
            self.on_media_playlist_loaded(id, result, final_url);
        } else {
            self.on_multivariant_playlist_loaded(result, final_url);
        }
    }

    fn push_and_validate_segment(&mut self,
        data: DataSource,
        media_type: MediaType,
        time_info: Option<(f64, f64)>
    ) {
        match self.media_element_ref.buffers_mut() {
            None => {
                let err = "Can't push: no Buffers created";
                self.fail_on_error(err);
                return;
            },
            Some(buffers) => match buffers.push_segment(media_type, PushMetadata::new(data)) {
                Err(_) => {
                    let err = &format!("Can't push: {} SourceBuffer not found", media_type);
                    self.fail_on_error(err);
                    return;
                }
                Ok(()) => if let Some(ti) = time_info {
                    self.segment_selectors.get_mut(media_type).validate_media(ti.0);
                    if was_last_segment(self.content_tracker.as_ref(), media_type, ti.0) {
                        Logger::debug(&format!("Last {} segment request finished, declaring its buffer's end", media_type));
                        buffers.end(media_type);
                    }
                } else {
                    self.segment_selectors.get_mut(media_type).validate_init();
                }
            }
        }
    }

    fn process_request_metrics(&mut self,
        resource_size: u32,
        duration_ms: f64,
    ) {
        self.adaptive_selector.add_metric(duration_ms, resource_size);
        if let Some(ctnt) = self.content_tracker.as_mut() {
            if let Some(bandwidth) = self.adaptive_selector.get_estimate() {
                Logger::debug(&format!("New bandwidth estimate: {}", bandwidth));
                ctnt.update_curr_bandwidth(bandwidth).iter().for_each(|mt| {
                    let mt = *mt;
                    Logger::debug(&format!("{} MediaPlaylist changed", mt));
                    self.requester.abort_segments(|x| { x.media_type == mt });
                    let selector = self.segment_selectors.get_mut(mt);
                    selector.rollback();
                    selector.reset_init_segment();
                    if ctnt.curr_media_playlist(mt).is_none() {
                        if let Some((url, id)) = ctnt.curr_media_playlist_request_info(mt) {
                            use PlaylistFileType::*;
                            Logger::debug("Media changed, requesting its media playlist");
                            let id = id.clone();
                            let url = url.clone();
                            self.requester.fetch_playlist(url, MediaPlaylist { id, media_type: mt });
                        }
                    }
                })
            }
        }
    }

    /// Method called once a segment request ended with success
    pub(super) fn on_segment_fetch_success(&mut self,
        segment_req: SegmentRequestInfo,
        result: DataSource,
        resource_size: u32,
        duration_ms: f64,
    ) {
        Logger::lazy_debug(&|| {
            let media_type = segment_req.media_type;
            match segment_req.time_info {
                None => format!("Loaded {} init segment", media_type),
                Some((start, duration)) =>
                    format!("Loaded {} segment: t: {}, d: {}", media_type, start, duration),
            }
        });

        self.push_and_validate_segment(result, segment_req.media_type, segment_req.time_info);
        self.process_request_metrics(resource_size, duration_ms);
        self.check_segments_to_request();
    }
}

fn was_last_segment(
    content_tracker: Option<&ContentTracker>,
    media_type: MediaType,
    seg_start: f64,
) -> bool {
    match content_tracker {
        None => false,
        Some(ctnt) => {
            match ctnt.curr_media_playlist(media_type) {
                None => false,
                Some(pl) => {
                    pl.last_segment_start() == Some(seg_start)
                },
            }
        },
    }
}
