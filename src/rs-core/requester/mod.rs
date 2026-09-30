use crate::{
    bindings::{
        formatters::format_range_for_js, jsAbortRequest, jsFetch, jsGetRandom, jsTimer, MediaType,
        RequestErrorReason, RequestId, TimerId, TimerReason,
    },
    parser::{ByteRange, MediaSegmentInfo, SegmentTimeInfo},
    playlist_store::MediaPlaylistPermanentId,
    utils::{logger::*, url::Url},
};

mod configuration;

pub(crate) use configuration::RequesterConfiguration;

const PRIORITY_STEPS: [f64; 6] = [2., 4., 8., 12., 18., 25.];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestLaneTag {
    Audio,
    Video,
    Probe,
}

impl RequestLaneTag {
    pub(crate) fn from_media_type(media_type: MediaType) -> Self {
        match media_type {
            MediaType::Audio => Self::Audio,
            MediaType::Video => Self::Video,
        }
    }

    fn media_type(self) -> Option<MediaType> {
        match self {
            Self::Audio => Some(MediaType::Audio),
            Self::Video => Some(MediaType::Video),
            Self::Probe => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Probe => "probe",
        }
    }
}

/// The `Requester` is the module performing HTTP(s) requests.
///
/// Depending on the nature of the resource and on its configuration, it also has both a
/// request-scheduling mechanism, allowing to perform more urgent request first, and a retry
/// mechanism based on an exponential backoff delay, to retry requesting resources without
/// overloading the server serving them.
pub(crate) struct Requester {
    /// List information on the current playlist requests awaited, by chronological order (from the
    /// time the request was made).
    pending_playlist_requests: Vec<PlaylistRequestInfo>,

    /// List information on the current segment requests performed, by chronological order (from the
    /// time the request was made).
    ///
    /// There should be only one request per MediaType pending or waiting (i.e. in the
    /// `segment_waiting_queue` vector) at the same time.
    pending_segment_requests: Vec<SegmentRequestInfo>,

    /// List information on segment requests awaiting for segment requests of higher priorities to
    /// finish before actually being made.
    ///
    /// There should be only one request per MediaType pending (i.e. in the
    /// `pending_segment_requests` vector) or waiting at the same time.
    segment_waiting_queue: Vec<WaitingSegmentInfo>,

    /// Depending the nature of the failure, failed requests might be retried.
    ///
    /// To avoid overloading the server serving those resources, retried requests are actually
    /// performed after a timer.
    /// This variable allows to store and link here a timer's TimerId (which will be communicated
    /// back when the timer has elapsed) to the RequestId.
    ///
    /// Note that retried segment requests stay in the `pending_segment_requests` vector and retried
    /// playlist requests stay in the `pending_playlist_requests` vector, even when the request is
    /// not really pending.
    retry_timers: Vec<(TimerId, RequestId)>,

    /// If `true`, no new requests will be started (they all will be pushed in
    /// `segment_waiting_queue` instead) until it is set to `false` again.
    ///
    /// Using this strategy allows to let outside code schedule multiple requests while the "lock"
    /// is on.
    /// Once all wanted requests have been scheduled, the same "lock" can be removed resulting in
    /// the `Requester` choosing between all of them which one it will actually requests immediately
    /// based on its internal priorization algorithm.
    ///
    /// Without the lock, priorization would be less efficient, for example, the initial request
    /// would always be performed immediately as it would always be the one with the highest
    /// priority until now.
    segment_request_locked: bool,

    /// Position in seconds, on which the `Requester` will base itself to decide segment requests
    /// priority.
    ///
    /// This position should be close the current playback condition.
    ///
    /// When `None`, all segment requests will have the highest possible priority.
    base_position: Option<f64>,

    /// Current configuration on which the `Requester` relies.
    config: RequesterConfiguration,
}

/// Identify a type of Playlist requested.
#[derive(PartialEq)]
pub(crate) enum PlaylistFileType {
    /// This is the top-level Playlist loaded through the public `load` API.
    TopLevelPlaylist,
    /// This is a Media Playlist with this associated `id` and `MediaType`.
    MediaPlaylist {
        // Identifier uniquely identifying that playlist
        id: MediaPlaylistPermanentId,
        // The media type associated to the media playlist.
        // TODO: The media_type should probably be removed long-term here.
        //       With segments, it makes sense due to the "lane concept" (one
        //       in-flight request for the given type). For playlists, the only
        //       reason is due to how it will be then exploited once the request
        //       is finished, which could be re-computed anyway.
        media_type: MediaType,
    },
}

