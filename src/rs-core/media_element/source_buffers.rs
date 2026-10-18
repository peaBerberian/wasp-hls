use std::collections::VecDeque;

use crate::bindings::{
    jsAddSourceBuffer, jsAppendBuffer, jsFlush, jsRemoveBuffer, AddSourceBufferErrorCode,
    MediaType, ParsedSegmentInfo, ResourceId, SegmentParsingErrorCode, SourceBufferId,
    TimescaledTimestamp,
};
use crate::dispatcher::JsMemoryBlob;
use crate::parser::{MediaPlaylistPermanentId, SegmentTimeInfo};
use crate::utils::logger::*;

/// The failed operation and media appends cancelled before they could run.
pub(super) struct CancelledOperations {
    pub(super) current: Option<SourceBufferQueueElement>,
    pub(super) cancelled_media_ids: Vec<u64>,
}

/// Abstraction over the Media Source Extension's `SourceBuffer` concept.
///
/// This is the interface allowing to interact with lower-level media buffers.
pub(super) struct SourceBuffer {
    /// The `SourceBufferId` given on SourceBuffer creation, used to identify
    /// this `SourceBuffer` in the current dispatcher instance.
    id: SourceBufferId,

    /// The current queue of operations being performed on the `SourceBuffer`.
    ///
    /// From the most imminent to the least.
    queue: VecDeque<SourceBufferQueueElement>,

    /// The Content-Type currently linked to the SourceBuffer
    typ: String,

    /// Set to `true` as soon as the first operation is being performed, at
    /// which point, some actions cannot be taken anymore (like creating other
    /// `SourceBuffer` instances).
    was_used: bool,

    /// XXX TODO: variable name and comment out-of-place
    /// Whether segment selection has supplied everything needed through the playlist's end.
    loading_complete: bool,

    /// The MediaType associated to this `SourceBuffer`.
    media_type: MediaType,

    /// If `true`, the `SourceBuffer` was very recently emptied.
    ///
    /// In that situation, various decoding issues may occur after new data is pushed to the
    /// buffer, so special considerations, such as calling the `jsFlush` function might need to be
    /// taken on buffer updates.
    needs_reflush: bool,

    /// Set when the next media append must force a transmuxer reset regardless of its sequence
    /// identity, such as after an explicit buffer flush.
    reset_transmuxer_on_next_segment: bool,

    // See `LastSegmentContinuityInfo`
    last_pushed_segment_info: Option<LastSegmentContinuityInfo>,
}

/// To better handle dts continuity shenanigans unique to mpeg-ts segment, we maintain a complex
/// state linked to the last segment pushed in this buffer
struct LastSegmentContinuityInfo {
    /// End timing anchor of the last media segment known to have been appended successfully.
    ///
    /// When available, this exact timescaled value is used as the continuity anchor for the next
    /// transmuxed append.
    end_dts: Option<TimescaledTimestamp>,

    /// That segment's sequence_number. This makes detection of non-contiguous segment extremely
    /// easy
    sequence_number: u32,

    /// Identity of the media timeline the last appended media segment belonged to.
    ///
    /// This allows the SourceBuffer to transparently determine whether the next append can reuse
    /// the lower-level transmuxer state or should start a new sequence.
    media_sequence_identity: MediaSequenceIdentity,
}

impl SourceBuffer {
    /// Create a new `SourceBuffer` for the given `MediaType` and the mime-type indicated by `typ`.
    ///
    /// # Arguments
    ///
    /// * `media_type` - The `MediaType` that will handle this `SourceBuffer`. A `SourceBuffer`
    ///   handling multiple times at once is expected to:
    ///     1. At least contain video content
    ///     2. Be set to `MediaType::Video`, even if it also contains audio for example
    ///
    /// * `mime_type` - Mime-type to use when creating this `SourceBuffer` on the JavaScript-side.
    pub(super) fn new(media_type: MediaType, typ: String) -> Result<Self, AddSourceBufferError> {
        log_info!("Creating new {} SourceBuffer", media_type);
        match jsAddSourceBuffer(media_type, &typ) {
            Ok(x) => Ok(Self {
                id: x,
                typ,
                queue: VecDeque::new(),
                was_used: false,
                needs_reflush: false,
                loading_complete: false,
                media_type,
                reset_transmuxer_on_next_segment: false,
                last_pushed_segment_info: None,
            }),
            Err(err) => Err(AddSourceBufferError::from_js_add_source_buffer_error(
                err, &typ,
            )),
        }
    }

