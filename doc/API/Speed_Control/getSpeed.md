# `getSpeed` method

## Description

Returns the last applied `speed` of playback on the currently-loaded content,
or `1` if no content is currently loaded.

The "speed" of playback is the average pace at which the content will be played.
A speed of `1` indicates that the `WaspHlsPlayer` will try to play 10 seconds of
media content in 10 seconds, a speed of `2` indicates that it will try to play
it in 5 seconds, a speed of `0.5`, and that will be 20 seconds and so on.

To update the speed of playback you need to call the `setSpeed` method when a
content is loaded (the `WaspHlsPlayer` is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md)). Note however that `getSpeed` won't
return that new value synchronously after the `setSpeed` call as the new speed
is first processed by the `WaspHlsPlayer`'s WebWorker, an inherently
asynchronous process.

## Syntax

```js
const speed = player.getSpeed();
```

- **return value**:

`number`: The last applied playback speed on the currently loaded content.
