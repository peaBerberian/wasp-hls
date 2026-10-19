# `resetSubtitleTrack` method

## Description

Restores automatic subtitle track selection for the current content, including
after `disableSubtitleTrack()`. Forced subtitles follow the selected audio language.

The change is asynchronous. The `subtitleTrackUpdate`
[event](../Player_Events.md) reports when and if the active track changes.
Calling this method without loaded content throws an error.

## Syntax

```js
player.resetSubtitleTrack();
```
