# `isRebuffering` method

Returns `true` when playback is currently not advancing because of
rebuffering.

You can also know when playback enters or exits a rebuffering period
respectively by listening to the `rebufferingStarted` or the `rebufferingEnded`
[events](../Player_Events.md).

_Rebuffering is a period during which playback is paused to build back buffer,
in that condition, playback will only restart (by itself) once enough buffer
is loaded._

_Rebuffering can for example happen if the network bandwidth is currently too
low to play sustainably the current content or due to some other event like
a [`seek`](../Position_Control/seek.md)._

_Note that rebuffering can also happen when playback is paused. It just means
that there's not enough media data to begin playback._

## Syntax

```js
const isRebuffering = player.isRebuffering();
```

- **return value**:

`boolean`: Return `true` if playback is currently on hold due to rebuffering.
