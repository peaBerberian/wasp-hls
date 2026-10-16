# Playlist parsing

This directory regroups the code translating HLS playlists into more exploitable
structures, that it defines for the rest of the player.

The entry point is `TopLevelPlaylist::parse`. It takes downloaded playlist data,
then returns either a Multivariant Playlist (a.k.a. Master Playlist) or a Media
Playlist directly depending of what the current content loaded is.

On top of parsing, this code:

- owns the relationships between variants, tracks and their Media Playlists,

- resolves URLs and constructs segment timelines.

When a Media Playlist is refreshed, it can use the previous version or another
loaded playlist to keep those timelines aligned.

The main structures are exported through `mod.rs`. Most of the code is split
between `multi_variant_playlist.rs`, describing the available variants and
renditions, and `media_playlist.rs`, describing segments and playlist
characteristics. The other files provide their parsing helpers and data types.

## Where it stops

Here we describe what is available in the playlists. Deciding what to play,
request and refresh is left to the caller.

This directory doesn't perform requests, parse media segment bytes or interact
with media buffers. Also, successful parsing isn't full HLS validation: some tags
and attributes may be ignored, and parsing a playlist doesn't guarantee that we
can play it.
