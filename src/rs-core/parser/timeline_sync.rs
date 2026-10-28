use crate::utils::logger::*;

use super::MediaSegmentInfo;

/// Extracted information from a parsed Media Playlist which can be useful for
/// synchronization when parsing another media playlist.
#[derive(Clone, Debug)]
pub(super) struct TimelineReference {
    /// Whether at least one EXT-X-PROGRAM-DATE-TIME tag was found in the playlist.
    /// This greatly simplifies synchronization.
    uses_program_date_time: bool,
    /// Sequence number of the first announced segment in that playlist.
    first_sequence: Option<u32>,
    /// `start`, in seconds, for the first announced segment in that playlist.
    first_start: Option<f64>,
    /// Sequence number of the last announced segment in that playlist.
    last_sequence: Option<u32>,
    /// `end`, in seconds, for the last announced segment in that playlist.
    last_end: Option<f64>,
    /// `start` time in seconds for every segments announced in that playlist.
    segment_starts: Vec<f64>,
    /// Context linked to every Announced "HLS discontinuity" in that playlist.
    discontinuities: Vec<TimelineReferenceDiscontinuity>,
    /// Selected "EXT-X-PROGRAM-DATE-TIME" value at the middle of the playlist.
    /// The middle of the playlist is chosen because it is a more likely candidate to be retrieved
    /// in another playlist than the first (because since not available) or last (because too new)
    /// segments.
    middle_pdt_anchor: Option<TimelineReferencePdtAnchor>,
}

impl TimelineReference {
    /// Construct the `TimelineReference` from a reference list of media segments, generally with
    /// the intent of later synchronizing another playlist to it.
    pub(super) fn from_segment_list(
        media: &[MediaSegmentInfo],
        uses_program_date_time: bool,
    ) -> Self {
        let middle_pdt_anchor = media
            .iter()
            .filter_map(|segment| {
                Some(TimelineReferencePdtAnchor {
                    discontinuity_sequence: segment.discontinuity_sequence(),
                    start: segment.start(),
                    program_date_time: segment.program_date_time?,
                })
            })
            .nth(media.len() / 2);

        let mut discontinuities: Vec<TimelineReferenceDiscontinuity> = Vec::new();
        for segment in media {
            match discontinuities.last_mut() {
                Some(entry) if entry.discontinuity_sequence == segment.discontinuity_sequence() => {
                    if entry.program_date_time.is_none() {
                        entry.program_date_time = segment.program_date_time()
                    }
                }
                _ => discontinuities.push(TimelineReferenceDiscontinuity {
                    discontinuity_sequence: segment.discontinuity_sequence(),
                    start: segment.start(),
                    program_date_time: segment.program_date_time(),
                }),
            }
        }

        Self {
            uses_program_date_time,
            first_sequence: media.first().map(|segment| segment.sequence()),
            first_start: media.first().map(MediaSegmentInfo::start),
            last_sequence: media.last().map(|segment| segment.sequence()),
            last_end: media.last().map(MediaSegmentInfo::end),
            segment_starts: media.iter().map(MediaSegmentInfo::start).collect(),
            discontinuities,
            middle_pdt_anchor,
        }
    }

    pub(super) fn infer_offset_for(&self, media_segments: &[MediaSegmentInfo]) -> Option<f64> {
        self.align_by_discontinuity_sequence(media_segments)
            .or_else(|| self.align_by_program_date_time(media_segments))
            .or_else(|| self.align_by_sequence_number(media_segments))
    }

    pub(super) fn uses_program_date_time(&self) -> bool {
        self.uses_program_date_time
    }

    // ---- Private methods ----

    fn first_discontinuity_sequence(&self) -> Option<u32> {
        self.discontinuities
            .first()
            .map(|entry| entry.discontinuity_sequence)
    }

    fn last_discontinuity_sequence(&self) -> Option<u32> {
        self.discontinuities
            .last()
            .map(|entry| entry.discontinuity_sequence)
    }

    fn discontinuity_start(&self, discontinuity_sequence: u32) -> Option<f64> {
        self.discontinuities
            .iter()
            .find(|entry| entry.discontinuity_sequence == discontinuity_sequence)
            .map(|entry| entry.start)
    }

    fn discontinuity_pdt_anchor(
        &self,
        discontinuity_sequence: u32,
    ) -> Option<TimelineReferencePdtAnchor> {
        self.discontinuities
            .iter()
            .find(|entry| entry.discontinuity_sequence == discontinuity_sequence)
            .and_then(|entry| {
                Some(TimelineReferencePdtAnchor {
                    discontinuity_sequence: entry.discontinuity_sequence,
                    start: entry.start,
                    program_date_time: entry.program_date_time?,
                })
            })
    }

    fn start_for_sequence(&self, sequence: u32) -> Option<f64> {
        let first_sequence = self.first_sequence?;
        let idx = usize::try_from(sequence.checked_sub(first_sequence)?).ok()?;
        self.segment_starts.get(idx).copied()
    }

