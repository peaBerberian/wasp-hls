# `getPlayerState` method

## Description

Returns a string describing the current "state" of playback.

The various values that this state can be set to are:

- `"Stopped"`: The last content is stopped or no content was ever loaded.

- `"Loading"`: Set synchronously when the [`load` method](./load.md) is
  called, to indicate that a new content is being loaded.

- `"Loaded"`: the last content loaded with `load` is now ready to play.

- `"Error"`: the last content loaded with `load` has been interrupted due to
  an error. An `"error"` [event](../Player_Events.md) should also have been
  triggered. You can also know which error happened by calling the
  [`getError`](./getError.md) method.

You can be directly notified of when this state change by listening to the
`playerStateChange` [event](../Player_Events.md).

## Syntax

```js
const playerState = player.getPlayerState();
```

- **return value**:

`string`: String describing the playback state the player is in.
