# `setSubtitleTrack` method

## Description

Selects a subtitle track by its `id` from
[`getSubtitleTrackList`](./getSubtitleTrackList.md).
Use [`resetSubtitleTrack()`](./resetSubtitleTrack.md) to restore automatic selection.
A call without loaded content throws an error.

A failed selection sends an asynchronous `warning` event with
`name: "WaspTrackSelectionError"`. Its `code` is `"UnknownTrack"` for an unknown ID
or `"NoCompatibleVariant"` when no variant can provide the track. The warning
includes `mediaType` and `trackId`; the selection stays unchanged.

Selection takes effect asynchronously. The `subtitleTrackUpdate`
[event](../Player_Events.md) reports when the active track changes.

## Syntax

```js
player.setSubtitleTrack(trackId);
```

- **arguments**:
  1. _trackId_ `number`: Track identifier.

Use `disableSubtitleTrack()` to disable subtitles.