    fn align_by_discontinuity_sequence(&self, media_segments: &[MediaSegmentInfo]) -> Option<f64> {
        let reference_start_cc = self.first_discontinuity_sequence()?;
        let reference_end_cc = self.last_discontinuity_sequence()?;
        let start_cc = media_segments.first()?.discontinuity_sequence();
        let end_cc = media_segments.last()?.discontinuity_sequence();
        if !(start_cc < reference_end_cc && end_cc > reference_start_cc) {
            return None;
        }

        let target_cc = reference_end_cc.min(end_cc);
        let ref_start = self.discontinuity_start(target_cc)?;
        let seg = media_segments
            .iter()
            .find(|seg| seg.discontinuity_sequence() == target_cc)?;
        let delta = ref_start - seg.start();
        log_debug!(
            "Parser: aligning playlist using discontinuity sequence {} (diff:{})",
            target_cc,
            delta
        );
        Some(delta)
    }

    fn align_by_program_date_time(&self, media_segments: &[MediaSegmentInfo]) -> Option<f64> {
        if !self.uses_program_date_time {
            return None;
        }
        if !media_segments
            .iter()
            .any(|seg| seg.program_date_time.is_some())
        {
            return None;
        }

        let reference_start_cc = self.first_discontinuity_sequence()?;
        let reference_end_cc = self.last_discontinuity_sequence()?;
        let start_cc = media_segments.first()?.discontinuity_sequence();
        let end_cc = media_segments.last()?.discontinuity_sequence();
        let target_cc = reference_end_cc.min(end_cc);

        let (ref_anchor, seg) = if reference_start_cc < target_cc && start_cc < target_cc {
            (
                self.discontinuity_pdt_anchor(target_cc),
                media_segments
                    .iter()
                    .find(|seg| seg.discontinuity_sequence() == target_cc),
            )
        } else {
            (None, None)
        };

        let ref_anchor = ref_anchor.or(self.middle_pdt_anchor)?;
        let seg = seg.unwrap_or_else(|| {
            media_segments
                .iter()
                .find(|seg| seg.discontinuity_sequence() == ref_anchor.discontinuity_sequence)
                .unwrap_or(&media_segments[media_segments.len() / 2])
        });

        let ref_pdt = ref_anchor.program_date_time;
        let pdt = seg.program_date_time()?;
        let date_difference = pdt - ref_pdt;
        let total_duration = media_segments
            .last()
            .map(|last| {
                last.end()
                    - media_segments
                        .first()
                        .map(|first| first.start())
                        .unwrap_or(0.)
            })
            .unwrap_or(0.);
        if date_difference.abs() > f64::max(60., total_duration) {
            log_debug!(
                "Parser: refusing PDT alignment without overlap ({} > {})",
                date_difference.abs(),
                total_duration
            );
            return None;
        }

        let delta = date_difference - (seg.start() - ref_anchor.start);
        log_debug!("Parser: aligning playlist using PDT (diff:{})", delta);
        Some(delta)
    }

    // TODO: Revisit this cross-playlist fallback: HLS permits independent media sequence
    // numbers and forbids using them to synchronize streams (RFC 8216 sections 6.3.2/6.3.4).
    // Keep the existing compatibility heuristic for now; use reliable timing anchors instead.
    fn align_by_sequence_number(&self, media_segments: &[MediaSegmentInfo]) -> Option<f64> {
        let new_first = media_segments.first()?;
        let ref_last_sequence = self.last_sequence?;
        let ref_last_end = self.last_end?;

        if let Some(ref_start) = self.start_for_sequence(new_first.sequence()) {
            let offset = ref_start - new_first.start();
            log_debug!(
                "Parser: aligning playlist based on media sequence {} (diff:{})",
                new_first.sequence(),
                offset
            );
            return Some(offset);
        }

        if ref_last_sequence.wrapping_add(1) == new_first.sequence() {
            let offset = ref_last_end - new_first.start();
            log_debug!(
                "Parser: aligning playlist based on first/last media sequence {} (diff:{})",
                new_first.sequence(),
                offset
            );
            return Some(offset);
        }

        None
    }
}

/// Context of an "EXT-X-PROGRAM-DATE-TIME" value usable for synchronization with another playlist.
#[derive(Clone, Copy, Debug)]
struct TimelineReferencePdtAnchor {
    /// The discontinuity sequence number corresponding to the segment having this program date time
    discontinuity_sequence: u32,
    /// The start time in seconds of the segment having this program date time
    start: f64,
    /// The program date time of that segment.
    program_date_time: f64,
}

/// Extracted information about an HLS media playlist "discontinuity", which may be useful for
/// synchronizing other media playlists with it.
#[derive(Clone, Debug)]
struct TimelineReferenceDiscontinuity {
    /// The discontinuity sequence number of that discontinuity.
    discontinuity_sequence: u32,
    /// The first post-discontinuity segment's start time in seconds.
    start: f64,
    /// Program date time associated to that discontinuity sequence, if known.
    program_date_time: Option<f64>,
}
