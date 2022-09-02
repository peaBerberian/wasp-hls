use crate::{
    bindings::{
        MediaType,
        jsFetchU8,
        jsFetchU8NoCopy,
        PlayerId,
        RequestId,
        jsAbortRequest,
    },
    Logger,
    content_tracker::MediaPlaylistPermanentId,
    parser::SegmentInfo,
    utils::url::Url,
};

pub struct Requester {
    player_id: PlayerId,

    /// List information on the current playlist requests awaited, by chronological order
    /// (from the time the request was made).
    pending_playlist_requests: Vec<PlaylistRequestInfo>,

    /// List information on the current segment requests performed, by chronological order
    /// (from the time the request was made).
    pending_segment_requests: Vec<SegmentRequestInfo>,
}

#[derive(PartialEq)]
pub enum PlaylistFileType {
    MultiVariantPlaylist,
    MediaPlaylist { id: MediaPlaylistPermanentId, media_type: MediaType },
    Unknown,
}

/// Metadata associated with a pending Playlist (either a MultiVariant Playlist or a Media
/// Playlist request.
pub struct PlaylistRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    request_id: RequestId,

    /// Url on which the request is done
    pub url: Url,

    /// Type of the Playlist that is requested
    pub playlist_type: PlaylistFileType,
}

/// Metadata associated with a pending media segment request.
pub struct SegmentRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    request_id: RequestId,

    /// type of media of the segment requested
    pub media_type: MediaType,

    /// Url on which the request is done
    pub url: Url,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    pub time_info: Option<(f64, f64)>,
}

pub enum FinishedRequestType {
    Playlist(PlaylistRequestInfo),
    Segment(SegmentRequestInfo),
}

impl Requester {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            pending_playlist_requests: vec![],
            pending_segment_requests: vec![],
        }
    }

    /// Fetch either the MultiVariantPlaylist or a MediaPlaylist reachable
    /// through the given `url` and add its `request_id` to `pending_playlist_requests`.
    ///
    /// Once it succeeds, `on_u8_request_finished` of the `WaspHlsPlayer` with the
    /// `player_id` associated to this Requester  will be called with the result.
    pub(crate) fn fetch_playlist(&mut self, url: Url, playlist_type: PlaylistFileType) {
        Logger::info(&format!("Fetching playlist {}", url.get_ref()));
        let request_id = jsFetchU8(self.player_id, url.get_ref());
        self.pending_playlist_requests.push(PlaylistRequestInfo { request_id, url, playlist_type });
    }

    /// Fetch the initialization segment whose metadata is given here add its
    /// `request_id` to `pending_segment_requests`.
    ///
    /// Once it succeeds, `on_u8_request_finished` of the `WaspHlsPlayer` with the
    /// `player_id` associated to this Requester  will be called with the result.
    pub(crate) fn request_init_segment(&mut self, media_type: MediaType, url: Url) {
        Logger::debug(&format!("Requesting {} initialization segment", media_type));
        let request_id = jsFetchU8NoCopy(self.player_id, url.get_ref());
        self.pending_segment_requests.push(SegmentRequestInfo {
            request_id,
            media_type,
            url,
            time_info: None,
        });
    }

    /// Fetch a segment in the right format through the given `url` and add its
    /// `request_id` to `pending_segment_requests`.
    ///
    /// Once it succeeds, `on_u8_request_finished` of the `WaspHlsPlayer` with the
    /// `player_id` associated to this Requester  will be called with the result.
    pub(crate) fn request_media_segment(&mut self,
        media_type: MediaType,
        seg: &SegmentInfo
    ) {
        Logger::debug(&format!("Requesting {} segment: t: {}, d: {}", media_type, seg.start, seg.duration));
        let time_info = Some((seg.start, seg.start + seg.duration));
        let request_id = jsFetchU8NoCopy(self.player_id, seg.url.get_ref());
        self.pending_segment_requests.push(SegmentRequestInfo {
            request_id,
            media_type,
            url: seg.url.clone(),
            time_info,
        });
    }

    pub(crate) fn abort_segments<F>(&mut self, f: F)
        where F: Fn(&SegmentRequestInfo) -> bool
    {
        let mut i = 0;
        while i < self.pending_segment_requests.len() {
            let next_req = &self.pending_segment_requests[i];
            if f(next_req) {
                Logger::lazy_debug(&|| {
                    let media_type = next_req.media_type;
                    match next_req.time_info {
                        None => format!("Aborting {} init segment", media_type),
                        Some((start, duration)) =>
                            format!("Aborting {} segment: t: {}, d: {}", media_type, start, duration),
                    }
                });
                jsAbortRequest(next_req.request_id);
                self.pending_segment_requests.remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub(crate) fn segment_requests(&self) -> &[SegmentRequestInfo] {
        self.pending_segment_requests.as_slice()
    }

    pub(crate) fn has_segment_request_pending(&self, media_type: MediaType) -> bool {
        self.pending_segment_requests.iter().any(|r| {
            r.media_type == media_type
        })
    }

    // pub(crate) fn has_init_segment_request_pending(&self,
    //     media_type: MediaType
    // ) -> bool {
    //     self.pending_segment_requests.iter().any(|r| {
    //         r.media_type == media_type &&
    //             r.time_info.is_none()
    //     })
    // }

    // pub(crate) fn has_media_segment_request_pending(&self,
    //     media_type: MediaType
    // ) -> bool {
    //     self.pending_segment_requests.iter().any(|r| {
    //         r.media_type == media_type && r.time_info.is_some()
    //     })
    // }

    pub(crate) fn remove_pending_request(&mut self,
        request_id: RequestId
    ) -> Option<FinishedRequestType> {
        if let Some(res) = self.remove_pending_segment_request(request_id) {
            Some(FinishedRequestType::Segment(res))
        } else {
            Some(
                FinishedRequestType::Playlist(self.remove_pending_playlist_request(request_id)?)
            )
        }
    }

    fn remove_pending_playlist_request(&mut self,
        request_id: RequestId
    ) -> Option<PlaylistRequestInfo> {
        let mut i = 0;
        while i < self.pending_playlist_requests.len() {
            if self.pending_playlist_requests[i].request_id == request_id {
                let req = self.pending_playlist_requests.remove(i);
                return Some(req);
            } else {
                i += 1;
            }
        }
        None
    }

    fn remove_pending_segment_request(&mut self,
        request_id: RequestId
    ) -> Option<SegmentRequestInfo> {
        let mut i = 0;
        while i < self.pending_segment_requests.len() {
            if self.pending_segment_requests[i].request_id == request_id {
                let removed = self.pending_segment_requests.remove(i);
                return Some(removed);
            } else {
                i += 1;
            }
        }
        None
    }

    pub(crate) fn abort_all(&mut self) {
        for elt in self.pending_playlist_requests.drain(..) {
            jsAbortRequest(elt.request_id);
        }
        for elt in self.pending_segment_requests.drain(..) {
            jsAbortRequest(elt.request_id);
        }
    }
}
