# `isPaused` method

## Description

Returns `true` when playback is currently paused.

Playback may restart only once a `resume` call is called:

- from the current position if playback was not [ended](./isEnded.md).

- from the minimum position if playback was ended.

You can also know when a pause begins by listening to the `paused`
[event](../Player_Events.md).

## Syntax

```js
const isPaused = player.isPaused();
```

- **return value**:

`boolean`: Return `true` if playback is considered paused.