/// Metadata associated with a pending Playlist (either a top-level Playlist or a Media
/// Playlist request.
pub(crate) struct PlaylistRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    host_id: RequestId,

    /// Url on which the request is done
    pub(crate) url: Url,

    /// Type of the Playlist that is requested
    pub(crate) playlist_type: PlaylistFileType,

    /// Number of time the request has already been attempted.
    pub(crate) attempts_failed: u32,

    /// If `true` the request is not really pending, we're currently pending for some
    /// timer to finish before retrying it.
    ///
    /// In that case, the `host_id` corresponds to the one of the previous request
    /// and should not be relied on.
    pub(crate) is_waiting_for_retry: bool,
}

/// Metadata associated with a pending media segment request.
pub(crate) struct WaitingSegmentInfo {
    /// Requester-side lane in which that request is scheduled.
    lane_tag: RequestLaneTag,

    /// Url on which the request is done
    url: Url,

    byte_range: Option<ByteRange>,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    ///
    /// TODO: This is only used for priorization and logging here it seems, maybe a more
    /// explicit priority-oriented value would be better?
    time_info: Option<SegmentTimeInfo>,

    /// Opaque identifier allowing the dispatcher layer to recover what should happen when the
    /// request completes.
    caller_id: u32,
}

impl WaitingSegmentInfo {
    pub(crate) fn lane_tag(&self) -> RequestLaneTag {
        self.lane_tag
    }

    /// Returns a reference to the URL leading to this awaiting segment.
    ///
    /// Note that this segment might only be present at a byte-range inside the
    /// resource fetched through that URL. You can know whether this is the case
    /// by calling `byte_range()`.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    /// If set, the resource is located at `url()` only at the range indicated by the returned
    /// `ByteRange` object.
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    /// Time information on that segment. `None` for initialization segment - which do not have
    /// media data.
    pub(crate) fn time_info(&self) -> Option<&SegmentTimeInfo> {
        self.time_info.as_ref()
    }

    pub(crate) fn id(&self) -> u32 {
        self.caller_id
    }
}

/// Trait unifying the Requester's segment which are either in a pending request or which are
/// awaiting one.
pub(crate) trait RequesterSegmentInfo {
    /// Returns the requester-side lane to which the corresponding request belongs.
    fn lane_tag(&self) -> RequestLaneTag;
    /// Returns the start time, in playlist time in seconds, at which the segment starts at.
    ///
    /// Should be `None` only for initialization segment.
    fn start_time(&self) -> Option<f64>;
    /// Returns the duration, in seconds, of that segment.
    ///
    /// Should be `None` only for initialization segment.
    fn duration(&self) -> Option<f64>;
    /// Returns the Url at which that segment is available.
    ///
    /// Note that this segment might only be present at a byte-range inside the
    /// resource fetched through that URL. You can know whether this is the case
    /// by calling `byte_range()`.
    fn url(&self) -> &Url;
}

impl RequesterSegmentInfo for SegmentRequestInfo {
    fn lane_tag(&self) -> RequestLaneTag {
        self.lane_tag
    }

    fn start_time(&self) -> Option<f64> {
        Some(self.time_info.as_ref()?.start())
    }

    fn duration(&self) -> Option<f64> {
        Some(self.time_info.as_ref()?.duration())
    }

    fn url(&self) -> &Url {
        &self.url
    }
}

impl RequesterSegmentInfo for WaitingSegmentInfo {
    fn lane_tag(&self) -> RequestLaneTag {
        self.lane_tag
    }

    fn start_time(&self) -> Option<f64> {
        Some(self.time_info.as_ref()?.start())
    }

    fn duration(&self) -> Option<f64> {
        Some(self.time_info.as_ref()?.duration())
    }

    fn url(&self) -> &Url {
        &self.url
    }
}

/// Metadata associated with a pending media segment request.
pub(crate) struct SegmentRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    host_id: RequestId,

    /// Requester-side lane in which that request is scheduled.
    lane_tag: RequestLaneTag,

    /// Url on which the request is done
    url: Url,

    byte_range: Option<ByteRange>,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    time_info: Option<SegmentTimeInfo>,

    /// Opaque identifier allowing the dispatcher layer to recover what should happen when the
    /// request completes.
    caller_id: u32,

    /// Number of time the request has already been attempted.
    attempts_failed: u32,

    /// If `true` the request is not really pending, we're currently pending for some
    /// timer to finish before retrying it.
    ///
    /// In that case, the `host_id` corresponds to the one of the previous request
    /// and should not be relied on.
    is_waiting_for_retry: bool,
}

