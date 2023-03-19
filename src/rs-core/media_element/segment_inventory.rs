use crate::{bindings::MediaType, dispatcher::BufferedRange, Logger};

/// Structure allowing to identify the quality represented by a given segment.
///
/// This metadata then allows to perform advanced optimizations such as avoiding the loading of
/// media data where segments of higher quality already exists.
#[derive(Clone, Debug)]
pub(crate) struct SegmentQualityContext {
    /// Indicator of the desirability of the current quality a higher number
    /// meaning a higher quality.
    ///
    /// If set to `None`, the segment will never be replaced by segments with a score just because
    /// they have a score.
    score: Option<f64>,

    /// Identifier for the corresponding quality. Allows to avoid re-downloading
    /// the same quality even if the score changes (e.g. a variant change with
    /// no change of track could lead to re-loading).
    ///
    /// If set to `None` only the `score` will be used.
    quality_id: Option<u32>,
}

impl SegmentQualityContext {
    pub(crate) fn new(score: Option<f64>, quality_id: Option<u32>) -> Self {
        Self { score, quality_id }
    }
}

/// Information on a single chunk, present in the buffer at least partially.
pub(crate) struct BufferedChunk {
    /// Identifier for the segment represented by this `BufferedChunk` object.
    /// Note that a same segment can be splitted into multiple chunks, in which
    /// case they will have the same `id`
    id: u64,

    /// Supposed start, in seconds, the chunk is expected to start at.
    start: f64,

    /// Supposed end, in seconds, the chunk is expected to end at
    end: f64,

    /// Offset to addition to `start` to obtain its initial end in the buffer
    correction_start: f64,

    /// Offset to addition to `end` to obtain its initial end in the buffer
    correction_end: f64,

    last_buffered_start: f64,
    last_buffered_end: f64,

    validated: bool,

    /// Indicator of the desirability of the current quality a higher number
    /// meaning a higher quality.
    score: Option<f64>,

    /// Identifier for the corresponding quality
    quality_id: Option<u32>,
}

impl BufferedChunk {
    fn new(metadata: BufferedSegmentMetadata) -> Self {
        BufferedChunk {
            end: metadata.end,
            id: metadata.id,
            score: metadata.context.score,
            start: metadata.start,
            quality_id: metadata.context.quality_id,

            last_buffered_end: metadata.end,
            last_buffered_start: metadata.start,
            correction_end: 0.,
            correction_start: 0.,
            validated: false,
        }
    }

    /// Returns its corrected initial start time, in seconds, based on what it was when first
    /// pushed to the buffer.
    fn accurate_start(&self) -> f64 {
        self.start + self.correction_start
    }

    /// Returns its corrected initial end time, in seconds based on what it was when first pushed
    /// to the buffer.
    fn accurate_end(&self) -> f64 {
        self.end + self.correction_end
    }
}

const FLOAT_TOLERANCE: f64 = 0.01;

/// Keep track of every media segments downloaded and currently in the linked media
/// buffer.
///
/// The main point of this struct is to know which segments are already pushed to
/// the corresponding media buffer, of which variant, and which have been
/// garbage-collected since by the browser (and thus may need to be re-loaded).
pub struct SegmentInventory {
    /// All the media segments which should be currently in the browser's
    /// memory, in chronological order
    /// This `Vec` contains objects, each being related to a single downloaded
    /// chunk or segment which is at least partially added in the media buffer.
    inventory: Vec<BufferedChunk>,

    /// The concerned media type. This is just used for logs.
    media_type: MediaType,
}

impl SegmentInventory {
    /// Create a new `SegmentInventory` linked to a `SourceBuffer` of the corresponding
    /// `MediaType`.
    ///
    /// When created, the `SegmentInventory` will initially be empty. Segments should be inserted
    /// to it through the `insert_segment` method as soon as it is being pushed to the
    /// `SourceBuffer`, `validate_segment` should then be called after that push operation
    /// succeeds, to ensure that the `SegmentInventory` knows where that segment is initially
    /// buffered.
    pub fn new(media_type: MediaType) -> Self {
        Self {
            inventory: vec![],
            media_type,
        }
    }

