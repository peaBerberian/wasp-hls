#[cfg(not(test))]
use crate::bindings::{jsClearTimer, jsMonotonicNow, jsTimer};
use crate::{
    bindings::{MediaType, TimerId, TimerReason},
    parser::{MediaPlaylistPermanentId, PlaylistRefreshInfo},
};
#[cfg(test)]
use tests::{clear_timer as jsClearTimer, now as jsMonotonicNow, timer as jsTimer};

/// Keeps playlist reload times and schedules refreshes only for playlists we need.
pub(super) struct PlaylistRefreshManager {
    playlists: Vec<PlaylistRefresh>,
    /// Playlists needed for playback or replacement inspection.
    followed_audio: Option<MediaPlaylistPermanentId>,
    followed_video: Option<MediaPlaylistPermanentId>,
}

struct PlaylistRefresh {
    playlist_id: MediaPlaylistPermanentId,
    /// Earliest next reload time, in monotonic milliseconds. Remains when the timer is canceled.
    next_refresh_at: f64,
    refresh_info: PlaylistRefreshInfo,
    timer_id: Option<TimerId>,
}

impl PlaylistRefresh {
    fn cancel_timer(&mut self) {
        if let Some(timer_id) = self.timer_id.take() {
            jsClearTimer(timer_id);
        }
    }
}

impl PlaylistRefreshManager {
    pub(super) fn new() -> Self {
        Self {
            playlists: Vec::new(),
            followed_audio: None,
            followed_video: None,
        }
    }

    /// Follow this playlist and arm its reload timer, preserving any previous deadline.
    /// Return true if it can be requested now; the caller still checks whether it is
    /// finalized or already being requested.
    pub(super) fn follow(&mut self, media_type: MediaType, id: MediaPlaylistPermanentId) -> bool {
        match media_type {
            MediaType::Audio => self.followed_audio = Some(id),
            MediaType::Video => self.followed_video = Some(id),
        }
        self.cancel_unused_timers();
        !self.wait_for_refresh(&id)
    }

    fn followed_playlist(&self, media_type: MediaType) -> Option<MediaPlaylistPermanentId> {
        match media_type {
            MediaType::Audio => self.followed_audio,
            MediaType::Video => self.followed_video,
        }
    }

    pub(super) fn forget_media_type(&mut self, media_type: MediaType) {
        match media_type {
            MediaType::Audio => self.followed_audio = None,
            MediaType::Video => self.followed_video = None,
        }
        self.cancel_unused_timers();
    }

    pub(super) fn clear(&mut self) {
        self.followed_audio = None;
        self.followed_video = None;
        for entry in &mut self.playlists {
            entry.cancel_timer();
        }
        self.playlists.clear();
    }

    pub(super) fn is_following(&self, id: &MediaPlaylistPermanentId) -> bool {
        self.followed_audio == Some(*id) || self.followed_video == Some(*id)
    }

    /// Record a deadline relative to the latest request attempt, including download and
    /// parsing time. Inactive playlists retain their deadline without running a timer.
    pub(super) fn on_playlist_loaded(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        refresh_info: Option<PlaylistRefreshInfo>,
        started_at: f64,
    ) {
        let previous = self
            .playlists
            .iter()
            .position(|entry| entry.playlist_id == playlist_id);
        let Some(refresh_info) = refresh_info else {
            if let Some(index) = previous {
                self.playlists.remove(index).cancel_timer();
            }
            return;
        };
        let unchanged = previous
            .is_some_and(|index| self.playlists[index].refresh_info.tail == refresh_info.tail);
        // Use the last positive segment duration for normal polling.
        // This deliberately permits shorter intervals than RFC 8216's target-duration rule.
        // Also, poll unchanged media again after half the target duration.
        let interval = if unchanged {
            refresh_info.target_duration / 2.
        } else {
            refresh_info
                .last_segment_duration
                .unwrap_or(refresh_info.target_duration)
        } * 1000.;
        let next_refresh_at = started_at + interval;
        let followed = self.is_following(&playlist_id);
        let entry = if let Some(index) = previous {
            let entry = &mut self.playlists[index];
            entry.cancel_timer();
            entry.next_refresh_at = next_refresh_at;
            entry.refresh_info = refresh_info;
            entry
        } else {
            self.playlists.push(PlaylistRefresh {
                playlist_id,
                next_refresh_at,
                refresh_info,
                timer_id: None,
            });
            self.playlists.last_mut().unwrap()
        };
        if followed {
            // Even an overdue response needs a callback: waiting a full interval here adds
            // latency, while relying on segment selection can stall when buffering is full.
            let remaining = (next_refresh_at - jsMonotonicNow()).max(0.);
            entry.timer_id = Some(jsTimer(remaining, TimerReason::MediaPlaylistRefresh));
        }
    }

