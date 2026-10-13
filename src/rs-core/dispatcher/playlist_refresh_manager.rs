use crate::{
    bindings::{jsClearTimer, jsMonotonicNow, jsTimer, MediaType, TimerId, TimerReason},
    parser::MediaPlaylistPermanentId,
};

/// Keeps playlist reload times and schedules refreshes only for playlists we need.
pub(super) struct PlaylistRefreshManager {
    playlists: Vec<PlaylistRefresh>,
    /// Explicit choices from segment selection. Before these are known, use selected playlists.
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
    }

    pub(super) fn followed_playlist(
        &self,
        media_type: MediaType,
    ) -> Option<MediaPlaylistPermanentId> {
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
    }

    pub(super) fn clear_refresh_targets(&mut self) {
        self.followed_audio = None;
        self.followed_video = None;
    }

    pub(super) fn is_following(&self, id: &MediaPlaylistPermanentId) -> bool {
        self.followed_audio == Some(*id) || self.followed_video == Some(*id)
    }

    pub(super) fn refresh_media_type(
        &self,
        id: &MediaPlaylistPermanentId,
        current_media_type: Option<MediaType>,
    ) -> Option<MediaType> {
        [MediaType::Audio, MediaType::Video]
            .into_iter()
            .find(|media_type| self.followed_playlist(*media_type) == Some(*id))
            .or_else(|| {
                current_media_type
                    .filter(|media_type| self.followed_playlist(*media_type).is_none())
            })
    }

    /// Remember when a loaded playlist may next be refreshed, without necessarily setting a timer.
    /// None means the playlist no longer needs refreshing.
    pub(super) fn set_next_refresh_at(
        &mut self,
        id: MediaPlaylistPermanentId,
        next_refresh_at: Option<f64>,
    ) {
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
    pub(super) fn resolve_timer(&mut self, timer_id: TimerId) -> Option<MediaPlaylistPermanentId> {
        let entry = self
            .playlists
            .iter_mut()
            .find(|entry| entry.timer_id == Some(timer_id))?;
        entry.timer_id = None;
        Some(entry.playlist_id)
    }

    pub(super) fn cancel_unused_timers(&mut self, needed: &[Option<MediaPlaylistPermanentId>; 2]) {
        for entry in &mut self.playlists {
            if !needed.contains(&Some(entry.playlist_id)) {
                entry.cancel_timer();
            }
        }
    }

    pub(super) fn clear_all_timers(&mut self) {
        self.clear_refresh_targets();
        for entry in &mut self.playlists {
            entry.cancel_timer();
        }
        self.playlists.clear();
    }

    /// Remove reload state for playlists no longer known to the store.
    pub(super) fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&MediaPlaylistPermanentId) -> bool,
    {
        self.playlists.retain_mut(|entry| {
            if keep(&entry.playlist_id) {
                true
            } else {
                entry.cancel_timer();
                false
            }
        });
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
        manager.cancel_unused_timers(&[None, Some(second)]);

        manager.follow(MediaType::Video, first);
        let previous = manager
            .playlists
            .iter()
            .find(|entry| entry.playlist_id == first)
            .unwrap();
        assert_eq!(previous.next_refresh_at, 6000.);
        assert!(previous.timer_id.is_none());
        assert_eq!(
            manager.refresh_media_type(&second, Some(MediaType::Video)),
            None
        );

        manager.set_next_refresh_at(first, None);
        assert!(!manager.wait_for_refresh(&first));
        manager.clear_all_timers();
        assert!(manager.playlists.is_empty());
    }
}