    /// Reset the whole inventory.
    pub fn reset(&mut self) {
        self.inventory = vec![];
    }

    /// Once a segment has been succesfully pushed, you should call `validate_segment` with the
    /// current `TimeRanges` object from the corresponding MSE `SourceBuffer` and the
    /// `id` of the segment that has been pushed as arguments, to allow the `SegmentInventory` to
    /// check the initial place of that segment in the buffer.
    ///
    /// We talk here about a "correction" because the expected start and end of the segment can be
    /// a little different from what the lower-level buffer advertise, this is what we're computing
    /// here.
    pub fn validate_segment(&mut self, seg_id: u64, buffered: &BufferedRange) {
        self.synchronize(buffered);
        let seg_idx = self
            .inventory
            .iter()
            .position(|seg| !seg.validated && seg.id == seg_id);
        if seg_idx.is_none() {
            return;
        }

        let seg_idx = seg_idx.unwrap();
        let seg = self.inventory.get(seg_idx).unwrap();

        let range = buffered.into_iter().fold(None, |acc: Option<(f64, f64)>, range| {
            let curr_overlap_size = overlap_size(seg, range);
            if curr_overlap_size > 0. {
                if let Some(previous) = acc {
                    if overlap_size(seg, previous) > curr_overlap_size {
                        acc
                    } else {
                        Some(range)
                    }
                } else {
                    Some(range)
                }
            } else {
                acc
            }
        });

        if range.is_none() {
            Logger::warn(
                &format!("SI: Buffered range of pushed segment not found (s:{}, e:{})",
                    seg.start, seg.end)
                );
            return;
        }
        let (range_start, range_end) = range.unwrap();
        let prev_seg = if seg_idx == 0 {
            None
        } else {
            self.inventory.get(seg_idx - 1)
        };
        let next_seg = self.inventory.get(seg_idx + 1);
        let correction_start = match prev_seg {
            None => guess_correction_start_from_range_start(seg, range_start, self.media_type),
            Some(prev) => {
                let prev_buff_end = prev.last_buffered_end;

                if prev_buff_end + 0.01 >= seg.start {
                    // Those two segments seems contiguous
                    if range_start >= prev_buff_end {
                        // The current range starts after the previous segment
                        // Should not happen but still handle it
                        Logger::warn(
                            &format!("SI: Current {} range starts after previous contiguous segment (rs: {}, pe:{}",
                                self.media_type, range_start, prev_buff_end)
                        );
                        range_start - seg.start
                    } else if f64::abs(prev_buff_end - seg.start) < 0.4 {
                        if seg.start < prev_buff_end {
                            // The current segment might have overlapped the previous one.
                            0.
                        } else {
                            // Let's say this segment starts where the previous one ends
                            prev_buff_end - seg.start
                        }
                    } else {
                        // Both segments should not have been contiguous yet they are
                        Logger::warn(&format!(
                            "SI: {} segments are unexpectedly contiguous (ps:{}, pe:{}, s:{}, e:{})",
                            self.media_type, prev.last_buffered_start, prev_buff_end, seg.start, seg.end
                        ));
                        0.
                    }
                } else if range_start < prev_buff_end {
                    // Segments are not contiguous yet the range starts before the previous one.
                    // Weird but OK
                    Logger::warn(&format!(
                        "SI: {} range unexpectedly include previous segment (rs: {}, ps: {}, pe:{}, s:{})",
                        self.media_type, range_start, prev.last_buffered_start, prev_buff_end, seg.start
                    ));
                    0.
                } else {
                    // Segments are not contiguous and the range is after the previous segment:
                    // The current segment is the first one of the range.
                    guess_correction_start_from_range_start(seg, range_start, self.media_type)
                }
            }
        };

        let correction_end = match next_seg {
            None => guess_correction_end_from_range_end(seg, range_end, self.media_type),
            Some(next) => {
                let next_buf_start = next.last_buffered_start;

                if seg.end + 0.01 >= next_buf_start {
                    // Those two segments seems contiguous
                    if range_end < next_buf_start {
                        // The current range ends before the next segment
                        // Should not happen but still handle it
                        Logger::warn(
                            &format!("SI: Current {} range ends before next contiguous segment (re: {}, ns:{}",
                                self.media_type, range_end, next_buf_start)
                        );
                        range_end - seg.end
                    } else if f64::abs(next_buf_start - seg.end) < 0.4 {
                        if seg.end > next_buf_start {
                            // The current segment might have overlapped the next one.
                            0.
                        } else {
                            // Let's say this segment ends where the next one starts
                            next_buf_start - seg.end
                        }
                    } else {
                        // Both segments should not have been contiguous yet they are
                        Logger::warn(&format!(
                            "SI: {} segments are unexpectedly contiguous (s:{}, e:{}, ns:{}, ne:{})",
                            self.media_type, seg.start, seg.end, next_buf_start, next.last_buffered_end
                        ));
                        0.
                    }
                } else if range_end > next_buf_start {
                    // Segments are not contiguous yet the range ends after the next one.
                    // Weird but OK
                    Logger::warn(&format!(
                        "SI: {} range unexpectedly include next segment (re: {}, ps:{}, s:{})",
                        self.media_type, range_end, next_buf_start, seg.start
                    ));
                    0.
                } else {
                    // Segments are not contiguous and the range ends before the next segment:
                    // The current segment is the last one of the range.
                    guess_correction_end_from_range_end(seg, range_end, self.media_type)
                }
            }
        };

        let mut seg = self.inventory.get_mut(seg_idx).unwrap();
        seg.correction_start = correction_start;
        seg.correction_end = correction_end;
        seg.last_buffered_start = seg.accurate_start();
        seg.last_buffered_end = seg.accurate_end();
        seg.validated = true;

        if f64::abs(seg.correction_start) >= 0.05 || f64::abs(seg.correction_end) >= 0.05 {
            Logger::debug(&format!(
                "SI: corrected {} segment (s:{}, e:{}, cs:{}, ce:{})",
                self.media_type, seg.start, seg.end, correction_start, correction_end
            ));
        }
    }

