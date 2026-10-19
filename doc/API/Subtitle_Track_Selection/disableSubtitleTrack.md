# `disableSubtitleTrack` method

## Description

Disables subtitles for the current content. Use
[`resetSubtitleTrack()`](./resetSubtitleTrack.md) to restore automatic selection.

The change is asynchronous. The `subtitleTrackUpdate`
[event](../Player_Events.md) reports when subtitles are disabled.
Calling this method without loaded content throws an error.

## Syntax

```js
player.disableSubtitleTrack();
```
