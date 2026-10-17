# Playlist store

This directory is where we keep the playlist information for the current content,
alongside the choices made from it. The main entry point is `PlaylistStore` in
`mod.rs`. It handles both multivariant playlists and media playlists loaded
directly (where there's no variant to choose).

Its job is to:

- Be the entry point to transform loaded playlist data, into the actual
  MultivariantPlaylist or MediaPlaylist struct data (or updates).

- Own the top-level playlist and its loaded media playlists.

- Keep track of the selected variant and audio/video playlists, explicit audio
  track and variant choices. Applying a choice also figures out which media
  playlists change as a consequence.

- Expose the available variants and tracks, taking audio track compatibility and
  known stream support into account when providing variant candidates.

- Keep stream metadata and codec support results, and figure out what's still
  missing before being "usable": a variant choice, loaded playlists, segment
  probes or a support check.

- Provide content information derived from the playlists, such as reachable
  positions, an expected starting position and whether the playlists are final.

One distinction to keep in mind: a selected playlist isn't necessarily loaded
yet.

Moreover, the concept of an "audio track" is separate from an "audio playlist":
an audio playlist is a media playlist transporting audio, an audio track is a
grouping of playlists appearing to be different renditions of the same content.

Playlists containing both audio and video are handled as video here, without a
separate audio playlist selection.

The caller can restrict the allowed variant/audio-track combinations for a
content. The store applies that restriction to available choices and selection
requests, separately from codec support. It retains the restriction across track
changes. The caller determines the policy; the store only provides playlist and
codec metadata and enforces the resulting allowed combinations.

## Where it stops

The store is fed playlist data and probe results by its caller. In return, its
operations report changes or what is still needed so the caller can act on them.

It checks stream support, but doesn't perform downloads or inspect segment bytes
itself. It also doesn't estimate bandwidth, choose an adaptive quality or manage
the media buffer. Selecting a playlist here only updates the selection state;
making that selection play is left to the caller.