    /// Push a new segment to the `SegmentInventory`.
    ///
    /// This should be done any time a segment just began to be pushed to the `SourceBuffer`. It
    /// allows the `SegmentInventory` to construct its representation of buffered segments.
    pub(super) fn insert_segment(&mut self, metadata: BufferedSegmentMetadata) {
        let start = metadata.start;
        let end = metadata.end;
        if start >= end {
            Logger::warn(&format!(
                "SI: Invalid {} chunked inserted: start ({}) inferior or equal to ({})",
                self.media_type, start, end
            ));
            return;
        }

        // Operations which will need to be performed at the end on the inventory to include the
        // segment.
        //
        // Those operations __SHOULD__ be performed from last-to-first as it may remove elements
        // (and as such, move indexes around).
        let mut updates: Vec<PendingBufferedChunkModificationTask> = vec![];
        let mut insertion_task: Option<PendingBufferChunkInsertionTask> = None;

        // Searching in antechronological order, look for first segment with start inferior or
        // equal to the one we want to push.
        let compared_pos = self
            .inventory
            .iter()
            .rev()
            .position(|s| s.start <= start)
            .map(|pos| self.inventory.len() - 1 - pos);

        if let Some(base_idx) = compared_pos {
            let base_seg = self.inventory.get(base_idx).unwrap();
            if base_seg.end <= start {
                // `base_seg` starts before and end before

                // our segment is after, push it after this one
                //
                // Case 1:
                //   base_seg      : |------|
                //   new_segment   :        |======|
                //   ===>          : |------|======|
                //
                // Case 2:
                //   base_seg      : |------|
                //   new_segment   :          |======|
                //   ===>          : |------| |======|
                Logger::debug(&format!(
                    "SI: Pushing {} segment strictly after previous one (s:{}, e:{}, pe: {}).",
                    self.media_type, start, end, base_seg.end
                ));
                insertion_task = Some(PendingBufferChunkInsertionTask::Insert(base_idx + 1));
                updates.extend(check_next_overlapping_segments(
                    start,
                    end,
                    base_idx + 1,
                    self.inventory.as_slice(),
                    self.media_type,
                ));
            } else if f64::abs(base_seg.start - start) < 0.01 {
                // `base_seg` starts at the same time and end before
                if base_seg.end <= end {
                    // In those cases, replace
                    //
                    // Case 1:
                    //  base_seg      : |-------|
                    //  new_segment   : |=======|
                    //  ===>          : |=======|
                    //
                    // Case 2:
                    //  base_seg      : |-------|
                    //  new_segment   : |==========|
                    //  ===>          : |==========|
                    Logger::debug(&format!(
                        "SI: {} segment pushed replace another one (s:{}, e:{}, pe:{})",
                        self.media_type, start, end, base_seg.end
                    ));
                    insertion_task = Some(PendingBufferChunkInsertionTask::Replace(base_idx));
                    updates.extend(check_next_overlapping_segments(
                        start,
                        end,
                        base_idx + 1,
                        self.inventory.as_slice(),
                        self.media_type,
                    ));
                } else {
                    // The previous segment starts at the same time and finishes
                    // after the new segment.
                    // Update the start of the previous segment and put the new
                    // segment before.
                    //
                    // Case 1:
                    //  base_seg      : |------------|
                    //  new_segment   : |==========|
                    //  ===>          : |==========|-|
                    Logger::debug(
                        &format!("SI: {} segment pushed ends before another with the same start (s:{}, e:{}, pe:{})",
                        self.media_type, start, end, base_seg.end));
                    insertion_task = Some(PendingBufferChunkInsertionTask::Insert(base_idx));
                    updates.push(PendingBufferedChunkModificationTask::UpdateStart {
                        index: base_idx,
                        start: end,
                    });
                }
            } else if base_seg.end <= end {
                // our segment has a "complex" relation with this one,
                // update the old one end and add this one after it.
                //
                // Case 1:
                //  base_seg      : |-------|
                //  new_segment   :    |======|
                //  ===>          : |--|======|
                //
                // Case 2:
                //  base_seg      : |-------|
                //  new_segment   :    |====|
                //  ===>          : |--|====|
                Logger::debug(&format!(
                    "SI: {} segment pushed updates end of previous one (s:{}, e:{}, ps: {}, pe:{}",
                    self.media_type, start, end, base_seg.start, base_seg.end
                ));

                updates.push(PendingBufferedChunkModificationTask::UpdateEnd {
                    index: base_idx,
                    end: start,
                });
                insertion_task = Some(PendingBufferChunkInsertionTask::Insert(base_idx + 1));
                updates.extend(check_next_overlapping_segments(
                    start,
                    end,
                    base_idx + 1,
                    self.inventory.as_slice(),
                    self.media_type,
                ));
            } else {
                // The previous segment completely recovers the new segment.
                // Split the previous segment into two segments, before and after
                // the new segment.
                //
                // Case 1:
                //  base_seg      : |---------|
                //  new_segment   :    |====|
                //  ===>          : |--|====|-|
                Logger::warn(
                    &format!("SI: {} segment pushed is contained in a previous one  (s:{}, e:{}, ns:{}, ne: {})",
                    self.media_type, start, end, base_seg.start, base_seg.end));
                // Note: this sadly means we're doing as if
                // that chunk is present two times.
                // Thankfully, this scenario should be
                // fairly rare.
                insertion_task = Some(PendingBufferChunkInsertionTask::InsertInside {
                    index: base_idx,
                    split_1_end: start,
                    split_2_start: end,
                });
            }
        }

        if insertion_task.is_none() {
            // if we got here, we are at the first segment
            // check bounds of the previous first segment
            if let Some(first_seg) = self.inventory.first() {
                if first_seg.start >= end {
                    // our segment is before, put it before
                    //
                    // Case 1:
                    //  first_seg     :      |----|
                    //  new_segment   : |====|
                    //  ===>          : |====|----|
                    //
                    // Case 2:
                    //  first_seg     :        |----|
                    //  new_segment   : |====|
                    //  ===>          : |====| |----|
                    Logger::debug(&format!(
                        "SI: {} segment pushed comes before all previous ones (s:{}, e:{}, fs:{})",
                        self.media_type, start, end, first_seg.start
                    ));
                    insertion_task = Some(PendingBufferChunkInsertionTask::Insert(0));
                } else if first_seg.end <= end {
                    // Our segment is bigger, replace the first
                    //
                    // Case 1:
                    //  first_seg     :   |---|
                    //  new_segment   : |=======|
                    //  ===>          : |=======|
                    //
                    // Case 2:
                    //  first_seg     :   |-----|
                    //  new_segment   : |=======|
                    //  ===>          : |=======|
                    Logger::debug(
                        &format!("SI: {} Segment pushed starts before and completely recovers the previous first one (s:{}, e:{}, fs:{}, fe:{})",
                        self.media_type, start, end , first_seg.start, first_seg.end));
                    insertion_task = Some(PendingBufferChunkInsertionTask::Replace(0));
                    updates.extend(check_next_overlapping_segments(
                        start,
                        end,
                        1,
                        self.inventory.as_slice(),
                        self.media_type,
                    ));
                } else {
                    // our segment has a "complex" relation with the first one,
                    // update the old one start and add this one before it.
                    //
                    // Case 1:
                    //  first_seg     :    |------|
                    //  new_segment   : |======|
                    //  ===>          : |======|--|
                    Logger::debug(&format!(
                        "SI: {} segment pushed start of the next one (s:{}, e:{}, ns:{}, ne: {})",
                        self.media_type, start, end, first_seg.start, first_seg.end
                    ));
                    updates.push(PendingBufferedChunkModificationTask::UpdateStart {
                        index: 0,
                        start: end,
                    });
                    insertion_task = Some(PendingBufferChunkInsertionTask::Insert(0));
                }
            } else {
                Logger::debug(&format!(
                    "SI: first {} segment pushed (s:{}, e:{})",
                    self.media_type, start, end
                ));
                insertion_task = Some(PendingBufferChunkInsertionTask::Insert(0));
            }
        }

        self.process_pending_updates(updates);

        match insertion_task {
            Some(PendingBufferChunkInsertionTask::Replace(index)) => {
                self.inventory[index] = BufferedChunk::new(metadata);
            }
            Some(PendingBufferChunkInsertionTask::Insert(index)) => {
                self.inventory.insert(index, BufferedChunk::new(metadata));
            }
            Some(PendingBufferChunkInsertionTask::InsertInside { index, .. }) => {
                if let Some(seg) = self.inventory.get_mut(index) {
                    let duplicated = BufferedSegmentMetadata {
                        id: seg.id,
                        start: end,
                        end: seg.end,
                        context: SegmentQualityContext {
                            score: seg.score,
                            quality_id: seg.quality_id,
                        },
                    };
                    let duplicated_after = BufferedChunk::new(duplicated);
                    seg.end = start;
                    self.inventory
                        .insert(index + 1, BufferedChunk::new(metadata));
                    self.inventory.insert(index + 2, duplicated_after);
                } else {
                    Logger::error("SI: unfound index when inserting inside");
                }
            }
            None => {
                Logger::warn("SI: wanted segment not inserted");
            }
        }
    }

