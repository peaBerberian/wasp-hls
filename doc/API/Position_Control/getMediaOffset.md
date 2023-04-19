# `getMediaOffset` method

## Description

Returns an offset allowing to convert from playlist time to media time and
vice-versa.

To convert from media time to playlist time you substract the media offset from
that media time.

To convert from playlist time to media time you add the media offset to the
playlist time.

If no content is currently loaded, `getMediaOffset` will return `undefined`.

## Playlist time and media time

What I call the "playlist time" here is the time extrapolated from the
MediaPlaylist (for example for a live content, it might be the unix
timestamp corresponding to the time at which the corresponding media was
broadcasted), which might be different from the "media time" actually associated
to the HTML media element (such as the `currentTime` attribute from an
`HTMLMediaElement`).

In the `WaspHlsPlayer`, we always rely on the playlist time to facilitate usage
of the API.

However on native HTML API, you will generally encounter media time.

## Syntax

```js
const mediaOffset = player.getMediaOffset();
```

- **return value**:

`number|undefined`: The media offset allowing to convert between playlist and
media time.
`undefined` if no content is currently loaded.
