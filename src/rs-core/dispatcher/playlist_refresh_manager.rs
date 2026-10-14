use crate::{
    bindings::{jsClearTimer, jsMonotonicNow, jsTimer, MediaType, TimerId, TimerReason},
    parser::MediaPlaylistPermanentId,
};

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

    pub(super) fn follow(&mut self, media_type: MediaType, id: MediaPlaylistPermanentId) {
        match media_type {
            MediaType::Audio => self.followed_audio = Some(id),
            MediaType::Video => self.followed_video = Some(id),
        }
        self.cancel_unused_timers();
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

    pub(super) fn clear_refresh_targets(&mut self) {
        self.followed_audio = None;
        self.followed_video = None;
        self.cancel_unused_timers();
    }

    pub(super) fn clear(&mut self) {
        self.clear_refresh_targets();
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
    pub(super) fn wait_for_refresh(&mut self, id: &MediaPlaylistPermanentId) -> bool {
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
    use crate::{bindings::MediaType, parser::MediaPlaylistPermanentId};

    #[test]
    fn switching_away_preserves_reload_time_and_a_finished_playlist_discards_it() {
        let first = MediaPlaylistPermanentId::new_for_test(1);
        let second = MediaPlaylistPermanentId::new_for_test(2);
        let mut manager = PlaylistRefreshManager::new();
        manager.follow(MediaType::Video, first);
        manager.set_next_refresh_at(first, Some(6000.));
        manager.follow(MediaType::Video, second);
        manager.set_next_refresh_at(second, Some(9000.));

        manager.follow(MediaType::Video, first);
        let previous = manager
            .playlists
            .iter()
            .find(|entry| entry.playlist_id == first)
            .unwrap();
        assert_eq!(previous.next_refresh_at, 6000.);
        assert!(previous.timer_id.is_none());
        assert!(!manager.is_following(&second));

        // A callback can only request a playlist that is still being followed.
        manager
            .playlists
            .iter_mut()
            .find(|entry| entry.playlist_id == second)
            .unwrap()
            .timer_id = Some(10.);
        assert_eq!(manager.on_timer_ended(10.), None);
        manager
            .playlists
            .iter_mut()
            .find(|entry| entry.playlist_id == first)
            .unwrap()
            .timer_id = Some(11.);
        assert_eq!(manager.on_timer_ended(11.), Some((first, MediaType::Video)));

        manager.set_next_refresh_at(first, None);
        assert!(!manager.wait_for_refresh(&first));
        manager.clear();
        assert!(manager.playlists.is_empty());
    }
}
