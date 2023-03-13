use crate::{
    bindings::{
        MediaType,
        jsFetch,
        RequestId,
        jsAbortRequest,
        jsTimer,
        TimerReason,
        TimerId, jsGetRandom, RequestErrorReason,
        formatters::format_range_for_js,
    },
    Logger,
    content_tracker::MediaPlaylistPermanentId,
    parser::{SegmentInfo, ByteRange},
    utils::url::Url,
};

const PRIORITY_STEPS : [f64; 6] = [2., 4., 8., 12., 18., 25.];

const DEFAULT_BACKOFF_BASE: f64 = 300.;
const DEFAULT_BACKOFF_MAX: f64 = 3000.;

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

/// The `Requester` is the module performing HTTP(s) requests.
///
/// Depending on the nature of the resource and on its configuration, it also
/// has both a request-scheduling mechanism, allowing to perform more urgent
/// request first, and a retry mechanism based on an exponential backoff delay,
/// to retry requesting resources without overloading the server serving them.
pub(crate) struct Requester {
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

    /// Depending the nature of the failure, failed requests might be retried.
    ///
    /// To avoid overloading the server serving those resources, retried
    /// requests are actually performed after a timer.
    /// This variable allows to store and link here a timer's TimerId (which
    /// will be communicated back when the timer has elapsed) to the RequestId.
    ///
    /// Note that retried segment requests stay in the
    /// `pending_segment_requests` vector and retried playlist requests stay in
    /// the `pending_playlist_requests` vector, even when the request is not
    /// really pending.
    retry_timers: Vec<(TimerId, RequestId)>,

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

    /// Timeout, in milliseconds, used for segment requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `None` to disable.
    segment_request_timeout: Option<f64>,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff
    /// for a retried segment request should be.
    segment_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay
    /// for a retried segment request should be.
    segment_backoff_max: f64,

    /// Timeout, in milliseconds, used for MultiVariant playlist requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `None` to disable.
    multi_variant_playlist_request_timeout: Option<f64>,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff
    /// for a retried MultiVariant Playlist request should be.
    multi_variant_playlist_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay
    /// for a retried MultiVariant Playlist request should be.
    multi_variant_playlist_backoff_max: f64,

    /// Timeout, in milliseconds, used for Media playlist requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `None` to disable.
    media_playlist_request_timeout: Option<f64>,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff
    /// for a retried Media Playlist request should be.
    media_playlist_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the
    /// server.
    /// That timeout then grows exponentially the more the request has to be
    /// retried (in the case it fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay
    /// for a retried Media Playlist request should be.
    media_playlist_backoff_max: f64,
}

#[derive(PartialEq)]
pub(crate) enum PlaylistFileType {
    MultiVariantPlaylist,
    MediaPlaylist { id: MediaPlaylistPermanentId },
    Unknown,
}

/// Metadata associated with a pending Playlist (either a MultiVariant Playlist or a Media
/// Playlist request.
pub(crate) struct PlaylistRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    request_id: RequestId,

    /// Url on which the request is done
    pub(crate) url: Url,

    /// Type of the Playlist that is requested
    pub(crate) playlist_type: PlaylistFileType,

    /// Number of time the request has already been attempted.
    pub(crate) attempts_failed: u32,

    /// If `true` the request is not really pending, we're currently pending for some
    /// timer to finish before retrying it.
    ///
    /// In that case, the `request_id` corresponds to the one of the previous request
    /// and should not be relied on.
    pub(crate) is_waiting_for_retry: bool,
}

/// Metadata associated with a pending media segment request.
pub struct WaitingSegmentInfo {
    /// type of media of the segment requested
    pub(crate) media_type: MediaType,

    /// Url on which the request is done
    pub(crate) url: Url,

    pub(crate) byte_range: Option<ByteRange>,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    pub(crate) time_info: Option<(f64, f64)>,
}