impl SegmentRequestInfo {
    pub(crate) fn lane_tag(&self) -> RequestLaneTag {
        self.lane_tag
    }

    pub(crate) fn media_type(&self) -> Option<MediaType> {
        self.lane_tag.media_type()
    }

    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    pub(crate) fn time_info(&self) -> Option<&SegmentTimeInfo> {
        self.time_info.as_ref()
    }

    pub(crate) fn id(&self) -> u32 {
        self.caller_id
    }

    pub(crate) fn attempts_failed(&self) -> u32 {
        self.attempts_failed
    }

    pub(crate) fn is_waiting_for_retry(&self) -> bool {
        self.is_waiting_for_retry
    }
}

pub(crate) enum FinishedRequestType {
    Playlist(PlaylistRequestInfo),
    Segment(SegmentRequestInfo),
}

pub(crate) enum RetryResult<'a> {
    NotFound,
    RetriedPlaylist {
        request_info: &'a PlaylistRequestInfo,
        reason: RequestErrorReason,
        status: Option<u32>,
    },
    RetriedSegment {
        request_info: &'a SegmentRequestInfo,
        reason: RequestErrorReason,
        status: Option<u32>,
    },
    Failed {
        request_type: FinishedRequestType,
        reason: RequestErrorReason,
        status: Option<u32>,
    },
}

impl Requester {
    pub(crate) fn new() -> Self {
        Self {
            pending_playlist_requests: vec![],
            pending_segment_requests: vec![],
            segment_waiting_queue: vec![],
            segment_request_locked: false,
            base_position: None,
            retry_timers: vec![],
            config: RequesterConfiguration::default(),
        }
    }

    /// Abort all pending requests and reset the `Requester` to a base state.
    pub(crate) fn reset(&mut self) {
        self.segment_request_locked = true;
        self.abort_all();
        self.pending_playlist_requests.clear();
        self.pending_segment_requests.clear();
        self.segment_waiting_queue.clear();
        self.retry_timers.clear();
        self.base_position = None;
        self.segment_request_locked = false;
    }

