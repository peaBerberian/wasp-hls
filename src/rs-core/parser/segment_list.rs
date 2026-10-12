use super::ByteRange;
use crate::utils::url::Url;

/// List of all segments a `MediaPlaylist` is associated to.
#[derive(Clone, Debug)]
pub(crate) struct SegmentList {
    /// Initialization segments a `MediaPlaylist` is associated to, ordered chronologically
    /// (initialization linked to earlier media segments are set first).
    pub(super) init: Vec<InitSegmentInfo>,
    /// Media segments a `MediaPlaylist` is associated to, in chronological order.
    pub(super) media: Vec<MediaSegmentInfo>,
}

impl SegmentList {
    /// Returns a reference to the potential initialization segment linked to the given media
    /// segment information.
    ///
    /// Returns `None` if the given media segment isn't linked to an initialization segment.
    pub(crate) fn init_for(&self, seg: &MediaSegmentInfo) -> Option<&InitSegmentInfo> {
        self.init.iter().rev().find(|i| i.start <= seg.start())
    }

    /// Returns the list of media segments associated to this `SegmentList` in chronological order.
    pub(crate) fn media(&self) -> &[MediaSegmentInfo] {
        self.media.as_slice()
    }

    /// Returns information on the segment including the position given, in seconds.
    ///
    /// Returns `None` if no such media segment is found.
    pub(crate) fn segment_from_pos(&self, pos: f64) -> Option<&MediaSegmentInfo> {
        self.media
            .iter()
            .find(|s| s.end() > pos && s.start() <= pos)
    }
}

/// Object storing the time information on a single media segment.
#[derive(Clone, Debug)]
pub(crate) struct SegmentTimeInfo {
    /// First presentation time the segment contains media data for, in seconds.
    pub(super) start: f64,
    /// Difference between the last presentation time at which the segment contains data for
    /// in seconds and `start`.
    pub(super) duration: f64,
}

impl SegmentTimeInfo {
    /// Create a new `SegmentTimeInfo` with the given `start` and `duration` in seconds.
    pub(crate) fn new(start: f64, duration: f64) -> Self {
        Self { start, duration }
    }

    /// First presentation time the segment contains media data for, in seconds.
    pub(crate) fn start(&self) -> f64 {
        self.start
    }

    /// Last presentation time the segment contains media data for, in seconds.
    pub(crate) fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Difference between `self.end()` and `self.start()`
    pub(crate) fn duration(&self) -> f64 {
        self.duration
    }
}

/// Information linked to an initialization segment.
#[derive(Clone, Debug)]
pub(crate) struct InitSegmentInfo {
    /// First segment's start time in seconds to which that initialization segment applies.
    pub(super) start: f64,
    /// URL through which that initialization segment may be requested.
    pub(super) url: Url,
    /// If set, byte-range to specifically request only the initialization segment at the given
    /// `url`.
    pub(super) byte_range: Option<ByteRange>,
}

impl InitSegmentInfo {
    /// Returns an identifier allowing to compare the initialization segment behind this
    /// `InitSegmentInfo` to other `InitSegmentInfo` objects coming from the same `MediaPlaylist`.
    pub(crate) fn id(&self) -> f64 {
        self.start
    }

    /// If set, byte-range at which the initialization segment should be requested.
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    /// URL at which the initialization segment should be requested.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}

/// Identity of a media segment within one media playlist lineage, independently of timestamps.
/// Compare it alongside the playlist's media ID when identifying media across playlists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SegmentIdentity {
    sequence: u32,
    discontinuity_sequence: u32,
}

impl SegmentIdentity {
    pub(crate) fn new(sequence: u32, discontinuity_sequence: u32) -> Self {
        Self {
            sequence,
            discontinuity_sequence,
        }
    }
}

/// Information linked to a single media segment.
#[derive(Clone, Debug)]
pub(crate) struct MediaSegmentInfo {
    /// Media sequence number identifying this segment within the current playlist lineage.
    /// TODO: Do we associate that one to **every segment** compated to a single base sequence
    /// number? Seems unnecessary...
    pub(super) sequence: u32,
    /// Discontinuity sequence number identifying the timeline continuity context of the segment.
    pub(super) discontinuity_sequence: u32,
    /// Information on the time boundaries of that segment.
    ///
    /// It should be exclusive to the time boundaries of all other segments in this Media Playlist.
    pub(super) time_info: SegmentTimeInfo,
    /// Program date time associated to the segment, in seconds since epoch, if known.
    /// TODO: Do we associate that one to **every segment** compared to e.g. a single base offset?
    /// Seems unnecessary...
    pub(super) program_date_time: Option<f64>,
    /// URL through which that media segment may be requested.
    pub(super) url: Url,
    /// If set, byte-range to specifically request only the media segment at the given `url`.
    pub(super) byte_range: Option<ByteRange>,
}

impl MediaSegmentInfo {
    pub(crate) fn identity(&self) -> SegmentIdentity {
        SegmentIdentity::new(self.sequence, self.discontinuity_sequence)
    }

    /// Media sequence number identifying this segment within the current playlist lineage.
    pub(crate) fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Discontinuity sequence number identifying the timeline continuity context of the segment.
    pub(crate) fn discontinuity_sequence(&self) -> u32 {
        self.discontinuity_sequence
    }

    /// First presentation time the segment contains media data for, in seconds.
    pub(crate) fn start(&self) -> f64 {
        self.time_info.start()
    }

    /// Last presentation time the segment contains media data for, in seconds.
    pub(crate) fn end(&self) -> f64 {
        self.time_info.end()
    }

    /// Difference between `self.end()` and `self.start()`
    pub(crate) fn duration(&self) -> f64 {
        self.time_info.duration()
    }

    /// Returns reference to the whole `SegmentTimeInfo` object linked to this media segment.
    pub(crate) fn time_info(&self) -> &SegmentTimeInfo {
        &self.time_info
    }

    /// Program date time associated to that segment, in seconds since epoch, if known.
    pub(crate) fn program_date_time(&self) -> Option<f64> {
        self.program_date_time
    }

    /// If set, byte-range at which this media segment should be requested.
    pub(crate) fn byte_range(&self) -> Option<&ByteRange> {
        self.byte_range.as_ref()
    }

    /// URL at which this initialization segment should be requested.
    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}