    /// Returns the whole inventory.
    ///
    /// To get a list synchronized with what a media buffer actually has buffered
    /// you might want to call `synchronize_buffered` before calling this method.
    pub(crate) fn inventory(&self) -> &[BufferedChunk] {
        self.inventory.as_slice()
    }

    pub fn synchronize(&mut self, buffered: &BufferedRange) {
        let mut segment_idx = 0;
        if segment_idx >= self.inventory.len() {
            return;
        }
        let mut updates: Vec<PendingBufferedChunkModificationTask> = vec![];
        buffered.into_iter().enumerate().for_each(|(range_index, (range_start, range_end))| {
            let mut curr_seg = if let Some(seg) = self.inventory.get(segment_idx) {
                seg
            } else {
                return;
            };
            while !curr_seg.validated {
                segment_idx += 1;
                curr_seg = if let Some(seg) = self.inventory.get(segment_idx) {
                    seg
                } else {
                    return;
                };
            }

            if range_end <= curr_seg.last_buffered_start {
                // That range is before the current segment
                return;
            }

            // Remove all segments that are actually before that range
            while range_start >= curr_seg.last_buffered_end || !curr_seg.validated {
                if curr_seg.validated {
                    Logger::info(
                        &format!("SI: {} segment has been completely GCed (s:{}, e:{}, rs:{})",
                        self.media_type, curr_seg.start, curr_seg.end, range_start));
                    updates.push(PendingBufferedChunkModificationTask::Removal(segment_idx));
                }
                segment_idx += 1;
                curr_seg = if let Some(seg) = self.inventory.get(segment_idx) {
                    seg
                } else {
                    return;
                };
            }
            if range_end <= curr_seg.last_buffered_start {
                // That range is before the current segment
                return;
            }

            // Here at last, we're sure to be the first chronological validated segment in that
            // range
            if range_start > curr_seg.last_buffered_start {
                // The range starts after the expected first segment of that range, it has been
                // GCed at least partially

                if range_end < curr_seg.last_buffered_end {
                    // The range also ends inside buffer allocated for `curr_seg`
                    // Check that the next range is not more appriopriate to classify the
                    // current segment
                    let next_range_start = buffered.start(range_index + 1);
                    if let Some(next_range_start) = next_range_start {
                        if range_end - f64::max(curr_seg.start, range_start) < next_range_start - curr_seg.end {
                            // There is a high risk that one of the following range is more adapted
                            // here, go to next range
                            return;
                        }

                    }
                }

                // Check unnecessary normally but let's repeat it to better indicate intent
                if curr_seg.validated {
                    Logger::info(
                        &format!("SI: {} segment has been partially GCed at the start (pbs:{}, nbs:{})",
                        self.media_type, curr_seg.last_buffered_start, range_start)
                    );
                    updates.push(PendingBufferedChunkModificationTask::UpdateStart {
                        index: segment_idx,
                        start: range_start,
                    });
                }
            }

            while range_end >= curr_seg.last_buffered_end || !curr_seg.validated {
                segment_idx += 1;
                curr_seg = if let Some(seg) = self.inventory.get(segment_idx) {
                    seg
                } else {
                    return;
                };
            }


            if range_end > curr_seg.last_buffered_start  {
                // The range finishes inside the current segment

                // Check (again) that the next range is not more appriopriate to classify the
                // current segment
                let next_range_start = buffered.start(range_index + 1);
                if let Some(next_range_start) = next_range_start {
                    if range_end - f64::max(curr_seg.start, range_start) < next_range_start - curr_seg.end {
                        return;
                    }
                }

                // Check unnecessary normally but let's repeat it to better indicate intent
                if curr_seg.validated {
                    Logger::info(
                        &format!("SI: {} segment has been partially GCed at the end (pbe:{}, nbe:{})",
                        self.media_type, curr_seg.last_buffered_end, range_end)
                    );
                    updates.push(PendingBufferedChunkModificationTask::UpdateEnd {
                        index: segment_idx,
                        end: range_end,
                    });
                }
                segment_idx += 1;
            }
        });

        if segment_idx < self.inventory.len() {
            Logger::info(
                &format!("SI: Multiple {} segments have been completely removed at the end (pl:{}, nl:{})",
                self.media_type, self.inventory.len(), segment_idx)
            );
            self.inventory.truncate(segment_idx);
        }
        self.process_pending_updates(updates);
    }

