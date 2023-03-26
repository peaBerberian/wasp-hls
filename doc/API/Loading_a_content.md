# Loading a content

## Description

Loading a content through the `WaspHlsPlayer` can only be done once it has been
[instantiated](./Instantiation.md) and once [initialization](./Initialization.md)
finished with success.

This is the step where the URL of the MultiVariant Playlist (before known as the
"Master Playlist") is provided to the `WaspHlsPlayer`, that takes care of media
playback.

That step is done through the `load` method, through a very straightforward
call:

```js
// Here `MultiVariantPlaylistUrl` is the HTTP(S) URL to the MultiVariant
// Playlist
player.load(MultiVariantPlaylistUrl);
```

You can then be notified of where the load operation is at (whether it is still
loading the content, has loaded it or encountered an error) by either listening
to [the `"playerStateChange"` event](XXX TODO) or at any point in time by
calling [the `getPlayerState` method](XXX TODO):

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
        "The last loaded content cannot play anymore due to an error."
      );
      break;
    case "Stopped":
      console.log("No content is currently loaded nor loading.");
      break;
  }
});
```

Note that the `WaspHlsPlayer` doesn't automatically play once the content is
loaded. To do so, you have to call the [`resume`](XXX TODO) method once the
`"Loaded"` player state is reached:

```js
// Automatically play the content once it's loaded
player.addEventlistener("playerStateChange", (playerState) => {
  if (playerState === "Loaded") {
    player.resume();
  }
});
```

## Syntax

```js
player.load(url);
```

- **arguments**:

  1. _url_ `string`: Url to the MultiVariant Playlist of the content you want to
     play.