    /// Returns mutable reference to the `Requester`'s inner configuration. Allowing to update it.
    pub(crate) fn config_mut(&mut self) -> &mut RequesterConfiguration {
        &mut self.config
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
    pub(crate) fn update_base_position(&mut self, time: Option<f64>) {
        self.base_position = time;
        self.check_segment_queue();
    }

    /// Fetch either the MultivariantPlaylist or a MediaPlaylist reachable
    /// through the given `url` and add its `host_id` to `pending_playlist_requests`.
    ///
    /// Once it succeeds, the `__web_event__request_finished` function will be called.
    pub(crate) fn fetch_playlist(&mut self, url: Url, playlist_type: PlaylistFileType) {
        let timeout = match playlist_type {
            PlaylistFileType::TopLevelPlaylist => {
                self.config.multi_variant_playlist_request_timeout
            }
            PlaylistFileType::MediaPlaylist { .. } => self.config.media_playlist_request_timeout,
        };
        let url_ref = url.get_ref();
        let host_id = jsFetch(url_ref, None, None, timeout);
        log_info!("Req: Fetching playlist u:{url_ref}, id:{host_id}");
        self.pending_playlist_requests.push(PlaylistRequestInfo {
            host_id,
            url,
            playlist_type,
            attempts_failed: 0,
            is_waiting_for_retry: false,
        });
    }

    pub(crate) fn is_requesting_playlist(
        &self,
        url: &Url,
        playlist_type: &PlaylistFileType,
    ) -> bool {
        self.pending_playlist_requests
            .iter()
            .any(|req| &req.url == url && &req.playlist_type == playlist_type)
    }

    /// Fetch the initialization segment whose metadata is given here add its
    /// `host_id` to `pending_segment_requests`.
    ///
    /// Once it succeeds, the `__web_event__request_finished` function will be called.
    pub(crate) fn request_init_segment(
        &mut self,
        media_type: MediaType,
        url: Url,
        byte_range: Option<&ByteRange>,
        caller_id: u32,
    ) {
        self.request_segment_now(
            &url,
            byte_range,
            RequestLaneTag::from_media_type(media_type),
            None,
            caller_id,
        );
    }

    /// Returns `true` if a segment with the given identifying characteristics is currently either
    /// loading or scheduled.
    ///
    /// Returns `false` if the segment's request is either already finished or if it has never been
    /// communicated to the `Requester`.
    pub(crate) fn is_requesting_segment(
        &mut self,
        media_type: MediaType,
        url: &Url,
        byte_range: Option<&ByteRange>,
    ) -> bool {
        let lane_tag = RequestLaneTag::from_media_type(media_type);
        self.pending_segment_requests
            .iter()
            .any(|s| s.lane_tag == lane_tag && &s.url == url && s.byte_range.as_ref() == byte_range)
            || self.segment_waiting_queue.iter().any(|s| {
                s.lane_tag == lane_tag && &s.url == url && s.byte_range.as_ref() == byte_range
            })
    }

    /// Fetch a segment in the right format through the given `url`.
    ///
    /// Depending on the estimated request priority (based on the `base_position`
    /// last communicated through the `update_base_position` method) and on if
    /// segment requests are currently being locked (see `lock_segment_requests` and
    /// `unlock_segment_requests` methods), the request will technically either be
    /// started right away or once the right condition is triggered.
    ///
    /// Once the request finishes with success, the `__web_event__request_finished`
    /// function will be called.
    pub(crate) fn request_media_segment(
        &mut self,
        media_type: MediaType,
        seg: &MediaSegmentInfo,
        caller_id: u32,
    ) {
        log_info!(
            "Req: Asking to request {} segment: t: {}, d: {}",
            media_type,
            seg.start(),
            seg.duration()
        );
        let lane_tag = RequestLaneTag::from_media_type(media_type);
        let time_info = Some(seg.time_info().clone());
        if self.can_start_request(seg.start()) {
            self.request_segment_now(seg.url(), seg.byte_range(), lane_tag, time_info, caller_id)
        } else {
            log_debug!("Req: pushing segment request to queue");
            self.segment_waiting_queue.push(WaitingSegmentInfo {
                lane_tag,
                url: seg.url().clone(),
                byte_range: seg.byte_range().cloned(),
                time_info,
                caller_id,
            });
        }
    }

    // TODO: Merge it with the others?
    pub(crate) fn request_segment_immediately(
        &mut self,
        lane_tag: RequestLaneTag,
        url: &Url,
        byte_range: Option<&ByteRange>,
        time_info: Option<SegmentTimeInfo>,
        caller_id: u32,
    ) {
        self.request_segment_now(url, byte_range, lane_tag, time_info, caller_id);
    }

    /// Prevent new requests from being started until `unlock_segment_requests` is called.
    ///
    /// This allows to schedule multiple segment request at once, then allowing the `Requester`'s
    /// priorization algorithm take care of which request to do first, instead of just doing
    /// immediately the first one that is scheduled.
    pub(crate) fn lock_segment_requests(&mut self) -> bool {
        let was_locked = self.segment_request_locked;
        self.segment_request_locked = true;
        was_locked
    }

    /// Allow new requests to be started again (after having called `lock_segment_requests`).
    pub(crate) fn unlock_segment_requests(&mut self) {
        self.segment_request_locked = false;
        self.check_segment_queue();
    }

    pub(crate) fn has_segment_request_pending(&self, media_type: MediaType) -> bool {
        let lane_tag = RequestLaneTag::from_media_type(media_type);
        self.pending_segment_requests
            .iter()
            .any(|r| r.lane_tag == lane_tag)
            || self
                .segment_waiting_queue
                .iter()
                .any(|r| r.lane_tag == lane_tag)
    }

    pub(crate) fn on_pending_request_success(
        &mut self,
        host_id: RequestId,
    ) -> Option<FinishedRequestType> {
        self.end_pending_request(host_id)
    }

    pub(crate) fn on_pending_request_failure(
        &'_ mut self,
        host_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>,
    ) -> RetryResult<'_> {
        let reason = match (has_timeouted, status) {
            (true, _) => Some(RequestErrorReason::Timeout),
            (false, Some(x)) if x == 404 || x == 412 || x >= 500 => {
                Some(RequestErrorReason::Status)
            }
            _ => None,
        };
        if let Some(reason) = reason {
            if let Some(pos) = self
                .pending_segment_requests
                .iter()
                .position(|x| x.host_id == host_id)
            {
                self.retry_pending_segment_request(pos, reason, status)
            } else if let Some(pos) = self
                .pending_playlist_requests
                .iter()
                .position(|x| x.host_id == host_id)
            {
                match self.pending_playlist_requests[pos].playlist_type {
                    PlaylistFileType::TopLevelPlaylist => self.retry_playlist_request(
                        pos,
                        reason,
                        status,
                        self.config.multi_variant_playlist_max_retry,
                    ),
                    _ => self.retry_playlist_request(
                        pos,
                        reason,
                        status,
                        self.config.media_playlist_max_retry,
                    ),
                }
            } else {
                log_info!("Req: Request to retry not found, id:{host_id}");
                RetryResult::NotFound
            }
        } else {
            log_info!("Req: Cannot retry request id:{host_id}");
            match self.end_pending_request(host_id) {
                None => RetryResult::NotFound,
                Some(req) => RetryResult::Failed {
                    request_type: req,
                    reason: RequestErrorReason::Error,
                    status,
                },
            }
        }
    }