    fn process_pending_updates(&mut self, mut updates: Vec<PendingBufferedChunkModificationTask>) {
        while let Some(update) = updates.pop() {
            match update {
                PendingBufferedChunkModificationTask::Removal(idx) => {
                    self.inventory.remove(idx);
                }
                PendingBufferedChunkModificationTask::UpdateStart { index, start } => {
                    if let Some(seg) = self.inventory.get_mut(index) {
                        seg.last_buffered_start = start;
                        if seg.last_buffered_end - seg.last_buffered_start < 0.01 {
                            self.inventory.remove(index);
                        }
                    } else {
                        Logger::error("SI: unfound index when updating start");
                    }
                }
                PendingBufferedChunkModificationTask::UpdateEnd { index, end } => {
                    if let Some(seg) = self.inventory.get_mut(index) {
                        seg.last_buffered_end = end;
                        if seg.last_buffered_end - seg.last_buffered_start < 0.01 {
                            self.inventory.remove(index);
                        }
                    } else {
                        Logger::error("SI: unfound index when updating end");
                    }
                }
            }
        }
    }
}

/// Returns `true` if the buffered start of the given chunk looks coherent enough
/// relatively to what was announced for it
fn correction_start_looks_coherent(seg: &BufferedChunk, correction_start: f64) -> bool {
    let start = seg.start;
    let end = seg.end;

    let corrected_start = start + correction_start;
    if f64::abs(start - correction_start) > 0.4 ||
        (seg.correction_end != 0. && seg.correction_end <= correction_start - 0.3)
    {
        false
    } else {
        let true_duration = corrected_start - (seg.start + corrected_start);
        let expected_duration = end - start;
        let diff = f64::abs(true_duration - expected_duration);
        diff <= f64::min(0.3, expected_duration / 3.)
    }
}

