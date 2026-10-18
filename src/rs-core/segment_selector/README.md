# Selecting segments

This directory is where we figure out which segment should be loaded next from
the available playlists and what appears to be buffered already. The main entry
point is `NextSegmentSelectors` in `mod.rs`, which keeps a separate
`NextSegmentSelector` for audio and video.

Its job is to:

- Follow a chronological cursor from a base position, normally the wanted
  playback position, up to a configurable buffer goal.

- Skip segments when equal- or higher-quality media already covers their range.
  Skipped ranges are checked again so they can be loaded if that coverage
  disappears.

- Look for opportunities to replace lower-quality buffered media, keeping some
  distance from playback. The caller provides separate candidates for extending
  the buffer and replacing media already there.

- Return the associated initialization segment when needed, and identify which
  playlist should be fetched or refreshed. That playlist can differ from the one
  containing the segment returned now.

- Tell when there's no segment remaining to request.

Returning a segment doesn't advance the cursor past it. The caller has to
"validate" media and initialization segments through `validate_media` and
`validate_init` when accepting them, otherwise they may be proposed again.

`segment_loading_history.rs` remembers which segments were already supplied.
It prevents repeated downloads and decides when removed data should be loaded again.

For regular playback, `advance_position` updates the base position while keeping
the cursor. After a seek or buffer flush, `restart_from_position` makes selection
reconsider the new position and buffered coverage instead. A full reset also
clears initialization and playlist selection state.

## Where it stops

This code owns selection state: cursors, initialization status, skipped ranges
and validation history. It borrows playlist segments and buffer inventory from
the caller; it doesn't own or update those.

It doesn't estimate bandwidth or choose the candidate qualities, perform
requests, decide playlist refresh timing or change media buffers. It returns
segment recommendations or a playlist to follow.