    pub(crate) fn on_timer_finished(&mut self, timer_id: TimerId) {
        let mut i = 0;
        while i < self.retry_timers.len() {
            if self.retry_timers[i].0 == timer_id {
                let timer = self.retry_timers.remove(i);
                let seg = self
                    .pending_segment_requests
                    .iter_mut()
                    .find(|s| s.host_id == timer.1);
                if let Some(seg) = seg {
                    seg.is_waiting_for_retry = false;
                    let (range_start, range_end) = format_range_for_js(seg.byte_range.as_ref());
                    let host_id = jsFetch(
                        seg.url.get_ref(),
                        range_start,
                        range_end,
                        self.config.segment_request_timeout,
                    );
                    seg.host_id = host_id;
                } else {
                    let pla = self
                        .pending_playlist_requests
                        .iter_mut()
                        .find(|p| p.host_id == timer.1);
                    if let Some(pla) = pla {
                        pla.is_waiting_for_retry = false;
                        let timeout = match pla.playlist_type {
                            PlaylistFileType::TopLevelPlaylist => {
                                self.config.multi_variant_playlist_request_timeout
                            }
                            PlaylistFileType::MediaPlaylist { .. } => {
                                self.config.media_playlist_request_timeout
                            }
                        };
                        let host_id = jsFetch(pla.url.get_ref(), None, None, timeout);
                        pla.host_id = host_id;
                    }
                }
            } else {
                i += 1;
            }
        }
    }

    /// Returns `true` if there is still in-flight segment work that could plausibly fill a hole
    /// before the given buffered range start.
    ///
    /// Media requests starting before `range_start` are treated as hole fillers. Initialization
    /// requests for audio/video are also treated conservatively, because they may be a prerequisite
    /// for a segment that would close that hole.
    pub(crate) fn has_pending_segment_before(&self, range_start: f64) -> bool {
        pending_segment_could_fill_before(self.pending_segment_requests.as_slice(), range_start)
            || pending_segment_could_fill_before(self.segment_waiting_queue.as_slice(), range_start)
    }

    pub(crate) fn abort_all(&mut self) {
        for elt in self.pending_playlist_requests.drain(..) {
            jsAbortRequest(elt.host_id);
        }
        self.abort_all_segments();
        self.check_segment_queue();
    }

    pub(crate) fn abort_all_segments(&mut self) {
        while let Some(last_req) = self.pending_segment_requests.pop() {
            log_segment_abort(&last_req);
            jsAbortRequest(last_req.host_id);
        }
        while let Some(last_req) = self.segment_waiting_queue.pop() {
            log_segment_abort(&last_req);
        }
    }

    pub(crate) fn abort_segments_with_type(&mut self, media_type: MediaType) -> Vec<u32> {
        let lane_tag = RequestLaneTag::from_media_type(media_type);
        let mut i = 0;
        let mut aborted_pending = false;
        let mut aborted_requests = vec![];
        while i < self.pending_segment_requests.len() {
            let next_req = &self.pending_segment_requests[i];
            if next_req.lane_tag() == lane_tag {
                log_segment_abort(next_req);
                aborted_pending = true;
                jsAbortRequest(next_req.host_id);
                let removed = self.pending_segment_requests.remove(i);
                aborted_requests.push(removed.caller_id);
            } else {
                i += 1;
            }
        }
        i = 0;
        while i < self.segment_waiting_queue.len() {
            let next_req = &self.segment_waiting_queue[i];
            if next_req.lane_tag() == lane_tag {
                log_segment_abort(next_req);
                let removed = self.segment_waiting_queue.remove(i);
                aborted_requests.push(removed.caller_id);
            } else {
                i += 1;
            }
        }
        if aborted_pending {
            self.check_segment_queue();
        }
        aborted_requests
    }