/// Returns `true` if the buffered end of the given chunk looks coherent enough
/// relatively to what was announced for it
fn correction_end_looks_coherent(seg: &BufferedChunk, correction_end: f64) -> bool {
    let start = seg.start;
    let end = seg.end;

    let corrected_end = end + correction_end;
    if f64::abs(end - corrected_end) > 0.4 ||
        (seg.correction_start != 0. && seg.correction_start > corrected_end - 0.2)
    {
        false
    } else {
        let true_duration = corrected_end - seg.accurate_start();
        let expected_duration = end - start;
        let diff = f64::abs(true_duration - expected_duration);
        diff <= f64::min(0.3, expected_duration / 3.)
    }
}

/// Evaluate the given buffered Chunk's buffered start from its range's start,
/// considering that this chunk is the first one in it.
fn guess_correction_start_from_range_start(
    first_seg_in_range: &BufferedChunk,
    range_start: f64,
    media_type: MediaType,
) -> f64 {
    if first_seg_in_range.start - range_start <= 0.4 {
        let correction_start = range_start - first_seg_in_range.start;
        if correction_start_looks_coherent(first_seg_in_range, correction_start) {
            correction_start
        } else {
            0.
        }
    } else if range_start < first_seg_in_range.start {
        Logger::debug(&format!(
            "SI: {} range start too far from expected start (r:{}, e:{})",
            media_type, range_start, first_seg_in_range.start
        ));
        0.
    } else {
        Logger::debug(&format!(
            "SI: {} segment appears immediately garbage collected at the start (r:{}, e:{})",
            media_type, range_start, first_seg_in_range.start
        ));
        range_start
    }
}

