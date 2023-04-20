# `getMediaDuration` method

## Description

`getMediaDuration` returns the last estimated position playable in the content
once it is fully loaded, in playlist time in seconds.

When the [position](./getPosition.md) reaches that value, the content will
generally [end](../Playback_Information/isEnded.md).

If that value isn't currently set because no content is loaded yet,
`getMediaDuration` will return `NaN`.

Under the hood, that method uses the HTML media element's `duration` attribute.

For content that aren't yet finished, i.e. contents that are not [VoD
contents](../Playback_Information/isVod.md), the duration will generally be set
to a very high value as the true latest estimated position is unknown yet.

When the content is a VOD content (when the `isVod` method returns `true), the
duration will be set as the end of the last segment.
Note however that it may slightly evolve even for VOD content when reaching the
`true` end of the content, as it goes from an estimate to its true precise
value.

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
const duration = player.getMediaDuration();
```

- **return value**:

`number`: The last estimated position playable in the content once fully loaded,
in playlist time in seconds.
`NaN` if no content is yet loaded.
