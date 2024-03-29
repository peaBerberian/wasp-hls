# `getMaximumPosition` method

## Description

`getMaximumPosition` is a method allowing to obtain the maximum playlist
position in seconds where playable data is currently available.

Basically, it is the last reachable position in the fetched media playlist, or
if there's separate audio and a video Media Playlists, the minimum of that last
reachable position between both of them (written another way: the maximum
reachable position with both audio and video playable data).

Its intended purpose is to indicate to you the range where you may be able to
[seek](./seek.md) in the content (i.e. change the position).

If no content is currently loaded, `getMaximumPosition` will return `undefined`.

Note that this minimum position might evolve over time, depdending on the type
of content being played. More information on this in this documentation page.

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

## For non-VoD contents

When playing non-VoD contents such as live contents, the maximum reachable
position might increase over time as new data may be made available
progressively.

To be alerted when the maximum position changes, you may want to listen to the
`contentInfoUpdate` [event](../Player_Events.md) which sends a `maximumPosition`
property reflecting that new maximum position as a payload.

It should be noted that in this scenario, the value returned by
`getMaximumPosition` might update, but will only do so gradually, e.g. once one
of the Media Playlist is updated.
This might be counter-intuitive if for example you expect the maximum position
to increase linearly (for example a 1 second increase every seconds) over time.

If you want to simulate a linear increase, for example, to simulate a UI
progress bar advancing at a regular pace, you'll have to calculate that linear
progression yourself (you may still want to regularly re-synchronize it by
getting `getMaximumPosition`).

You can know is you're playing such type of content by calling the [`isVod`
method](../Playback_Information/isVod.md) after reaching the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) for that content or by reading
the `isVod` property from a `contentInfoUpdate` event (which is moreover
first sent even before the `"Loaded"` state is reached).
If it returns `false`, the maximum position might increase.

For live contents for example (you can know if you're playing a live content by
calling the [`isLive` method](../Playback_Information/isLive.md) after reaching
the `"Loaded"` [state](../Basic_Methods/getPlayerState.md)) or by reading the
`isLive` property from `ContentInfoUpdate` events, the maximum position increase
can generally be approximated as a linear increase (such as 1 second every
seconds) until the end of the content (at which point `isLive` will be set to
`false`).

## For VOD contents

When playing a VOD content, the maximum position will be set before the
[`"Loaded"` state](../Basic_Methods/getPlayerState.md) is reached and won't
evolve as long as that content is loaded.

You can know is you're playing a VOD content by calling the [`isVod`
method](../Playback_Information/isVod.md) after reaching the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) for that content or by reading the
`isVod` property from a `contentInfoUpdate` [event](../Player_Events.md) (which
is moreover first sent even before the `"Loaded"` state is reached).

## Syntax

```js
const maximumPosition = player.getMaximumPosition();
```

- **return value**:

`number|undefined`: The maximum position with playable content. in playlist time
in seconds. `undefined` if no content is currently loaded.