/// Evaluate the given buffered Chunk's buffered end from its range's end,
/// considering that this chunk is the last one in it.
fn guess_correction_end_from_range_end(
    last_segment_in_range: &BufferedChunk,
    range_end: f64,
    media_type: MediaType,
) -> f64 {
    if range_end - last_segment_in_range.end <= 0.4 {
        let correction_end = range_end - last_segment_in_range.end;
        if correction_end_looks_coherent(last_segment_in_range, correction_end) {
            correction_end
        } else {
            Logger::debug(&format!(
                "SI: Unreliable {} buffered end correction (r:{}, e:{}, c:{})",
                media_type, range_end, last_segment_in_range.end, correction_end
            ));
            0.
        }
    } else if range_end > last_segment_in_range.end {
        Logger::debug(&format!(
            "SI: {} range end too far from expected end (r:{}, e:{})",
            media_type, range_end, last_segment_in_range.end
        ));
        0.
    } else {
        Logger::debug(&format!(
            "SI: {} segment appears immediately garbage collected at the end (r:{}, e:{})",
            media_type, range_end, last_segment_in_range.end
        ));
        range_end - last_segment_in_range.end
    }
}

enum PendingBufferedChunkModificationTask {
    UpdateStart { index: usize, start: f64 },
    UpdateEnd { index: usize, end: f64 },
    Removal(usize),
}

