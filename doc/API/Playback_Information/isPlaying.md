# `isPlaying` method

## Description

Returns `true` when there's both:

1.  A content loaded
2.  Playback is not paused.

Note that `isPlaying` returns true even if playback is stalled due to
rebuffering (you can check [`isRebuffering`](./isRebuffering.md) for this).

You can also know when playback starts playing by listening to the `playing`
[event](../Player_Events.md).

## Syntax

```js
const isPlaying = player.isPlaying();
```

- **return value**:

`boolean`: Return `true` if playback is considered as not paused.
