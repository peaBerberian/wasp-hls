# `seek` method

## Description

Change the current playback position, in playlist time in seconds.

Calling `seek` can only be done when the `WaspHlsPlayer` is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) and thus when a content is currently
loaded. Calling it in any other scenario leads to an error being thrown.

This is the method you want to call when you want to "move" playback to another
position, e.g. when clicking on the "progress bar" displayed in your UI.

## About "playlist time"

As written above, the given time is in playlist time in seconds.

What I mean by that is that that time is expressed as the time extrapolated
from the MediaPlaylist (for example for a live content, it might be the unix
timestamp corresponding to the time at which the corresponding media was
broadcasted), which might be different from the "media time" actually associated
to the HTML media element (such as the `currentTime` attribute from an
`HTMLMediaElement`).

In the `WaspHlsPlayer`, we always rely on playlist time to facilitate usage of
the API.
If you wish to convert between media time and playlist time (for example if you
want to exploit HTML properties), you may obtain the offset between the two
through the [getMediaOffset method](./getMediaOffset.md).

## Syntax

```js
player.seek(newPosition);
```

- **arguments**:

  1. _newPosition_ `string`: The new position to "seek" to, in playlist time in
     seconds.
