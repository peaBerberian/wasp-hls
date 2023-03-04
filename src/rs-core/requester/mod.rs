use crate::{
    bindings::{
        MediaType,
        jsFetch,
        RequestId,
        jsAbortRequest,
    },
    Logger,
    content_tracker::MediaPlaylistPermanentId,
    parser::SegmentInfo,
    utils::url::Url,
};

const PRIORITY_STEPS : [f64; 6] = [2., 4., 8., 12., 18., 25.];

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
enum PriorityLevel {
    ExtremelyHigh = 0,
    VeryHigh = 1,
    High = 2,
    Medium = 3,
    Low = 4,
    VeryLow = 5,
    ExtremelyLow = 6,
}

impl PriorityLevel {
    fn from_time_distance(distance: f64) -> Self {
        let step_info = PRIORITY_STEPS.iter().enumerate().find(|(_, step)| {
            distance < **step
        });
        match step_info {
            Some((0, _)) => PriorityLevel::ExtremelyHigh,
            Some((1, _)) => PriorityLevel::VeryHigh,
            Some((2, _)) => PriorityLevel::High,
            Some((3, _)) => PriorityLevel::Medium,
            Some((4, _)) => PriorityLevel::Low,
            Some((5, _)) => PriorityLevel::VeryLow,
            _ => PriorityLevel::ExtremelyLow,
        }
    }
}

fn get_segment_priority(start_time: Option<f64>, current_time: f64) -> PriorityLevel {
    match start_time {
        Some(start_time) =>
            PriorityLevel::from_time_distance(start_time - current_time),
        _ => PriorityLevel::ExtremelyHigh,
    }
}

pub struct Requester {
    /// List information on the current playlist requests awaited, by chronological order
    /// (from the time the request was made).
    pending_playlist_requests: Vec<PlaylistRequestInfo>,

    /// List information on the current segment requests performed, by chronological order
    /// (from the time the request was made).
    ///
    /// There should be only one request per MediaType pending or waiting (i.e. in
    /// the `segment_waiting_queue` vector) at the same time.
    pending_segment_requests: Vec<SegmentRequestInfo>,

    /// List information on segment requests awaiting for segment requests of
    /// higher priorities to finish before actually being made.
    ///
    /// There should be only one request per MediaType pending (i.e. in
    /// the `pending_segment_requests` vector) or waiting at the same time.
    segment_waiting_queue: Vec<WaitingSegmentInfo>,

    /// If `true`, no new requests will be started (they all will be pushed in
    /// `segment_waiting_queue` instead) until it is set to `false` again.
    ///
    /// Using this strategy allows to let outside code schedule multiple
    /// requests while the "lock" is on.
    /// Once all wanted requests have been scheduled, the same "lock" can be
    /// removed resulting in the `Requester` choosing between all of them which
    /// one it will actually requests immediately based on its internal
    /// priorization algorithm.
    ///
    /// Without the lock, priorization would be less efficient, for example, the
    /// initial request would always be performed immediately as it would always
    /// be the one with the highest priority until now.
    segment_request_locked: bool,

    /// Position in seconds, on which the `Requester` will base itself to decide
    /// segment requests priority.
    ///
    /// This position should be close the current playback condition.
    ///
    /// When `None`, all segment requests will have the highest possible
    /// priority.
    base_position: Option<f64>,
}

#[derive(PartialEq)]
pub enum PlaylistFileType {
    MultiVariantPlaylist,
    MediaPlaylist { id: MediaPlaylistPermanentId },
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
pub struct WaitingSegmentInfo {
    /// type of media of the segment requested
    pub media_type: MediaType,

    /// Url on which the request is done
    pub url: Url,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    pub time_info: Option<(f64, f64)>,
}

pub trait RequesterSegmentInfo {
    fn media_type(&self) -> MediaType;
    fn start_time(&self) -> Option<f64>;
    fn duration(&self) -> Option<f64>;
    fn url(&self) -> &Url;
}

impl RequesterSegmentInfo for SegmentRequestInfo {
    fn media_type(&self) -> MediaType {
        self.media_type
    }

    fn start_time(&self) -> Option<f64> {
        Some(self.time_info?.0)
    }

    fn duration(&self) -> Option<f64> {
        Some(self.time_info?.1)
    }

    fn url(&self) -> &Url {
        &self.url
    }
}

impl RequesterSegmentInfo for WaitingSegmentInfo {
    fn media_type(&self) -> MediaType {
        self.media_type
    }

    fn start_time(&self) -> Option<f64> {
        Some(self.time_info?.0)
    }

    fn duration(&self) -> Option<f64> {
        Some(self.time_info?.1)
    }

    fn url(&self) -> &Url {
        &self.url
    }
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
    pub fn new() -> Self {
        Self {
            pending_playlist_requests: vec![],
            pending_segment_requests: vec![],
            segment_waiting_queue: vec![],
            segment_request_locked: false,
            base_position: None,
        }
    }

