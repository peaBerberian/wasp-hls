# `resetAudioTrack` method

## Description

Restores automatic audio track selection for the current content. The player
uses the default audio track, or the first available track if no default is set.

The change is asynchronous. The `audioTrackUpdate`
[event](../Player_Events.md) reports when and if the active track changes.
Calling this method without loaded content throws an error.

## Syntax

```js
player.resetAudioTrack();
```