enum PendingBufferChunkInsertionTask {
    Insert(usize),
    Replace(usize),
    InsertInside {
        index: usize,
        split_1_end: f64,
        split_2_start: f64,
    },
}

fn check_next_overlapping_segments(
    start: f64,
    end: f64,
    base_idx: usize,
    inventory: &[BufferedChunk],
    media_type: MediaType,
) -> Vec<PendingBufferedChunkModificationTask> {
    let mut updates = vec![];
    let mut new_idx = base_idx;
    while new_idx < inventory.len() {
        let next_seg = inventory.get(new_idx);
        if next_seg.is_none() {
            break;
        }
        let next_seg = next_seg.unwrap();
        if next_seg.start >= end {
            break;
        }
        if next_seg.end > end {
            // The next segment ends after new_segment.
            // Mutate the next segment.
            //
            // Case 1:
            //   base_seg      : |------|
            //   new_segment   :        |======|
            //   next_seg      :            |----|
            //   ===>          : |------|======|-|
            Logger::debug(&format!(
                "SI: {} segment pushed updates the start of the next one (e:{}, ns: {})",
                media_type, end, next_seg.start
            ));
            updates.push(PendingBufferedChunkModificationTask::UpdateStart {
                index: new_idx,
                start: next_seg.end,
            });
        } else {
            // The next segment was completely contained in new_segment.
            // Remove it.
            //
            // Case 1:
            //   base_seg      : |------|
            //   new_segment   :        |======|
            //   next_seg      :          |---|
            //   ===>          : |------|======|
            //
            // Case 2:
            //   base_seg      : |------|
            //   new_segment   :        |======|
            //   next_seg      :          |----|
            //   ===>          : |------|======|
            Logger::debug(&format!(
                "SI: {} segment pushed removes the next one (s:{}, e:{}, ns:{}, ne: {})",
                media_type, start, end, next_seg.start, next_seg.end
            ));
            updates.push(PendingBufferedChunkModificationTask::Removal(new_idx));
        }
        new_idx += 1;
    }
    updates
}

#[derive(Clone, Debug)]
pub(super) struct BufferedSegmentMetadata {
    /// Identifier for that segment, unique between all other segments currently
    /// buffered.
    pub(super) id: u64,

    /// Supposed start, in seconds, the segment is expected to start at.
    pub(super) start: f64,

    /// Supposed end, in seconds, the segment is expected to end at
    pub(super) end: f64,

    pub(super) context: SegmentQualityContext,
}

fn overlap_size(
    seg: &BufferedChunk,
    range: (f64, f64)
) -> f64 {
    let overlap_start = f64::max(seg.start, range.0);
    let overlap_end = f64::min(seg.end, range.1);
    overlap_end - overlap_start
}

// Note: to log
// let str = self.inventory.iter().map(|x| {
//     format!("{} - {}", x.last_buffered_start, x.last_buffered_end)
// }).collect::<Vec<String>>().join(" / ");
// Logger::error(&format!("SI: {}", str));