    pub fn reset(&mut self) {
        self.abort_all();
        self.base_position = None;
        self.segment_request_locked = false;
    }

    /// Update the `Requester`'s inner concept of a `base_position`, which is the position in
    /// seconds on which the `Requester` will base itself to deduce the priorization of segment
    /// requests:
    ///
    /// Segments which start close to (or before) this `base_position` will be considered of higher
    /// priority than the ones starting further from it, and thus the requests for the former might
    /// be priorized (i.e. started sooner) compared to requests for the latter.
    ///
    /// For an optimal `Requester` behavior, it should be set to the wanted playback position.
    pub fn update_base_position(&mut self, time: Option<f64>) {
        self.base_position = time;
        self.check_segment_queue();
    }

    /// Fetch either the MultiVariantPlaylist or a MediaPlaylist reachable
    /// through the given `url` and add its `request_id` to `pending_playlist_requests`.
    ///
    /// Once it succeeds, the `on_request_finished` function will be called.
    pub(crate) fn fetch_playlist(&mut self, url: Url, playlist_type: PlaylistFileType) {
        Logger::info(&format!("Fetching playlist {}", url.get_ref()));
        let request_id = jsFetch(url.get_ref());
        self.pending_playlist_requests.push(PlaylistRequestInfo { request_id, url, playlist_type });
    }

    /// Fetch the initialization segment whose metadata is given here add its
    /// `request_id` to `pending_segment_requests`.
    ///
    /// Once it succeeds, the `on_request_finished` function will be called.
    pub(crate) fn request_init_segment(&mut self, media_type: MediaType, url: Url) {
        Logger::info(&format!("Requesting {} initialization segment", media_type));
        let request_id = jsFetch(url.get_ref());
        self.pending_segment_requests.push(SegmentRequestInfo {
            request_id,
            media_type,
            url,
            time_info: None,
        });
    }

    /// Fetch a segment in the right format through the given `url`.
    ///
    /// Depending on the estimated request priority (based on the `base_position`
    /// last communicated through the `update_base_position` method) and on if
    /// segment requests are currently being locked (see `lock_segment_requests` and
    /// `unlock_segment_requests` methods), the request will technically either be
    /// started right away or once the right condition is triggered.
    ///
    /// Once the request finishes with success, the `on_request_finished`
    /// function will be called.
    pub(crate) fn request_media_segment(&mut self,
        media_type: MediaType,
        seg: &SegmentInfo
    ) {
        Logger::info(&format!("Req: Asking to request {} segment: t: {}, d: {}",
                media_type, seg.start, seg.duration));
        let time_info = Some((seg.start, seg.start + seg.duration));
        if !self.can_start_new_media_segment_request(seg.start) {
            Logger::debug("Req: pushing segment request to queue");
            self.segment_waiting_queue.push(WaitingSegmentInfo {
                media_type,
                url: seg.url.clone(),
                time_info,
            });
        } else {
            Logger::debug("Req: Performing request right away");
            let request_id = jsFetch(seg.url.get_ref());
            self.pending_segment_requests.push(SegmentRequestInfo {
                request_id,
                media_type,
                url: seg.url.clone(),
                time_info,
            });
        }
    }

    pub(crate) fn lock_segment_requests(&mut self) -> bool {
        let was_locked = self.segment_request_locked;
        self.segment_request_locked = true;
        was_locked
    }

    pub(crate) fn unlock_segment_requests(&mut self) {
        self.segment_request_locked = false;
        self.check_segment_queue();
    }

    pub(crate) fn has_segment_request_pending(&self, media_type: MediaType) -> bool {
        self.pending_segment_requests.iter().any(|r| {
            r.media_type == media_type
        }) || self.segment_waiting_queue.iter().any(|r| {
            r.media_type == media_type
        })
    }

    pub(crate) fn end_pending_request(&mut self,
        request_id: RequestId
    ) -> Option<FinishedRequestType> {
        if let Some(res) = self.end_pending_segment_request(request_id) {
            Some(FinishedRequestType::Segment(res))
        } else {
            Some(
                FinishedRequestType::Playlist(self.end_pending_playlist_request(request_id)?)
            )
        }
    }

    pub(crate) fn abort_all(&mut self) {
        for elt in self.pending_playlist_requests.drain(..) {
            jsAbortRequest(elt.request_id);
        }
        for elt in self.pending_segment_requests.drain(..) {
            jsAbortRequest(elt.request_id);
        }
        self.check_segment_queue();
    }

    pub(crate) fn abort_all_segments(&mut self) {
        while let Some(last_req) = self.pending_segment_requests.pop() {
            log_segment_abort(&last_req);
            jsAbortRequest(last_req.request_id);
        }
        while let Some(last_req) = self.segment_waiting_queue.pop() {
            log_segment_abort(&last_req);
        }
    }

