# `setSubtitleTrack` method

## Description

Selects a subtitle track by its `id` from
[`getSubtitleTrackList`](./getSubtitleTrackList.md). Pass `null` to restore automatic
selection. An unknown ID or a call without loaded content throws an error.

Selection takes effect asynchronously. The `subtitleTrackUpdate`
[event](../Player_Events.md) reports when the active track changes.

## Syntax

```js
player.setSubtitleTrack(trackId);
```

- **arguments**:
  1. _trackId_ `number | null`: Track identifier, or `null` for automatic selection.

Use `disableSubtitleTrack()` to disable subtitles.
