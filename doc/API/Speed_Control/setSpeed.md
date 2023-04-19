# `setSpeed` method

## Description

Update the `speed` of playback for the currently-loaded content.

The "speed" of playback is the average pace at which the content will be played.
A speed of `1` indicates that the `WaspHlsPlayer` will try to play 10 seconds of
media content in 10 seconds, a speed of `2` indicates that it will try to play
it in 5 seconds, a speed of `0.5`, and that will be 20 seconds and so on.

Calling `setSpeed` can only be done when the `WaspHlsPlayer` is in the
`"Loaded"` [state](../Basic_Methods/getPlayerState.md) and thus when a content
is currently loaded.
Calling it in any other scenario leads to an error being thrown.

You may know the current speed of playback by calling the [`getSpeed`](./getSpeed.md)
method, note however that `getSpeed` may not be up-to-date yet if `setSpeed` has
been called very recently. This is because `getSpeed` only returns the speed
considered by the `WaspHlsPlayer`'s WebWorker and that WebWorker communication
happens asynchronously.

## Syntax

```js
player.setSpeed(newSpeed);
```

- **arguments**:

  1. _newSpeed_ `number`: The new playback speed to set.
