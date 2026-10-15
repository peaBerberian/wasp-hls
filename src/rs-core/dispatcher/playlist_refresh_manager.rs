#[cfg(not(test))]
use crate::bindings::{jsClearTimer, jsMonotonicNow, jsTimer};
use crate::{
    bindings::{MediaType, TimerId, TimerReason},
    parser::MediaPlaylistPermanentId,
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

    /// Record the reload time and schedule the next refresh if this playlist is being followed.
    pub(super) fn on_playlist_loaded(
        &mut self,
        playlist_id: MediaPlaylistPermanentId,
        refresh_interval: Option<f64>,
    ) {
        self.set_next_refresh_at(
            playlist_id,
            refresh_interval.map(|interval| jsMonotonicNow() + interval),
        );
        if self.is_following(&playlist_id) && refresh_interval.is_some() {
            self.wait_for_refresh(&playlist_id);
        }
    }

    /// Remember when a loaded playlist may next be refreshed, without necessarily setting a timer.
    /// None means the playlist no longer needs refreshing.
    fn set_next_refresh_at(&mut self, id: MediaPlaylistPermanentId, next_refresh_at: Option<f64>) {
        if let Some(entry) = self
            .playlists
            .iter_mut()
            .find(|entry| entry.playlist_id == id)
        {
            entry.cancel_timer();
            if let Some(next_refresh_at) = next_refresh_at {
                entry.next_refresh_at = next_refresh_at;
                return;
            }
        }
        if let Some(next_refresh_at) = next_refresh_at {
            self.playlists.push(PlaylistRefresh {
                playlist_id: id,
                next_refresh_at,
                timer_id: None,
            });
        } else {
            self.playlists.retain(|entry| entry.playlist_id != id);
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
        parser::MediaPlaylistPermanentId,
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

    fn reset_clock() {
        CLOCK.with_borrow_mut(|clock| *clock = Clock::default());
    }

    #[test]
    fn switching_back_waits_only_until_the_original_deadline() {
        reset_clock();
        let first = MediaPlaylistPermanentId::new_for_test(1);
        let second = MediaPlaylistPermanentId::new_for_test(2);
        let mut manager = PlaylistRefreshManager::new();
        assert!(manager.follow(MediaType::Video, first));
        manager.on_playlist_loaded(first, Some(6000.));
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
        manager.on_playlist_loaded(id, Some(6000.));
        assert!(!manager.follow(MediaType::Audio, id));
        manager.forget_media_type(MediaType::Video);
        CLOCK.with_borrow(|clock| assert!(clock.canceled.is_empty()));
        assert_eq!(manager.on_timer_ended(1.), Some((id, MediaType::Audio)));

        manager.on_playlist_loaded(id, Some(6000.));
        manager.forget_media_type(MediaType::Audio);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![2.]));
        assert_eq!(manager.on_timer_ended(2.), None);
    }

    #[test]
    fn loading_inactive_playlists_defers_timers_and_endlist_cancels_refresh() {
        reset_clock();
        let id = MediaPlaylistPermanentId::new_for_test(1);
        let mut manager = PlaylistRefreshManager::new();
        manager.on_playlist_loaded(id, Some(6000.));
        CLOCK.with_borrow(|clock| assert!(clock.timers.is_empty()));
        assert!(!manager.follow(MediaType::Video, id));
        manager.on_playlist_loaded(id, None);
        assert!(manager.playlists.is_empty());
        assert_eq!(manager.on_timer_ended(1.), None);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![1.]));

        manager.on_playlist_loaded(id, Some(6000.));
        manager.clear();
        assert!(!manager.is_following(&id));
        assert!(manager.playlists.is_empty());
        assert_eq!(manager.on_timer_ended(2.), None);
        CLOCK.with_borrow(|clock| assert_eq!(clock.canceled, vec![1., 2.]));
    }
}