pub(crate) trait RequesterSegmentInfo {
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
pub(crate) struct SegmentRequestInfo {
    /// ID identifying the request on the JavaScript-side.
    request_id: RequestId,

    /// type of media of the segment requested
    pub(crate) media_type: MediaType,

    /// Url on which the request is done
    pub(crate) url: Url,

    pub(crate) byte_range: Option<ByteRange>,

    /// Start and end of the requested segment.
    /// `None` if the segment contains no media data, such as initialization segments
    pub(crate) time_info: Option<(f64, f64)>,

    /// Number of time the request has already been attempted.
    pub(crate) attempts_failed: u32,

    /// If `true` the request is not really pending, we're currently pending for some
    /// timer to finish before retrying it.
    ///
    /// In that case, the `request_id` corresponds to the one of the previous request
    /// and should not be relied on.
    pub(crate) is_waiting_for_retry: bool,
}

pub(crate) enum FinishedRequestType {
    Playlist(PlaylistRequestInfo),
    Segment(SegmentRequestInfo),
}

pub(crate) enum RetryResult {
    NotFound,
    Retried,
    Failed((FinishedRequestType, RequestErrorReason)),
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
            segment_request_timeout: None,
            multi_variant_playlist_request_timeout: None,
            media_playlist_request_timeout: None,
            segment_backoff_base: DEFAULT_BACKOFF_BASE,
            segment_backoff_max: DEFAULT_BACKOFF_MAX,
            multi_variant_playlist_backoff_base: DEFAULT_BACKOFF_BASE,
            multi_variant_playlist_backoff_max: DEFAULT_BACKOFF_MAX,
            media_playlist_backoff_base: DEFAULT_BACKOFF_BASE,
            media_playlist_backoff_max: DEFAULT_BACKOFF_MAX,
        }
    }

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

    #[inline(always)]
    pub(crate) fn segment_request_timeout(&mut self) -> Option<f64> {
        self.segment_request_timeout
    }

    #[inline(always)]
    pub(crate) fn segment_backoff_base(&mut self) -> f64 {
        self.segment_backoff_base
    }

    #[inline(always)]
    pub(crate) fn segment_backoff_max(&mut self) -> f64 {
        self.segment_backoff_max
    }

    #[inline(always)]
    pub(crate) fn multi_variant_playlist_request_timeout(&mut self) -> Option<f64> {
        self.multi_variant_playlist_request_timeout
    }

    #[inline(always)]
    pub(crate) fn multi_variant_playlist_backoff_base(&mut self) -> f64 {
        self.multi_variant_playlist_backoff_base
    }

    #[inline(always)]
    pub(crate) fn multi_variant_playlist_backoff_max(&mut self) -> f64 {
        self.multi_variant_playlist_backoff_max
    }

    #[inline(always)]
    pub(crate) fn media_playlist_request_timeout(&mut self) -> Option<f64> {
        self.media_playlist_request_timeout
    }

    #[inline(always)]
    pub(crate) fn media_playlist_backoff_base(&mut self) -> f64 {
        self.media_playlist_backoff_base
    }

    #[inline(always)]
    pub(crate) fn media_playlist_backoff_max(&mut self) -> f64 {
        self.media_playlist_backoff_max
    }

    #[inline(always)]
    pub(crate) fn update_segment_request_timeout(&mut self, timeout: Option<f64>) {
        self.segment_request_timeout = timeout;
    }

    #[inline(always)]
    pub(crate) fn update_segment_backoff_base(&mut self, base: f64) {
        self.segment_backoff_base = base;
    }

    #[inline(always)]
    pub(crate) fn update_segment_backoff_max(&mut self, max: f64) {
        self.segment_backoff_max = max;
    }

    #[inline(always)]
    pub(crate) fn update_multi_variant_playlist_request_timeout(&mut self, timeout: Option<f64>) {
        self.multi_variant_playlist_request_timeout = timeout;
    }

    #[inline(always)]
    pub(crate) fn update_multi_variant_playlist_backoff_base(&mut self, base: f64) {
        self.multi_variant_playlist_backoff_base = base;
    }

    #[inline(always)]
    pub(crate) fn update_multi_variant_playlist_backoff_max(&mut self, max: f64) {
        self.multi_variant_playlist_backoff_max = max;
    }

    #[inline(always)]
    pub(crate) fn update_media_playlist_request_timeout(&mut self, timeout: Option<f64>) {
        self.media_playlist_request_timeout = timeout;
    }

    #[inline(always)]
    pub(crate) fn update_media_playlist_backoff_base(&mut self, base: f64) {
        self.media_playlist_backoff_base = base;
    }

    #[inline(always)]
    pub(crate) fn update_media_playlist_backoff_max(&mut self, max: f64) {
        self.media_playlist_backoff_max = max;
    }

    /// Fetch either the MultiVariantPlaylist or a MediaPlaylist reachable
    /// through the given `url` and add its `request_id` to `pending_playlist_requests`.
    ///
    /// Once it succeeds, the `on_request_finished` function will be called.
    pub(crate) fn fetch_playlist(&mut self, url: Url, playlist_type: PlaylistFileType) {
        let timeout = match playlist_type {
            PlaylistFileType::MultiVariantPlaylist =>
                self.multi_variant_playlist_request_timeout,
            PlaylistFileType::MediaPlaylist { .. } =>
                self.media_playlist_request_timeout,
            _ => None,
        };
        let url_ref = url.get_ref();
        let request_id = jsFetch(url_ref, None, None, timeout);
        Logger::info(&format!("Req: Fetching playlist u:{url_ref}, id:{request_id}"));
        self.pending_playlist_requests.push(PlaylistRequestInfo {
            request_id,
            url,
            playlist_type,
            attempts_failed: 0,
            is_waiting_for_retry: false,
        });
    }

    /// Fetch the initialization segment whose metadata is given here add its
    /// `request_id` to `pending_segment_requests`.
    ///
    /// Once it succeeds, the `on_request_finished` function will be called.
    pub(crate) fn request_init_segment(&mut self, media_type: MediaType, url: Url, byte_range: Option<&ByteRange>) {
        let (range_start, range_end) = format_range_for_js(byte_range);
        let url_ref = url.get_ref();
        let request_id = jsFetch(
            url_ref,
            range_start, range_end,
            self.segment_request_timeout);
        Logger::info(&format!("Req: Fetching init segment u:{url_ref}, id:{request_id}"));
        self.pending_segment_requests.push(SegmentRequestInfo {
            request_id,
            media_type,
            url,
            byte_range: byte_range.cloned(),
            time_info: None,
            attempts_failed: 0,
            is_waiting_for_retry: false,
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
                byte_range: seg.byte_range.clone(),
                time_info,
            });
        } else {
            let (range_start, range_end) = format_range_for_js(seg.byte_range.as_ref());
            let request_id = jsFetch(seg.url.get_ref(),
                range_start, range_end,
                self.segment_request_timeout);
            Logger::debug(&format!("Req: Performing request right away. u:{} id:{request_id}",
                    seg.url.get_ref()));
            self.pending_segment_requests.push(SegmentRequestInfo {
                request_id,
                media_type,
                url: seg.url.clone(),
                byte_range: seg.byte_range.clone(),
                time_info,
                attempts_failed: 0,
                is_waiting_for_retry: false,
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

    pub(crate) fn on_pending_request_success(&mut self,
        request_id: RequestId
    ) -> Option<FinishedRequestType> {
        self.end_pending_request(request_id)
    }

    pub(crate) fn on_pending_request_failure(&mut self,
        request_id: RequestId,
        has_timeouted: bool,
        status: Option<u32>
    ) -> RetryResult {
        let reason = match (has_timeouted, status) {
            (true, _) => Some(RequestErrorReason::Timeout),
            (false, Some(x)) if x == 404 || x == 412 || x >= 500 =>
                Some(RequestErrorReason::Status),
            _ => None
        };
        if let Some(reason) = reason {
            if let Some(pos) = self.pending_segment_requests
                .iter()
                .position(|x| x.request_id == request_id)
            {
                self.retry_pending_segment_request(pos, reason)
            } else if let Some(pos) = self.pending_playlist_requests
                .iter()
                    .position(|x| x.request_id == request_id)
            {
                self.retry_playlist_segment_request(pos, reason)
            } else {
                Logger::info(&format!("Req: Request to retry not found, id:{request_id}"));
                RetryResult::NotFound
            }
        } else {
            Logger::info(&format!("Req: Cannot retry request id:{request_id}"));
            match  self.end_pending_request(request_id) {
                None => RetryResult::NotFound,
                Some(req) => RetryResult::Failed((req, RequestErrorReason::Error))
            }
        }
    }

    pub(crate) fn on_timer_finished(&mut self,
        timer_id: TimerId
    ) {
        let mut i = 0;
        while i < self.retry_timers.len() {
            if self.retry_timers[i].0 == timer_id {
                let timer = self.retry_timers.remove(i);
                let seg = self.pending_segment_requests.iter_mut().find(|s| {
                    s.request_id == timer.1
                });
                if let Some(seg) = seg {
                    seg.is_waiting_for_retry = false;
                    let (range_start, range_end) = format_range_for_js(seg.byte_range.as_ref());
                    let request_id = jsFetch(seg.url.get_ref(),
                        range_start, range_end,
                        self.segment_request_timeout);
                    seg.request_id = request_id;
                } else {
                    let pla = self.pending_playlist_requests.iter_mut().find(|p| {
                            p.request_id == timer.1
                    });
                    if let Some(pla) = pla {
                        pla.is_waiting_for_retry = false;
                        let timeout = match pla.playlist_type {
                            PlaylistFileType::MultiVariantPlaylist =>
                                self.multi_variant_playlist_request_timeout,
                            PlaylistFileType::MediaPlaylist { .. } =>
                                self.media_playlist_request_timeout,
                            PlaylistFileType::Unknown => None,
                        };
                        let request_id = jsFetch(pla.url.get_ref(), None, None, timeout);
                        pla.request_id = request_id;
                    }
                }
            } else {
                i += 1;
            }
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

    fn end_pending_request(&mut self,
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

    fn retry_pending_segment_request(&mut self, pos: usize, reason: RequestErrorReason) -> RetryResult {
       let req = &mut self.pending_segment_requests[pos];
       if req.attempts_failed >= 3 {
           Logger::info(&format!("Req: Too much attempts for segment request id:{} a:{}",
                   req.request_id, req.attempts_failed));
           let seg = self.pending_segment_requests.remove(pos);
           RetryResult::Failed((FinishedRequestType::Segment(seg), reason))
       } else {
           req.attempts_failed += 1;
           req.is_waiting_for_retry = true;
           let retry_delay = get_waiting_delay(
               req.attempts_failed, self.segment_backoff_base, self.segment_backoff_max);
           Logger::info(&format!("Req: Retrying segment request after timer id:{} d:{} a:{}",
                   req.request_id, retry_delay, req.attempts_failed));
           let timer_id = jsTimer(retry_delay, TimerReason::RetryRequest);
           self.retry_timers.push((timer_id, req.request_id));
           RetryResult::Retried
       }
    }

    fn retry_playlist_segment_request(&mut self, pos: usize, reason: RequestErrorReason) -> RetryResult {
       let req = &mut self.pending_playlist_requests[pos];
       if req.attempts_failed >= 3 {
           Logger::info(&format!("Req: Too much attempts for playlist request id:{} a:{}",
                   req.request_id, req.attempts_failed));
           let pl = self.pending_playlist_requests.remove(pos);
           RetryResult::Failed((FinishedRequestType::Playlist(pl), reason))
       } else {
           req.attempts_failed += 1;
           req.is_waiting_for_retry = true;
           let (base, max) = match req.playlist_type {
                PlaylistFileType::MultiVariantPlaylist => (
                    self.multi_variant_playlist_backoff_base,
                    self.multi_variant_playlist_backoff_max),
                PlaylistFileType::MediaPlaylist { .. } => (
                    self.media_playlist_backoff_base,
                    self.media_playlist_backoff_max),
                _ => (DEFAULT_BACKOFF_BASE, DEFAULT_BACKOFF_MAX),
           };
           let retry_delay = get_waiting_delay(req.attempts_failed, base, max);
           Logger::info(&format!("Req: Retrying playlist request after timer id:{} d:{} a:{}",
                   req.request_id, retry_delay, req.attempts_failed));
           let timer_id = jsTimer(retry_delay, TimerReason::RetryRequest);
           self.retry_timers.push((timer_id, req.request_id));
           RetryResult::Retried
       }
    }

    fn retry_pending_playlist_request(&mut self,
        request_id: RequestId,
        reason: RequestErrorReason
    ) -> RetryResult {
       let pos = self.pending_playlist_requests
           .iter_mut()
           .position(|x| x.request_id == request_id);
       if let Some(pos) = pos {
           if self.pending_playlist_requests[pos].attempts_failed >= 3 {
               let seg = self.pending_playlist_requests.remove(pos);
               RetryResult::Failed((FinishedRequestType::Playlist(seg), reason))
           } else {
               self.pending_playlist_requests[pos].attempts_failed += 1;
               self.pending_playlist_requests[pos].is_waiting_for_retry = true;
               let timer_id = jsTimer(1000., TimerReason::RetryRequest);
               self.retry_timers.push((timer_id, self.pending_playlist_requests[pos].request_id));
               RetryResult::Retried
           }
       } else {
           RetryResult::NotFound
       }
    }

    fn check_segment_queue(&mut self) {
        if self.segment_request_locked || self.segment_waiting_queue.is_empty() {
            return ;
        }
        if let Some(base_pos) = self.base_position {
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
                    let (range_start, range_end) = format_range_for_js(seg.byte_range.as_ref());
                    let url_ref = seg.url.get_ref();
                    let request_id = jsFetch(url_ref, range_start, range_end, self.segment_request_timeout);
                    Logger::debug(&format!("Req: Performing request of queued segment u:{url_ref} id:{request_id}"));
                    self.pending_segment_requests.push(SegmentRequestInfo {
                        request_id,
                        media_type: seg.media_type,
                        url: seg.url,
                        byte_range: seg.byte_range,
                        time_info: seg.time_info,
                        attempts_failed: 0,
                        is_waiting_for_retry: false,
                    });
                });
            }
        } else {
            while let Some(seg) = self.segment_waiting_queue.pop() {
                let (range_start, range_end) = format_range_for_js(seg.byte_range.as_ref());
                let url_ref = seg.url.get_ref();
                let request_id = jsFetch(url_ref, range_start, range_end, self.segment_request_timeout);
                Logger::debug(&format!("Req: Performing request of queued segment u:{url_ref} id:{request_id}"));
                self.pending_segment_requests.push(SegmentRequestInfo {
                    request_id,
                    media_type: seg.media_type,
                    url: seg.url,
                    byte_range: seg.byte_range,
                    time_info: seg.time_info,
                    attempts_failed: 0,
                    is_waiting_for_retry: false,
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
        if let (Some(start), Some(duration)) = (seg.start_time(), seg.duration()) {
            format!("Req: Aborting {media_type} segment: t: {start}, d: {duration}")
        } else {
            format!("Req: Aborting {media_type} init segment")
        }
    });
}

fn get_waiting_delay(retry_attempt: u32, base: f64, max: f64) -> f64 {
    let delay = f64::min(
        base * u64::pow(2, retry_attempt - 1) as f64,
        max
    );
    let fuzzing_factor = (jsGetRandom() * 2. - 1.) * 0.3; // Max 1.3 Min 0.7
    delay * (fuzzing_factor + 1.)
}
