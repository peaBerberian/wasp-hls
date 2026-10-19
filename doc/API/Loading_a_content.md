# Loading a content

## Description

Loading a content through the `WaspHlsPlayer` can only be done once it has been
[instantiated](./Instantiation.md) and once [`initialize`](./Initialization.md)
has been called (it is not necessary to await the returned Promise).

This is the step where the URL of the top-level playlist is provided to the
`WaspHlsPlayer`, that takes care of media playback.
That top-level playlist can be either a Multivariant Playlist (before known as
the "Master Playlist") or a Media Playlist directly.

That step is done through the `load` method, through a very straightforward
call:

```js
// Here `playlistUrl` is the HTTP(S) URL to the top-level playlist.
player.load(playlistUrl);
```

You can then be notified of where the load operation is at (whether it is still
loading the content, has loaded it or encountered an error) by either listening
to [the `"playerStateChange"` event](./Player_Events.md) or at any point in time
by calling [the `getPlayerState` method](./Basic_Methods/getPlayerState.md):

```js
player.addEventlistener("playerStateChange", (playerState) => {
  switch (playerState) {
    case "Loading":
      console.log("A new content is loading.");
      break;
    case "Loaded":
      console.log("The last loaded content is currently loaded.");
      break;
    case "Error":
      console.log(
        "The last loaded content cannot play anymore due to an error.",
      );
      break;
    case "Stopped":
      console.log("No content is currently loaded nor loading.");
      break;
  }
});
```

Note that the `WaspHlsPlayer` doesn't automatically begin playback once the
content is loaded. To do so, you have to call the [`resume`](./Basic_Methods/resume.md)
method once the `"Loaded"` player state is reached:

```js
// Automatically play the content once it's loaded
player.addEventlistener("playerStateChange", (playerState) => {
  if (playerState === "Loaded") {
    player.resume();
  }
});
```

## Options

`load` also can take an optional second argument, an object representing its
options.

It can currently contain two optional properties: `startingPosition` and
`initialAudioTrack`.

### `startingPosition`

_type: `number | object | undefined`_

The `startingPosition` option allows to indicate a preferred position at which
playback should begin.

If not set or set to `undefined`, the `WaspHlsPlayer` will decide by itself
where to begin in the content, based on content information. In most cases, this
is what you want.

#### As a number

If you do want to start at a specific position, `startingPosition` can be set
as a number, which will corresponds to the wanted starting position, in terms
of content time (the position as deduced from the HLS Media Playlists, which may
not be the same than the media time inside media segments) in seconds.

For example, to begin playback of a VoD content at the second `10`, you can
write:

```js
player.load(playlistUrl, {
  startingPosition: 10,
});
```

Or to play some live contents at what was recorded one minute ago, you could
write:

```js
// Unix timestamp, in seconds, corresponding to now minus 1 minute
const date = Date.now() / 1000 - 60;

player.load(playlistUrl, {
  startingPosition: date,
});
```

#### As an object

More complex relative wanted starting positions can be communicated by setting
`startingPosition` to an object instead.

Those objects each have two properties, `startType`, a string indicating the
type of relative or absolute position, and `position` indicating the position
value.

A relative position to the initially minimum or maximum position can this way
be given respectively by setting `startType` to `"FromBeginning"` and
`"FromEnd"`.

For example, to start playback 10 seconds before the last initially available
position (10 seconds before the end for a VoD content, or before live for a live
content), you can write:

```js
player.load(playlistUrl, {
  startingPosition: {
    startType: "FromEnd",
    position: 10,
  },
});
```

If you however want to play 10 seconds after the first initially available
position, you can write:

```js
player.load(playlistUrl, {
  startingPosition: {
    startType: "FromBeginning",
    position: 10,
  },
});
```

For completeness-sake, it's also possible to communicate through an object an
absolute position, exactly as if a number was directly communicated, you can
do so through the `"Absolute"` `startType`.
Thus to start at exactly the second `30`, you can write:

```js
player.load(playlistUrl, {
  startingPosition: {
    startType: "Absolute",
    position: 30,
  },
});
```

### `initialAudioTrack`

_type: `object | Array.<object> | undefined`_

The `initialAudioTrack` option allows to indicate a preferred audio track
before the player starts requesting the initial audio playlist and media
segments.

This is useful when you want the first announced and loaded audio track to
already match a preferred language or accessibility role.

When set as an object, it can contain the following optional properties:

- `language` (`string | undefined`)
- `assocLanguage` (`string | undefined`)
- `name` (`string | undefined`)
- `characteristics` (`Array.<string> | undefined`)
- `channels` (`number | undefined`)

Every set property must match for a track to be selected. If several tracks
match, the first available track in playlist order is chosen.

You can also provide an ordered array of such objects. The player tries each
preference in order. If none matches an available track, it uses the default
audio selection.

For example, to start with the French audio track:

```js
player.load(playlistUrl, {
  initialAudioTrack: {
    language: "fr",
  },
});
```

Or to try German first and then French:

```js
player.load(playlistUrl, {
  initialAudioTrack: [{ language: "de" }, { language: "fr" }],
});
```
