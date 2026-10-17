# `getCurrentSubtitleTrack` method

## Description

Returns the active subtitle track, `null` when subtitles are disabled, or
`undefined` when no content is loaded or no active track is known. The returned object has the
same properties as those returned by
[`getSubtitleTrackList`](./getSubtitleTrackList.md).

The `subtitleTrackUpdate` [event](../Player_Events.md) reports track changes.

## Syntax

```js
const subtitleTrack = player.getCurrentSubtitleTrack();
```

- **return value**: `Object | null | undefined` describing the active track.