    fn end_pending_request(&mut self, host_id: RequestId) -> Option<FinishedRequestType> {
        if let Some(res) = self.end_pending_segment_request(host_id) {
            Some(FinishedRequestType::Segment(res))
        } else {
            Some(FinishedRequestType::Playlist(
                self.end_pending_playlist_request(host_id)?,
            ))
        }
    }

    fn end_pending_playlist_request(&mut self, host_id: RequestId) -> Option<PlaylistRequestInfo> {
        let mut i = 0;
        while i < self.pending_playlist_requests.len() {
            if self.pending_playlist_requests[i].host_id == host_id {
                let req = self.pending_playlist_requests.remove(i);
                return Some(req);
            } else {
                i += 1;
            }
        }
        None
    }

    fn end_pending_segment_request(&mut self, host_id: RequestId) -> Option<SegmentRequestInfo> {
        let mut i = 0;
        while i < self.pending_segment_requests.len() {
            if self.pending_segment_requests[i].host_id == host_id {
                let removed = self.pending_segment_requests.remove(i);
                self.check_segment_queue();
                return Some(removed);
            } else {
                i += 1;
            }
        }
        None
    }

    fn retry_pending_segment_request(
        &mut self,
        pos: usize,
        reason: RequestErrorReason,
        status: Option<u32>,
    ) -> RetryResult<'_> {
        let req = self.pending_segment_requests.get(pos).unwrap();
        let max_retry = self.config.segment_request_max_retry;
        if max_retry >= 0 && req.attempts_failed >= (max_retry as u32) {
            log_info!(
                "Req: Too much attempts for segment request id:{} a:{}",
                req.host_id,
                req.attempts_failed
            );
            let seg = self.pending_segment_requests.remove(pos);
            RetryResult::Failed {
                request_type: FinishedRequestType::Segment(seg),
                reason,
                status,
            }
        } else {
            let req = self.pending_segment_requests.get_mut(pos).unwrap();
            req.attempts_failed += 1;
            req.is_waiting_for_retry = true;
            let retry_delay = get_waiting_delay(
                req.attempts_failed,
                self.config.segment_backoff_base,
                self.config.segment_backoff_max,
            );
            log_info!(
                "Req: Retrying segment request after timer id:{} d:{} a:{}",
                req.host_id,
                retry_delay,
                req.attempts_failed
            );
            let timer_id = jsTimer(retry_delay, TimerReason::RetryRequest);
            self.retry_timers.push((timer_id, req.host_id));
            let req = self.pending_segment_requests.get(pos).unwrap();
            RetryResult::RetriedSegment {
                reason,
                status,
                request_info: req,
            }
        }
    }

    fn retry_playlist_request(
        &mut self,
        pos: usize,
        reason: RequestErrorReason,
        status: Option<u32>,
        max_retry: i32,
    ) -> RetryResult<'_> {
        let req = self.pending_playlist_requests.get(pos).unwrap();
        if max_retry >= 0 && req.attempts_failed >= (max_retry as u32) {
            log_info!(
                "Req: Too much attempts for playlist request id:{} a:{}",
                req.host_id,
                req.attempts_failed
            );
            let pl = self.pending_playlist_requests.remove(pos);
            RetryResult::Failed {
                request_type: FinishedRequestType::Playlist(pl),
                reason,
                status,
            }
        } else {
            let req = self.pending_playlist_requests.get_mut(pos).unwrap();
            req.attempts_failed += 1;
            req.is_waiting_for_retry = true;
            let (base, max) = match req.playlist_type {
                PlaylistFileType::TopLevelPlaylist => (
                    self.config.multi_variant_playlist_backoff_base,
                    self.config.multi_variant_playlist_backoff_max,
                ),
                PlaylistFileType::MediaPlaylist { .. } => (
                    self.config.media_playlist_backoff_base,
                    self.config.media_playlist_backoff_max,
                ),
            };
            let retry_delay = get_waiting_delay(req.attempts_failed, base, max);
            log_info!(
                "Req: Retrying playlist request after timer id:{} d:{} a:{}",
                req.host_id,
                retry_delay,
                req.attempts_failed
            );
            let timer_id = jsTimer(retry_delay, TimerReason::RetryRequest);
            self.retry_timers.push((timer_id, req.host_id));

            let req = self.pending_playlist_requests.get(pos).unwrap();
            RetryResult::RetriedPlaylist {
                reason,
                status,
                request_info: req,
            }
        }
    }

    fn retry_pending_playlist_request(
        &mut self,
        host_id: RequestId,
        reason: RequestErrorReason,
        status: Option<u32>,
    ) -> RetryResult<'_> {
        let pos = self
            .pending_playlist_requests
            .iter_mut()
            .position(|x| x.host_id == host_id);
        if let Some(pos) = pos {
            if self.pending_playlist_requests[pos].attempts_failed >= 3 {
                let seg = self.pending_playlist_requests.remove(pos);
                RetryResult::Failed {
                    request_type: FinishedRequestType::Playlist(seg),
                    reason,
                    status,
                }
            } else {
                let req = self.pending_playlist_requests.get_mut(pos).unwrap();
                req.attempts_failed += 1;
                req.is_waiting_for_retry = true;
                let timer_id = jsTimer(1000., TimerReason::RetryRequest);
                self.retry_timers.push((timer_id, req.host_id));
                RetryResult::RetriedPlaylist {
                    reason,
                    status,
                    request_info: req,
                }
            }
        } else {
            RetryResult::NotFound
        }
    }

    fn check_segment_queue(&mut self) {
        if self.segment_request_locked || self.segment_waiting_queue.is_empty() {
            return;
        }
        if let Some(base_pos) = self.base_position {
            if let Some(new_min_prio) = self.min_priority_for_base(base_pos) {
                // TODO drain_filter when it's stabilized
                let indexes_of_segment_to_request: Vec<usize> = self
                    .segment_waiting_queue
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| get_segment_priority(w.start_time(), base_pos) <= new_min_prio)
                    .map(|w| w.0)
                    .collect();

                indexes_of_segment_to_request.iter().enumerate().for_each(
                    |(enum_idx, original_idx)| {
                        // We sadly have to subtract `enum_idx` to account for already removed items
                        let seg = self.segment_waiting_queue.remove(original_idx - enum_idx);
                        self.request_segment_now(
                            &seg.url,
                            seg.byte_range.as_ref(),
                            seg.lane_tag,
                            seg.time_info,
                            seg.caller_id,
                        );
                    },
                );
            }
        } else {
            while let Some(seg) = self.segment_waiting_queue.pop() {
                self.request_segment_now(
                    &seg.url,
                    seg.byte_range.as_ref(),
                    seg.lane_tag,
                    seg.time_info,
                    seg.caller_id,
                );
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
                    let first_pending_priority =
                        get_segment_priority(self.pending_segment_requests[0].start_time(), pos);
                    Some(self.pending_segment_requests.iter().skip(1).fold(
                        first_pending_priority,
                        |acc, req| {
                            let priority = get_segment_priority(req.start_time(), pos);
                            if priority < acc {
                                priority
                            } else {
                                acc
                            }
                        },
                    ))
                }
            }
        }
    }

    fn can_start_request(&self, start_time: f64) -> bool {
        if self.segment_request_locked {
            return false;
        }
        let min_pending_priority = self.min_pending_priority();
        if let (Some(pos), Some(min_pending_priority)) = (self.base_position, min_pending_priority)
        {
            PriorityLevel::from_time_distance(start_time - pos) <= min_pending_priority
        } else {
            true
        }
    }

    fn request_segment_now(
        &mut self,
        url: &Url,
        byte_range: Option<&ByteRange>,
        lane_tag: RequestLaneTag,
        time_info: Option<SegmentTimeInfo>,
        caller_id: u32,
    ) {
        let (range_start, range_end) = format_range_for_js(byte_range);
        let url_ref = url.get_ref();
        let host_id = jsFetch(
            url_ref,
            range_start,
            range_end,
            self.config.segment_request_timeout,
        );
        log_debug!("Req: Performing segment request. u:{url_ref} id:{host_id}");
        self.pending_segment_requests.push(SegmentRequestInfo {
            host_id,
            lane_tag,
            url: url.clone(),
            byte_range: byte_range.cloned(),
            time_info,
            caller_id,
            attempts_failed: 0,
            is_waiting_for_retry: false,
        });
    }

    fn min_priority_for_base(&self, base_pos: f64) -> Option<PriorityLevel> {
        let min_pending_priority = self.min_pending_priority();
        self.segment_waiting_queue
            .iter()
            .enumerate()
            .fold(min_pending_priority, |acc, (_, w)| {
                let w_prio = get_segment_priority(w.start_time(), base_pos);
                match acc {
                    None => Some(w_prio),
                    Some(priority) => Some(w_prio.min(priority)),
                }
            })
    }
}