    /// Return true if a timer is already running, or start one for the remaining wait.
    /// Return false when this playlist can be requested now.
    fn wait_for_refresh(&mut self, id: &MediaPlaylistPermanentId) -> bool {
        let Some(entry) = self
            .playlists
            .iter_mut()
            .find(|entry| entry.playlist_id == *id)
        else {
            return false;
        };
        if entry.timer_id.is_some() {
            return true;
        }
        let remaining = entry.next_refresh_at - jsMonotonicNow();
        if remaining <= 0. {
            return false;
        }
        entry.timer_id = Some(jsTimer(remaining, TimerReason::MediaPlaylistRefresh));
        true
    }

    /// A canceled timer's callback is ignored. Its playlist's next refresh time stays recorded.
    pub(super) fn on_timer_ended(
        &mut self,
        timer_id: TimerId,
    ) -> Option<(MediaPlaylistPermanentId, MediaType)> {
        let entry = self
            .playlists
            .iter_mut()
            .find(|entry| entry.timer_id == Some(timer_id))?;
        entry.timer_id = None;
        let playlist_id = entry.playlist_id;
        let media_type = [MediaType::Audio, MediaType::Video]
            .into_iter()
            .find(|media_type| self.followed_playlist(*media_type) == Some(playlist_id))?;
        Some((playlist_id, media_type))
    }