    pub(crate) fn abort_segments_with_type(&mut self, media_type: MediaType) {
        let mut i = 0;
        let mut aborted_pending = false;
        while i < self.pending_segment_requests.len() {
            let next_req = &self.pending_segment_requests[i];
            if next_req.media_type() == media_type {
                log_segment_abort(next_req);
                aborted_pending = true;
                jsAbortRequest(next_req.request_id);
                self.pending_segment_requests.remove(i);
            } else {
                i += 1;
            }
        }
        while i < self.segment_waiting_queue.len() {
            let next_req = &self.segment_waiting_queue[i];
            if next_req.media_type() == media_type {
                log_segment_abort(next_req);
                self.segment_waiting_queue.remove(i);
            } else {
                i += 1;
            }
        }
        if aborted_pending {
            self.check_segment_queue();
        }

    }

    fn end_pending_playlist_request(&mut self,
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

    fn end_pending_segment_request(&mut self,
        request_id: RequestId
    ) -> Option<SegmentRequestInfo> {
        let mut i = 0;
        while i < self.pending_segment_requests.len() {
            if self.pending_segment_requests[i].request_id == request_id {
                let removed = self.pending_segment_requests.remove(i);
                self.check_segment_queue();
                return Some(removed);
            } else {
                i += 1;
            }
        }
        None
    }

    fn check_segment_queue(&mut self) {
        if self.segment_request_locked {
            return ;
        }
        if self.segment_waiting_queue.is_empty() {
            return ;
        } else if let Some(base_pos) = self.base_position {
            let min_pending_priority = self.min_pending_priority();
            let new_min_priority = self.segment_waiting_queue
                .iter()
                .enumerate()
                .fold(min_pending_priority, |acc, (_, w)| {
                    let w_prio = get_segment_priority(w.start_time(), base_pos);
                    match acc {
                        None =>  Some(w_prio),
                        Some(priority) =>  Some(w_prio.min(priority)),
                    }
                });

            if let Some(new_min_priority) = new_min_priority {
                // TODO drain_filter when it's stabilized
                let indexes: Vec<usize> = self.segment_waiting_queue
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        get_segment_priority(w.start_time(), base_pos) <= new_min_priority
                    })
                .map(|w| { w.0 })
                    .collect();

                indexes
                    .iter()
                    .enumerate()
                    .map(|(idx_idx, idx)| {
                        self.segment_waiting_queue.remove(idx - idx_idx)
                    })
                .for_each(|seg| {
                    // TODO indicate which segment in log
                    Logger::debug("Req: Performing request of queued segment");
                    let request_id = jsFetch(seg.url.get_ref());
                    self.pending_segment_requests.push(SegmentRequestInfo {
                        request_id,
                        media_type: seg.media_type,
                        url: seg.url,
                        time_info: seg.time_info,
                    });
                });
            }
        } else {
            while let Some(seg) = self.segment_waiting_queue.pop() {
                // TODO indicate which segment in log
                Logger::debug("Req: Performing request of queued segment");
                let request_id = jsFetch(seg.url.get_ref());
                self.pending_segment_requests.push(SegmentRequestInfo {
                    request_id,
                    media_type: seg.media_type,
                    url: seg.url,
                    time_info: seg.time_info,
                });
            }
        }
    }

    fn min_pending_priority(&self) -> Option<PriorityLevel> {
        if self.pending_segment_requests.is_empty() {
            None
        } else {
            match self.base_position {
                None => None,
                Some(pos) => {
                    let first_pending_priority = get_segment_priority(
                        self.pending_segment_requests[0].start_time(), pos);
                    Some(self.pending_segment_requests.iter().skip(1)
                        .fold(first_pending_priority, |acc, req| {
                            let priority = get_segment_priority(req.start_time(), pos);
                            if priority < acc {
                                priority
                            } else {
                                acc
                            }
                        }))
                }
            }
        }
    }

    fn can_start_new_media_segment_request(&self, start_time: f64) -> bool {
        if self.segment_request_locked {
            return false;
        }
        let min_pending_priority = self.min_pending_priority();
        if let (Some(pos), Some(min_pending_priority)) =
            (self.base_position, min_pending_priority)
        {
            let curr_seg_priority = PriorityLevel::from_time_distance(start_time - pos);
            if curr_seg_priority > min_pending_priority {
                return false;
            }
        }
        true
    }
}

fn log_segment_abort(seg: &impl RequesterSegmentInfo) {
    Logger::lazy_info(&|| {
        let media_type = seg.media_type();
        match (seg.start_time(), seg.duration()) {
            (Some(start), Some(duration)) =>
                format!("Aborting {} segment: t: {}, d: {}", media_type, start, duration),
            _ => format!("Aborting {} init segment", media_type),
        }
    });
}
