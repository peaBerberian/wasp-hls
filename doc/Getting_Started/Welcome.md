<p align="center">
  <img style="max-width: 250px;" src="../images/logo.png" />
</p>

## WaspHlsPlayer overview

The `WaspHlsPlayer` is a media player library allowing to play HLS contents on
the web efficiently by exploiting a [WebWorker](https://en.wikipedia.org/wiki/Web_worker),
allowing to run concurrently to the user interface and [WebAssembly](https://webassembly.org/),
allowing to do so in an optimized way.

The `WaspHlsPlayer` requires both features to be available in environment it
runs. It also support more advanced features, such as [MSE-in-Workers](https://chromestatus.com/feature/5177263249162240)
on browsers that support them, though it can also run efficiently on
environments without those.
All of those features should ensure that an heavy UI won't have a huge influence
on media buffering, as well as ensuring that the media loading, parsing and
buffering operations won't be felt when interacting with the page.

## Very quick start

To let you quickly test the project, the `WaspHlsPlayer` provides embedded
versions of its WebAssembly and Worker files, even if I recommend to store
and serve those as separate files on production (more details in the rest of the
documentation).

You can thus very quickly test the `WaspHlsPlayer` by just installing it:

```sh
// With npm
npm install wasp-hls

// or with yarn
yarn add wasp-hls
```

And then running the following JavaScript file.

```js
import WaspHlsPlayer from "wasp-hls";
import EmbeddedWasm from "wasp-hls/wasm";
import EmbeddedWorker from "wasp-hls/worker";

const player = new WaspHlsPlayer(videoElement);
player
  .initialize({
    workerUrl: EmbeddedWorker,
    wasmUrl: EmbeddedWasm,
  })
  .catch((err) => {
    console.error("Could not initialize WaspHlsPlayer:", err);
  });
player.load(HLS_MULTIVARIANT_PLAYLIST_URL);
```

Where `HLS_MULTIVARIANT_PLAYLIST_URL` is the URL to the main playlist (called
either the Multivariant Playlist or the Master Playlist) of your HLS content.

Of course, once `initialize` has succeeded, you can play any HLS content you
want on that `WaspHlsPlayer` instance.

## The documentation pages

Those documentation pages are splitted into multiple categories:

- You're here in the "Getting Started" category which provides tutorials and
  other resources allowing to help you with basic usage of the `WaspHlsPlayer`.

- You can also dive into the [API](../API/Overview.md), which specifies the
  behavior of everything that is possible with the `WaspHlsPlayer`.