    /// Returns the `SourceBufferId` needed to refer to that SourceBuffer when interacting with
    /// JavaScript.
    pub(super) fn id(&self) -> SourceBufferId {
        self.id
    }

    /// Returns the `MediaType` linked to that SourceBuffer.
    pub(super) fn media_type(&self) -> MediaType {
        self.media_type
    }

    /// Returns the mime-type linked to that SourceBuffer.
    pub(super) fn mime_type(&self) -> &str {
        &self.typ
    }

    /// Returns `true` if there is at least one pending buffer operation that
    /// isn't finished yet.
    pub(super) fn has_operations_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Pushes a new initialization segment to the underlying `SourceBuffer.
    ///
    /// # Arguments
    ///
    /// * `segment_data` - Actual initialization segment's data.
    pub(super) fn push_init_segment(
        &mut self,
        segment_data: JsMemoryBlob,
    ) -> Result<AppendBufferResponse, PushSegmentError> {
        self.was_used = true;
        self.last_pushed_segment_info = None;
        self.reset_transmuxer_on_next_segment = true;
        self.queue
            .push_back(SourceBufferQueueElement::PushInit(segment_data.id()));
        log_debug!(
            "Buffer {} ({}): Pushing initialization segment",
            self.id,
            self.typ
        );
        match jsAppendBuffer(self.id, segment_data.id(), &SegmentHints::new(0, 1, true)) {
            Err(err) => Err(PushSegmentError::from_js_append_buffer_error(
                self.media_type,
                err,
            )),
            Ok(x) => Ok(AppendBufferResponse { parsed: x }),
        }
    }

    /// Pushes a new media segment to the SourceBuffer.
    ///
    /// If the `parse_time_info` bool in argument is set to `true`, the segment might be parsed to
    /// recuperate its time information which will be returned if found.
    ///
    /// # Arguments
    ///
    /// * `data` - Actual data AND metadata on the segment you want to push. See
    ///   `PreparedPushData` documentation for more information.
    ///
    /// * `parse_time_info` - If set to `true`, the segment's data will be read before pushing it
    ///   to try recuperate its timing information. If it has been parsed with success, it will
    ///   be contained in the `AppendBufferResponse` returned by this method.
    pub(super) fn push_media_segment(
        &mut self,
        data: PreparedPushData,
    ) -> Result<AppendBufferResponse, PushSegmentError> {
        self.was_used = true;
        let sequence_number = data.sequence_number;
        let segment_data = data.segment_data.id();
        let segment_time_info = data.time_info.clone();
        let media_sequence_identity = data.media_sequence_identity;
        let should_reset_transmuxer = should_reset_transmuxer(
            self.last_pushed_segment_info
                .as_ref()
                .map(|x| x.media_sequence_identity),
            media_sequence_identity,
            self.reset_transmuxer_on_next_segment,
        );

        // Only use the previous segment if the new is contiguous with it
        let prev_segment_dts = self
            .last_pushed_segment_info
            .as_ref()
            .filter(|info| info.sequence_number + 1 == sequence_number)
            .and_then(|info| info.end_dts);

        let segment_hints = build_segment_hints(
            &segment_time_info,
            prev_segment_dts,
            data.base_dts_hint,
            should_reset_transmuxer,
        );
        self.reset_transmuxer_on_next_segment = false;

        let id = data.id;
        log_debug!(
            "Buffer {} ({}): Pushing seq {}",
            self.id,
            self.typ,
            data.sequence_number
        );
        self.queue
            .push_back(SourceBufferQueueElement::PushMedia { data, id });
        let parsed = match jsAppendBuffer(self.id, segment_data, &segment_hints) {
            Err(err) => {
                return Err(PushSegmentError::from_js_append_buffer_error(
                    self.media_type,
                    err,
                ));
            }
            Ok(parsed) => parsed,
        };

        // Use the parsed segment end as a good basis for the next decode time
        self.last_pushed_segment_info = Some(LastSegmentContinuityInfo {
            sequence_number,
            end_dts: parsed.as_ref().and_then(|x| {
                x.end()
                    .map(|value| TimescaledTimestamp::new(value, x.timescale()))
            }),
            media_sequence_identity,
        });
        Ok(AppendBufferResponse { parsed })
    }

    /// Remove media data from this `SourceBuffer`, based on a `start` and `end` time in seconds.
    ///
    /// # Arguments
    ///
    /// * `start` - Start time, in seconds, of the range of time which should be removed from the
    ///   `SourceBuffer`.
    ///
    /// * `end` - End time, in seconds, of the range of time which should be removed from the
    ///   `SourceBuffer`.
    pub(super) fn remove_buffer(&mut self, start: f64, end: f64) {
        self.loading_complete = false;
        self.was_used = true;
        self.queue
            .push_back(SourceBufferQueueElement::Remove { start, end });
        log_debug!(
            "Buffer {} ({}): Removing {} {}",
            self.id,
            self.typ,
            start,
            end
        );
        let _ = jsRemoveBuffer(self.id, start, end);
    }

    /// Empty media data from this `SourceBuffer`.
    ///
    /// There's special considerations too take care of here as we'll remove data corresponding to
    /// the current position. As such a seek will have to be performed once the remove is done
    pub(super) fn flush_buffer(&mut self) {
        self.loading_complete = false;
        self.was_used = true;
        self.last_pushed_segment_info = None;
        self.reset_transmuxer_on_next_segment = true;
        self.queue.push_back(SourceBufferQueueElement::Emptying);
        log_debug!("Buffer {} ({}): emptying", self.id, self.typ);
        let _ = jsRemoveBuffer(self.id, 0., f64::INFINITY);
    }

    /// SourceBuffers maintain a queue of planned operations such as push and remove to media
    /// buffers.
    ///
    /// In some rare scenarios, we could be left in a situation where all previously scheduled
    /// operations are cancelled, such as when one of them fails.
    /// This method allows to empty that SourceBuffer's queue in such situations.
    pub(super) fn clear_queue(&mut self) {
        log_info!("Buffer {} ({}): clearing queue.", self.id, self.typ);
        self.queue.clear();
    }

    /// Cancel the current operation and every remaining queued operation.
    ///
    /// This should be used when the underlying `SourceBuffer` has failed one
    /// operation, making the rest of the planned queue unreliable.
    ///
    /// Contrary to `on_operation_end`, this intentionally does not trigger any
    /// success-oriented side effects such as reflushes.
    pub(super) fn cancel_current_operations(&mut self) -> CancelledOperations {
        let current = self.queue.pop_front();
        let cancelled_media_ids = self
            .queue
            .iter()
            .filter_map(|operation| match operation {
                SourceBufferQueueElement::PushMedia { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        self.clear_queue();
        CancelledOperations {
            current,
            cancelled_media_ids,
        }
    }

    /// Update the result of segment selection. Appends do not change this result.
    pub(super) fn set_loading_complete(&mut self, complete: bool) {
        self.loading_complete = complete;
    }

    pub(super) fn is_loading_complete(&self) -> bool {
        self.loading_complete
    }

    /// To call once a `SourceBuffer` operation, either created through `append_buffer`,
    /// `remove_buffer` or `flush_buffer` has been finished by the underlying MSE SourceBuffer.
    pub(super) fn on_operation_end(&mut self) -> Option<SourceBufferQueueElement> {
        let queue_elt = self.queue.pop_front();
        match queue_elt {
            Some(SourceBufferQueueElement::Emptying) => {
                self.needs_reflush = true;
                jsFlush();
            }
            Some(SourceBufferQueueElement::PushMedia { .. }) if self.needs_reflush => {
                self.needs_reflush = false;
                jsFlush();
            }
            _ => {}
        }
        queue_elt
    }
}

/// Structure describing a media segment that should be pushed to the SourceBuffer.
pub(crate) struct PreparedPushData {
    /// Identifier used to identify the pushed segment in question.
    ///
    /// It can be useful for example to easily detect which segment has succesfully been pushed or
    /// caused an issue.
    pub(super) id: u64,

    /// Raw data of the segment to push.
    pub(super) segment_data: JsMemoryBlob,

    /// Time information for that segment as sourced from the media playlist.
    pub(super) time_info: SegmentTimeInfo,

    /// Media sequence number associated with this segment. Used to detect contiguous/non-contiguous
    /// segments.
    pub(super) sequence_number: u32,

    /// Discontinuity sequence (from the HLS playlist) associated with this segment.
    /// Used to detect contiguous/non-contiguous segments or state reset from previous segment.
    pub(super) discontinuity_sequence: u32,

    /// Optional precise timing anchor to align this append against an already buffered segment.
    pub(super) base_dts_hint: Option<TimescaledTimestamp>,

    /// Identity of the media sequence this append belongs to.
    pub(super) media_sequence_identity: MediaSequenceIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MediaSequenceIdentity {
    media_playlist_id: MediaPlaylistPermanentId,
    discontinuity_sequence: u32,
    init_segment_id: Option<u64>,
}

impl MediaSequenceIdentity {
    pub(crate) fn new(
        media_playlist_id: MediaPlaylistPermanentId,
        discontinuity_sequence: u32,
        init_segment_id: Option<f64>,
    ) -> Self {
        Self {
            media_playlist_id,
            discontinuity_sequence,
            init_segment_id: init_segment_id.map(f64::to_bits),
        }
    }
}

fn should_reset_transmuxer(
    previous_identity: Option<MediaSequenceIdentity>,
    next_identity: MediaSequenceIdentity,
    forced_reset: bool,
) -> bool {
    forced_reset || previous_identity != Some(next_identity)
}

/// Represents a successful response from the `append_buffer` SourceBuffer's method.
pub(crate) struct AppendBufferResponse {
    /// Time information optionally parsed from the segment itself.
    parsed: Option<ParsedSegmentInfo>,
}

impl AppendBufferResponse {
    pub(crate) fn precise_start(&self) -> Option<TimescaledTimestamp> {
        self.parsed.as_ref().and_then(|p| {
            p.start()
                .map(|x| TimescaledTimestamp::new(x, p.timescale()))
        })
    }

    pub(crate) fn precise_end(&self) -> Option<TimescaledTimestamp> {
        self.parsed
            .as_ref()
            .and_then(|p| p.end().map(|x| TimescaledTimestamp::new(x, p.timescale())))
    }

    /// Returns the optionally parsed start time, in seconds, found when parsing the segment's
    /// internals.
    ///
    /// If set it is generally closer to the real segment's start time, once pushed to the browser,
    /// than what the Media Playlist told us.
    pub(crate) fn media_start(&self) -> Option<f64> {
        self.parsed
            .as_ref()
            .and_then(|p| p.start().map(|x| x as f64 / p.timescale() as f64))
    }

    /// Returns the optionally parsed end, in seconds, found when parsing the segment's
    /// internals.
    ///
    /// If set it is generally closer to the real segment's end, once pushed to the browser,
    /// than what the Media Playlist told us.
    pub(crate) fn media_end(&self) -> Option<f64> {
        self.parsed
            .as_ref()
            .and_then(|p| p.end().map(|x| x as f64 / p.timescale() as f64))
    }
}

/// Enum listing possible operations awaiting to be performed on a `SourceBuffer`.
pub(crate) enum SourceBufferQueueElement {
    /// A new initialization segment needs to be pushed.
    PushInit(ResourceId),

    /// A new chunk of media data needs to be pushed.
    /// The `u64` is the corresponding `id` of the given `PreparedPushData` when the
    /// `push_media_segment` method was called.
    PushMedia { data: PreparedPushData, id: u64 },

    /// Some already-buffered needs to be removed, `start` and `end` giving the
    /// time range of the data to remove, in seconds.
    Remove { start: f64, end: f64 },

    /// The buffer is being completely emptied.
    Emptying,
}

use thiserror::Error;

/// Formatted error when the creation of a MSE SourceBuffer fails.
///
/// This is almost a 1:1 to error codes returned by `AddSourceBufferErrorCode`.
///
/// Note that `thiserror` is not used here because this error will really be formatted at a later
/// time.
#[derive(Debug)]
pub(super) enum AddSourceBufferError {
    /// No MediaSource was found to attach that `SourceBuffer`.
    NoMediaSourceAttached { message: String },

    /// The current `MediaSource` instance is in a "closed" state.
    /// As such it is not possible to attach a `SourceBuffer` to it anymore.
    MediaSourceIsClosed,

    /// A `QuotaExceededError` was received while trying to add the `SourceBuffer`
    ///
    /// Such errors are often encountered when another SourceBuffer attached to the same
    /// MediaSource instance was already updated through a buffer operation.
    QuotaExceededError { message: String },

    /// The given mime-type is not supported
    TypeNotSupportedError { mime_type: String, message: String },

    /// The given mime-type was an empty string
    EmptyMimeType,

    /// An unknown error happened.
    UnknownError { message: String },
}

impl AddSourceBufferError {
    /// Translate `SegmentParsingErrorCode` and its optional accompanying message, as returned by the
    /// `jsAppendBuffer` JavaScript function, into the corresponding `AddSourceBufferError`.
    ///
    /// # Arguments
    ///
    /// * `err` - The error received from the `jsAppendBuffer` JavaScript function.
    ///
    /// * `mime_type` - Mime-type linked to this SourceBuffer.
    pub(super) fn from_js_add_source_buffer_error(
        err: (AddSourceBufferErrorCode, Option<String>),
        mime_type: &str,
    ) -> Self {
        match err.0 {
            AddSourceBufferErrorCode::NoMediaSourceAttached => {
                AddSourceBufferError::NoMediaSourceAttached {
                    message: err
                        .1
                        .unwrap_or_else(|| "MediaSource instance not found.".to_owned()),
                }
            }
            AddSourceBufferErrorCode::MediaSourceIsClosed => {
                AddSourceBufferError::MediaSourceIsClosed
            }
            AddSourceBufferErrorCode::QuotaExceededError => {
                AddSourceBufferError::QuotaExceededError {
                    message: err
                        .1
                        .unwrap_or_else(|| "Unknown QuotaExceededError error".to_owned()),
                }
            }
            AddSourceBufferErrorCode::TypeNotSupportedError => {
                AddSourceBufferError::TypeNotSupportedError {
                    mime_type: mime_type.to_string(),
                    message: err
                        .1
                        .unwrap_or_else(|| "Unknown NotSupportedError error".to_owned()),
                }
            }
            AddSourceBufferErrorCode::EmptyMimeType => AddSourceBufferError::EmptyMimeType,
            AddSourceBufferErrorCode::UnknownError => AddSourceBufferError::UnknownError {
                message: err.1.unwrap_or_else(|| "Unknown error.".to_owned()),
            },
        }
    }
}

/// Error encountered synchronously after trying to push a segment to a `SourceBuffer`.
#[derive(Error, Debug)]
pub(crate) enum PushSegmentError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
    #[error("The {0} resource appended did not exist.")]
    NoResource(MediaType),
    #[error("Could not transmux {0} resource: {1}.")]
    TransmuxerError(MediaType, String),
    #[error("Uncategorized Error with {0} buffer: {1}")]
    UnknownError(MediaType, String),
}

impl PushSegmentError {
    /// Returns the `MediaType` associated to the `PushSegmentError`.
    pub(crate) fn media_type(&self) -> MediaType {
        match self {
            PushSegmentError::NoResource(m) => *m,
            PushSegmentError::NoSourceBuffer(m) => *m,
            PushSegmentError::TransmuxerError(m, _) => *m,
            PushSegmentError::UnknownError(m, _) => *m,
        }
    }

    /// Creates a new `PushSegmentError` based on a given `MediaType` and the original error as
    /// returned by the `jsAppendBuffer` binding.
    ///
    /// # Arguments
    ///
    /// * `media_type` - The `MediaType` linked to the corresponding `SourceBuffer`.
    ///
    /// * `err` - The error received from the `jsAppendBuffer` JavaScript function.
    fn from_js_append_buffer_error(
        media_type: MediaType,
        err: (SegmentParsingErrorCode, Option<String>),
    ) -> Self {
        match err.0 {
            SegmentParsingErrorCode::NoSourceBuffer => PushSegmentError::NoSourceBuffer(media_type),
            SegmentParsingErrorCode::NoResource => PushSegmentError::NoResource(media_type),
            SegmentParsingErrorCode::TransmuxerError => PushSegmentError::TransmuxerError(
                media_type,
                err.1
                    .unwrap_or_else(|| "Unknown transmuxing error.".to_owned()),
            ),
            SegmentParsingErrorCode::UnknownError => PushSegmentError::UnknownError(
                media_type,
                err.1.unwrap_or_else(|| "Unknown error.".to_owned()),
            ),
        }
    }
}

/// Error encountered synchronously after trying to remove media data from a `SourceBuffer`.
#[derive(Error, Debug)]
pub(crate) enum RemoveDataError {
    #[error("No SourceBuffer created for {0}")]
    NoSourceBuffer(MediaType),
}

/// Context about the continuity context of a pushed segment with a previously-pushed one.
#[derive(Clone, Debug)]
pub(crate) struct SegmentHints {
    /// Exact DTS anchor to use when computing the next base decode time.
    base_decode_time_start: u64,
    /// Timescale associated to `base_decode_time_start`.
    base_decode_time_start_timescale: u32,
    /// Whether transmuxer state should be discarded before processing this segment.
    ///
    /// This is intended for hard discontinuities such as rendition switches,
    /// codec/config changes, explicit discontinuity tags, or parser invalidation.
    ///
    /// This does NOT indicate whether the segment is timestamp-contiguous with
    /// the previous one. Timestamp continuity is determined solely through
    /// `base_decode_time_start`.
    reset_transmuxer_state: bool,
}

impl SegmentHints {
    /// Create a new `SegmentHints`
    pub(crate) fn new(
        base_decode_time_start: u64,
        base_decode_time_start_timescale: u32,
        reset_transmuxer_state: bool,
    ) -> Self {
        Self {
            base_decode_time_start,
            base_decode_time_start_timescale,
            reset_transmuxer_state,
        }
    }

    pub(crate) fn base_decode_time_start(&self) -> u64 {
        self.base_decode_time_start
    }

    pub(crate) fn base_decode_time_start_timescale(&self) -> u32 {
        self.base_decode_time_start_timescale
    }

    pub(crate) fn reset_transmuxer_state(&self) -> bool {
        self.reset_transmuxer_state
    }
}

fn build_segment_hints(
    segment_time_info: &SegmentTimeInfo,
    prev_segment_end: Option<TimescaledTimestamp>,
    base_dts_hint: Option<TimescaledTimestamp>,
    reset_transmuxer: bool,
) -> SegmentHints {
    const TIMESCALE: u32 = 90_000;

    let playlist_start = (segment_time_info.start() * TIMESCALE as f64).round() as u64;

    let (start_dts, start_dts_timescale) = if reset_transmuxer {
        if let Some(base_dts_hint) = base_dts_hint {
            log_debug!("Buffer: using base dts hint");
            (base_dts_hint.value(), base_dts_hint.timescale())
        } else {
            log_debug!("Buffer: determine dts hint from playlist time");
            (playlist_start, TIMESCALE)
        }
    } else {
        // just assume continuity
        match prev_segment_end {
            Some(last_end) => {
                log_debug!("Buffer: determine dts hint as last segment end dts");
                (last_end.value(), last_end.timescale())
            }
            None => {
                if let Some(base_dts_hint) = base_dts_hint {
                    log_debug!("Buffer: using base dts hint");
                    (base_dts_hint.value(), base_dts_hint.timescale())
                } else {
                    log_debug!("Buffer: determine dts hint from playlist time");
                    (playlist_start, TIMESCALE)
                }
            }
        }
    };
    SegmentHints::new(start_dts, start_dts_timescale, reset_transmuxer)
}

#[cfg(test)]
mod tests {
    use super::{should_reset_transmuxer, MediaSequenceIdentity};
    use crate::parser::MediaPlaylistPermanentId;

    #[test]
    fn transmuxer_reset_is_forced_when_requested() {
        let identity =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(3.0));
        assert!(should_reset_transmuxer(Some(identity), identity, true));
    }

    #[test]
    fn transmuxer_reset_is_not_needed_for_same_sequence_identity() {
        let identity =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(3.0));
        assert!(!should_reset_transmuxer(Some(identity), identity, false));
    }

    #[test]
    fn transmuxer_reset_happens_when_media_playlist_changes() {
        let previous =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(3.0));
        let next =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(2), 2, Some(3.0));
        assert!(should_reset_transmuxer(Some(previous), next, false));
    }

    #[test]
    fn transmuxer_reset_happens_when_discontinuity_changes() {
        let previous =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(3.0));
        let next =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 3, Some(3.0));
        assert!(should_reset_transmuxer(Some(previous), next, false));
    }

    #[test]
    fn transmuxer_reset_happens_when_init_segment_changes() {
        let previous =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(3.0));
        let next =
            MediaSequenceIdentity::new(MediaPlaylistPermanentId::new_for_test(1), 2, Some(4.0));
        assert!(should_reset_transmuxer(Some(previous), next, false));
    }
}
