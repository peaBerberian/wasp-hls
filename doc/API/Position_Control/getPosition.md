# `getPosition` method

## Description

`getPosition` returns the current playback position in playlist time in seconds.

That is, it returns the currently-played position.

If a content is loaded, is not paused and not rebuffering, that position should
advance roughly at a linear pace, e.g. around one second every seconds if the
[`speed`](../Speed_Control/getSpeed.md) is currently set to `1` (or two seconds
per seconds with a speed of `2` and so on) as long as it is not paused, do not
rebuffers and do not encounter playback issues.
However, some small differences between increases of position and linear time
are to be expected (for example, due to various small decoding-related and
sometimes performance-related unimportant issues).

If no content is currently loaded, `getPosition` will return `0`.

## About "playlist time"

As written above, the returned time is in playlist time in seconds.

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
const position = player.getPosition();
```

- **return value**:

`number`: The current playing position, in playlist time in seconds.
