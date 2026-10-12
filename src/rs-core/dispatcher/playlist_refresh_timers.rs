use crate::{
    bindings::{jsClearTimer, jsTimer, MediaType, TimerId, TimerReason},
    parser::MediaPlaylistPermanentId,
};

/// Live/Event Media Playlist may need to be refreshed at regular interval.
///
/// This structure simplifies the setup of a timer associated to a content's media playlist(s) by
/// ensuring:
/// - that only one timer is set at once for a particular media playlist
/// - facilitating clean-up of the media playlists that are not of interest anymore
pub(super) struct PlaylistRefreshTimers {
    timers: Vec<(TimerId, MediaPlaylistPermanentId)>,
    /// Playlist to refresh for each media type, as reported by segment selection.
    refresh_targets: Vec<(MediaType, MediaPlaylistPermanentId)>,
}

impl PlaylistRefreshTimers {
    /// Create a new empty `PlaylistRefreshTimers`
    pub(super) fn new() -> Self {
        Self {
            timers: Vec::new(),
            refresh_targets: Vec::new(),
        }
    }

    pub(super) fn follow(&mut self, media_type: MediaType, id: MediaPlaylistPermanentId) {
        self.forget_media_type(media_type);
        self.refresh_targets.push((media_type, id));
    }

    pub(super) fn forget_media_type(&mut self, media_type: MediaType) {
        self.refresh_targets.retain(|(kind, _)| *kind != media_type);
    }

    pub(super) fn clear_refresh_targets(&mut self) {
        self.refresh_targets.clear();
    }

    pub(super) fn is_following(&self, id: &MediaPlaylistPermanentId) -> bool {
        self.refresh_targets.iter().any(|(_, target)| target == id)
    }

    /// Before segment selection supplies a refresh target, use the current playlist.
    pub(super) fn refresh_media_type(
        &self,
        id: &MediaPlaylistPermanentId,
        current_media_type: Option<MediaType>,
    ) -> Option<MediaType> {
        self.refresh_targets
            .iter()
            .find(|(_, target)| target == id)
            .map(|(kind, _)| *kind)
            .or_else(|| {
                current_media_type.filter(|kind| {
                    !self
                        .refresh_targets
                        .iter()
                        .any(|(target_kind, _)| target_kind == kind)
                })
            })
    }

    /// Pending timers also prevent an early reload when an inactive cached list is reused.
    pub(super) fn has_timer(&self, playlist_id: &MediaPlaylistPermanentId) -> bool {
        self.timers.iter().any(|(_, id)| id == playlist_id)
    }

    /// Add or re-set a timer for a specific media playlist, corresponding to the given
    /// `refresh_interval`.
    pub(super) fn set_timer(
        &mut self,
        pl: MediaPlaylistPermanentId,
        refresh_interval: Option<f64>,
    ) {
        // First if a timer for that one was already set-up, clear it
        self.timers.retain(|(timer_id, playlist_type)| {
            if playlist_type == &pl {
                jsClearTimer(*timer_id);
                false
            } else {
                true
            }
        });

        if let Some(refresh_interval) = refresh_interval {
            let timer_id = jsTimer(refresh_interval, TimerReason::MediaPlaylistRefresh);
            self.timers.push((timer_id, pl));
        }
    }

    /// To call once a timer has resolved: remove it from stored timers and return information on
    /// the corresponding media playlist.
    pub(super) fn resolve_timer(&mut self, timer_id: TimerId) -> Option<MediaPlaylistPermanentId> {
        let found_idx = self.timers.iter().position(|x| x.0 == timer_id)?;
        let (_, playlist_type) = self.timers.remove(found_idx);
        Some(playlist_type)
    }

    /// Clear all pending timers triggered by this struct.
    pub(super) fn clear_all_timers(&mut self) {
        self.clear_refresh_targets();
        while let Some(timer_info) = self.timers.pop() {
            jsClearTimer(timer_info.0);
        }
    }

    /// Retains only the timers specified by the predicate.
    pub(super) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&MediaPlaylistPermanentId) -> bool,
    {
        self.timers.retain(|x| {
            if !f(&x.1) {
                jsClearTimer(x.0);
                false
            } else {
                true
            }
        });
    }
}
