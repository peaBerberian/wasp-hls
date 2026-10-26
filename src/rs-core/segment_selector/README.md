# Selecting segments

This directory is where we figure out which segment should be loaded next from
the available playlists and what appears to be buffered already. The main entry
point is `NextSegmentSelectors` in `mod.rs`, which keeps a separate
`NextSegmentSelector` for audio and video.

Its job is to:

- Look for segments from the wanted position up to the buffer goal.

- Skip segments which are already buffered or do not need to be loaded again.

- Do not request segments which are still being pushed.

- Record a media segment in history when its append completes.

- Look for opportunities to replace lower-quality buffered media, keeping some
  distance from playback. The caller provides separate candidates for extending
  the buffer and replacing media already there.

- Return the associated initialization segment when needed, and identify which
  playlist should be fetched or refreshed. That playlist can differ from the one
  containing the segment returned now.

- Tell when there's no segment remaining to request.

Returning a segment doesn't mark it as loaded. The caller has to
call `validate_media` when a media append completes and `validate_init` when an
initialization segment is supplied, otherwise they may be proposed again.

`segment_loading_history.rs` remembers which segments were already supplied.
It prevents repeated downloads and decides when removed data should be loaded again.

For regular playback, `advance_position` updates the base position and removes
old history. After a seek or buffer flush, `restart_from_position` clears history
and sets the new position. A full reset also clears initialization and playlist
selection state.

## Where it stops

This code owns initialization status and validation history. It borrows playlist
segments and buffer inventory from the caller; it doesn't own or update those.

It doesn't estimate bandwidth or choose the candidate qualities, perform
requests, decide playlist refresh timing or change media buffers. It returns
segment recommendations or a playlist to follow.
