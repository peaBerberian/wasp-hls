use super::MediaElementReference;
use crate::{
    bindings::{jsStartRebuffering, jsStopRebuffering},
    requester::Requester,
    utils::{logger::*, time_ranges::find_next_gap},
};

/// Distance before a buffer hole at which playback may stop and jump, in seconds.
const GAP_JUMP_DISTANCE: f64 = 0.5;

/// Rebuffering state and recovery from holes in the browser's buffered ranges.
pub(super) struct StallResolver {
    is_rebuffering: bool,
    min_buffer_time: f64,
}

impl StallResolver {
    pub(super) fn new() -> Self {
        Self {
            is_rebuffering: false,
            min_buffer_time: 5.,
        }
    }

    pub(super) fn is_rebuffering(&self) -> bool {
        self.is_rebuffering
    }

    /// Keep the resume threshold within the amount the loader is asked to buffer.
    pub(super) fn update_buffering_settings(
        &mut self,
        target_duration: Option<f64>,
        buffer_goal: f64,
    ) {
        // TODO: account for low-latency playlists.
        let normal_threshold = target_duration
            .map(|duration| (duration - 1.).clamp(2., 8.))
            .unwrap_or(5.);
        self.min_buffer_time = normal_threshold.min(buffer_goal.max(0.));
        log_debug!("Stall: Updating min_buffer_time: {}", self.min_buffer_time);
    }

    pub(super) fn set_rebuffering(&mut self, value: bool) {
        if self.is_rebuffering != value {
            self.is_rebuffering = value;
            log_info!(
                "Stall: Rebuffering {}",
                if value { "started" } else { "ended" }
            );
            if value {
                jsStartRebuffering();
            } else {
                jsStopRebuffering();
            }
        }
    }
}

// TODO: Return playback actions (hold, resume or seek) and let the media element
// apply them.
pub(super) fn resolve(
    media: &mut MediaElementReference,
    requester: &Requester,
    loading_ready: bool,
) {
    let Some(observation) = media.last_observation.as_ref() else {
        media.stall_resolver.set_rebuffering(true);
        return;
    };
    if media.is_seek_pending() {
        // Wait for an observation which accounts for the requested position.
        return;
    }

    let position = media.wanted_position();
    let common = media.common_buffered_ranges();
    // SourceBuffer ranges may be newer than the media element snapshot.
    // Use the same ranges for rebuffering and hole detection when available.
    // TODO: Consider requiring the media element snapshot to also show little
    // playable data before starting recovery. Newer worker ranges may reflect
    // eviction behind the actual playhead and cause a pause or backward seek.
    // Still allow newer ranges to justify resuming playback sooner.
    let buffer_ahead = match common.as_ref() {
        Some(ranges) => ranges
            .iter()
            .find(|&&(start, end)| start <= position && position < end)
            .map_or(0., |&(_, end)| end - position),
        None => observation
            .buffered()
            .buffer_gap(observation.current_time())
            .unwrap_or(0.),
    };
    let buffer_hole = common
        .as_ref()
        .and_then(|ranges| find_next_gap(ranges, position));
    let can_skip_hole = loading_ready
        && buffer_hole.is_some_and(|(_, next_start)| {
            !requester.has_pending_segment_before(next_start)
                && !media.has_pending_append_before(next_start)
        });

    // readyState may stay at 2 while the last buffered frames are playing.
    let can_play_to_end = observation.ended()
        || (observation.ready_state() >= 2 && media.buffers_ended() && buffer_hole.is_none())
        || (buffer_ahead > 0.
            && buffer_ahead >= observation.duration() - observation.current_time() - 0.001);
    let should_rebuffer = if can_play_to_end {
        false
    } else if can_skip_hole {
        // Play toward the hole even below the normal resume threshold.
        buffer_ahead <= GAP_JUMP_DISTANCE
    } else if media.stall_resolver.is_rebuffering {
        buffer_ahead <= 0. || buffer_ahead < media.stall_resolver.min_buffer_time
    } else {
        buffer_ahead < GAP_JUMP_DISTANCE
    };

    media.stall_resolver.set_rebuffering(should_rebuffer);
    if let Some((end, next_start)) = buffer_hole {
        if can_skip_hole && should_rebuffer && end - position <= GAP_JUMP_DISTANCE {
            log_warn!(
                "Stall: Jumping over a browser buffer hole (p:{}, e:{}, n:{})",
                position,
                end,
                next_start
            );
            media.seek(next_start + 0.01);
        }
    }
}
