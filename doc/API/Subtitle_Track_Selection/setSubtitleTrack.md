# `setSubtitleTrack` method

## Description

Selects a subtitle track by its `id` from
[`getSubtitleTrackList`](./getSubtitleTrackList.md). Pass `null` to restore automatic
selection. A call without loaded content throws an error.

An unknown ID triggers an asynchronous `warning` event and leaves the selection
unchanged.

Selection takes effect asynchronously. The `subtitleTrackUpdate`
[event](../Player_Events.md) reports when the active track changes.

## Syntax

```js
player.setSubtitleTrack(trackId);
```

- **arguments**:
  1. _trackId_ `number | null`: Track identifier, or `null` for automatic selection.

Use `disableSubtitleTrack()` to disable subtitles.