fn log_segment_abort(seg: &impl RequesterSegmentInfo) {
    log_info!(lazy: || {
        let lane_label = seg.lane_tag().label();
        if let (Some(start), Some(duration)) = (seg.start_time(), seg.duration()) {
            format!("Req: Aborting {lane_label} segment: t: {start}, d: {duration}")
        } else {
            format!("Req: Aborting {lane_label} init segment")
        }
    });
}

fn pending_segment_could_fill_before<T: RequesterSegmentInfo>(
    requests: &[T],
    range_start: f64,
) -> bool {
    requests.iter().any(
        |req| match (req.lane_tag().media_type(), req.start_time()) {
            (Some(_), None) => true,
            (Some(_), Some(start)) => start < range_start,
            (None, _) => false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        pending_segment_could_fill_before, PlaylistFileType, PlaylistRequestInfo, Requester,
        WaitingSegmentInfo,
    };
    use crate::{parser::SegmentTimeInfo, requester::RequestLaneTag, utils::url::Url};

    fn waiting_media(start: f64) -> WaitingSegmentInfo {
        WaitingSegmentInfo {
            lane_tag: RequestLaneTag::Video,
            url: Url::new("https://example.com/seg.ts".to_string()),
            byte_range: None,
            time_info: Some(SegmentTimeInfo::new(start, 2.0)),
            caller_id: 0,
        }
    }

    fn waiting_init() -> WaitingSegmentInfo {
        WaitingSegmentInfo {
            lane_tag: RequestLaneTag::Video,
            url: Url::new("https://example.com/init.mp4".to_string()),
            byte_range: None,
            time_info: None,
            caller_id: 0,
        }
    }

    #[test]
    fn pending_media_before_range_blocks_gap_jump() {
        assert!(pending_segment_could_fill_before(
            &[waiting_media(9.5)],
            12.0
        ));
        assert!(!pending_segment_could_fill_before(
            &[waiting_media(12.0)],
            12.0
        ));
    }

    #[test]
    fn pending_init_blocks_gap_jump_conservatively() {
        assert!(pending_segment_could_fill_before(&[waiting_init()], 12.0));
    }

    #[test]
    fn requesting_playlist_detects_matching_pending_media_playlist() {
        let url = Url::new("https://example.com/media.m3u8".to_string());
        let playlist_type = PlaylistFileType::TopLevelPlaylist;
        let requester = Requester {
            pending_playlist_requests: vec![PlaylistRequestInfo {
                host_id: 1,
                url: url.clone(),
                playlist_type: PlaylistFileType::TopLevelPlaylist,
                attempts_failed: 0,
                is_waiting_for_retry: false,
            }],
            pending_segment_requests: vec![],
            segment_waiting_queue: vec![],
            retry_timers: vec![],
            segment_request_locked: false,
            base_position: None,
            config: super::RequesterConfiguration::default(),
        };

        assert!(requester.is_requesting_playlist(&url, &playlist_type));
    }
}

fn get_waiting_delay(retry_attempt: u32, base: f64, max: f64) -> f64 {
    let delay = f64::min(base * f64::from(u32::pow(2, retry_attempt - 1)), max);
    let fuzzing_factor = (jsGetRandom() * 2. - 1.) * 0.3; // Max 1.3 Min 0.7
    delay * (fuzzing_factor + 1.)
}

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
        let step_info = PRIORITY_STEPS
            .iter()
            .enumerate()
            .find(|(_, step)| distance < **step);
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
        Some(start_time) => PriorityLevel::from_time_distance(start_time - current_time),
        _ => PriorityLevel::ExtremelyHigh,
    }
}
