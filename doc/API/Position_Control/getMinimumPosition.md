# `getMinimumPosition` method

## Description

`getMinimumPosition` is a method allowing to obtain the minimum playlist
position in seconds where playable data is currently available.

Basically, it is the first reachable position in the fetched media playlist, or
if there's separate audio and a video Media Playlists, the minimum of that first
reachable position between both of them (written another way: the minimum
reachable position with both audio and video playable data).

Its intended purpose is to indicate to you the range where you may be able to
[seek](./seek.md) in the content (i.e. change the position).

If no content is currently loaded, `getMaximumPosition` will return `undefined`.

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

## For live contents

When playing a live content, the minimum reachable position might increase over
time as old data may become progressively unavailable.

It should be noted that in this scenario, the value returned by
`getMinimumPosition` might update, but will only do so gradually, e.g. once one
of the Media Playlist is updated.
This might be counter-intuitive if for example you expect the minimum position
to increase linearly (for example a 1 second increase every seconds) over time.

If you want to simulate a linear increase, for example, to simulate a UI
progress bar advancing at a regular pace, you'll have to calculate that linear
progression yourself (you may still want to regularly re-synchronize it by
getting `getMinimumPosition`).

You can know is you're playing a live content by calling the [`isLive`
method](XXX TODO) after reaching the `"Loaded"` [state](../Basic_Methods/getPlayerState.md)
for that content.
If it returns `true`, the minimum position might increase.
On such contents, the minimum position increase can generally be approximated as
a linear increase (such as 1 second every seconds) until the end of the content
(at which point `isLive` returns `false`).

## For VOD and EVENT contents

When playing a VOD or an HLS EVENT content, the minimum position will be set
before the [`"Loaded"` state](../Basic_Methods/getPlayerState.md) is reached
and won't evolve as long as that content is loaded.

You can know is you're playing such type of contents by calling the [`isLive`
method](XXX TODO) after reaching the `"Loaded"` [state](../Basic_Methods/getPlayerState.md)
for that content. If it returns `false`, you're playing one of both (either a
VOD or an EVENT content).

## Syntax

```js
const minimumPosition = player.getMinimumPosition();
```

- **return value**:

`number|undefined`: The minimum position with playable content. in playlist time
in seconds. `undefined` if no content is currently loaded.