    fn cancel_unused_timers(&mut self) {
        let needed = [self.followed_audio, self.followed_video];
        for entry in &mut self.playlists {
            if !needed.contains(&Some(entry.playlist_id)) {
                entry.cancel_timer();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlaylistRefreshManager;
    use crate::{
        bindings::{MediaType, TimerId, TimerReason},
        parser::{MediaPlaylistPermanentId, PlaylistRefreshInfo},
    };
    use std::cell::RefCell;

    #[derive(Default)]
    struct Clock {
        now: f64,
        timers: Vec<f64>,
        canceled: Vec<TimerId>,
    }

    thread_local! {
        static CLOCK: RefCell<Clock> = RefCell::new(Clock::default());
    }

    pub(super) fn now() -> f64 {
        CLOCK.with_borrow(|clock| clock.now)
    }

    pub(super) fn timer(duration: f64, _reason: TimerReason) -> TimerId {
        CLOCK.with_borrow_mut(|clock| {
            clock.timers.push(duration);
            clock.timers.len() as f64
        })
    }

    pub(super) fn clear_timer(id: TimerId) {
        CLOCK.with_borrow_mut(|clock| clock.canceled.push(id));
    }

    fn refresh_info() -> Option<PlaylistRefreshInfo> {
        Some(PlaylistRefreshInfo {
            target_duration: 6.,
            last_segment_duration: Some(6.),
            tail: Some((crate::parser::SegmentIdentity::new(1, 0), 6.)),
        })
    }

    fn reset_clock() {
        CLOCK.with_borrow_mut(|clock| *clock = Clock::default());
    }

    #[test]
    fn deadlines_compensate_for_requests_and_unchanged_media_retries_sooner() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        let mut refresh_info = refresh_info().unwrap();
        refresh_info.target_duration = 10.;
        refresh_info.last_segment_duration = Some(6.);
        manager.follow(MediaType::Video, id);

        CLOCK.with_borrow_mut(|clock| clock.now = 1200.);
        manager.on_playlist_loaded(id, Some(refresh_info), 200.);
        // Six-second cadence includes the second spent downloading and parsing.
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![5000.]));

        CLOCK.with_borrow_mut(|clock| clock.now = 7200.);
        manager.on_playlist_loaded(id, Some(refresh_info), 6200.);
        // No new media: half the target (five seconds), minus request duration.
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![5000., 4000.]));

        refresh_info.tail = Some((crate::parser::SegmentIdentity::new(2, 0), 6.));
        CLOCK.with_borrow_mut(|clock| clock.now = 12200.);
        manager.on_playlist_loaded(id, Some(refresh_info), 11200.);
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![5000., 4000., 5000.]));

        // A growing final segment also counts as new media, even with the same sequence.
        refresh_info.tail = Some((crate::parser::SegmentIdentity::new(2, 0), 7.));
        refresh_info.last_segment_duration = Some(7.);
        CLOCK.with_borrow_mut(|clock| clock.now = 18200.);
        manager.on_playlist_loaded(id, Some(refresh_info), 17200.);
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers.last(), Some(&6000.)));
    }

    #[test]
    fn slow_responses_schedule_an_immediate_callback_even_without_segment_selection() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        manager.follow(MediaType::Video, id);
        CLOCK.with_borrow_mut(|clock| clock.now = 9000.);
        manager.on_playlist_loaded(id, refresh_info(), 1000.);
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![0.]));
        assert_eq!(manager.on_timer_ended(1.), Some((id, MediaType::Video)));
        assert!(manager.follow(MediaType::Video, id));
    }

    #[test]
    fn empty_playlists_use_target_duration_then_half_target_until_media_arrives() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        let empty = PlaylistRefreshInfo {
            target_duration: 5.,
            last_segment_duration: None,
            tail: None,
        };
        manager.follow(MediaType::Video, id);
        CLOCK.with_borrow_mut(|clock| clock.now = 500.);
        manager.on_playlist_loaded(id, Some(empty), 0.);
        CLOCK.with_borrow_mut(|clock| clock.now = 5500.);
        manager.on_playlist_loaded(id, Some(empty), 5000.);
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![4500., 2000.]));

        CLOCK.with_borrow_mut(|clock| clock.now = 8000.);
        manager.on_playlist_loaded(id, refresh_info(), 7500.);
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers.last(), Some(&5500.)));
    }

    #[test]
    fn switching_back_waits_only_until_the_original_deadline() {
        reset_clock();
        let first = MediaPlaylistPermanentId::new_for_test(1);
        let second = MediaPlaylistPermanentId::new_for_test(2);
        let mut manager = PlaylistRefreshManager::new();
        assert!(manager.follow(MediaType::Video, first));
        manager.on_playlist_loaded(first, refresh_info(), 0.);
        assert!(manager.follow(MediaType::Video, second));
        assert_eq!(manager.on_timer_ended(1.), None);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![1.]));

        CLOCK.with_borrow_mut(|clock| clock.now = 2000.);
        assert!(!manager.follow(MediaType::Video, first));
        // Following again must neither duplicate the timer nor postpone its deadline.
        assert!(!manager.follow(MediaType::Video, first));
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers, vec![6000., 4000.]));

        manager.follow(MediaType::Video, second);
        CLOCK.with_borrow_mut(|clock| clock.now = 7000.);
        assert!(manager.follow(MediaType::Video, first));
        CLOCK.with_borrow(|clock| assert_eq!(clock.timers.len(), 2));
    }

    #[test]
    fn shared_playlist_keeps_its_timer_until_both_types_stop_following() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        manager.follow(MediaType::Video, id);
        manager.on_playlist_loaded(id, refresh_info(), 0.);
        assert!(!manager.follow(MediaType::Audio, id));
        manager.forget_media_type(MediaType::Video);
        CLOCK.with_borrow(|clock| assert!(clock.canceled.is_empty()));
        assert_eq!(manager.on_timer_ended(1.), Some((id, MediaType::Audio)));

        manager.on_playlist_loaded(id, refresh_info(), 0.);
        manager.forget_media_type(MediaType::Audio);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![2.]));
        assert_eq!(manager.on_timer_ended(2.), None);
    }

    #[test]
    fn loading_inactive_playlists_defers_timers_and_endlist_cancels_refresh() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        manager.on_playlist_loaded(id, refresh_info(), 0.);
        CLOCK.with_borrow(|clock| assert!(clock.timers.is_empty()));
        assert!(!manager.follow(MediaType::Video, id));
        manager.on_playlist_loaded(id, None, 0.);
        assert!(manager.playlists.is_empty());
        assert_eq!(manager.on_timer_ended(1.), None);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![1.]));

        manager.on_playlist_loaded(id, refresh_info(), 0.);
        manager.clear();
        assert!(!manager.is_following(&id));
        assert!(manager.playlists.is_empty());
        assert_eq!(manager.on_timer_ended(2.), None);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![1., 2.]));
    }
}
