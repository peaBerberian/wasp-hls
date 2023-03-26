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

Because on a Web Browser, both WebWorker and WebAssembly files have to be
loaded separately from your main script, both of those also have to be
served (on your servers) separately from your application's script.

You can find them [on the release page](https://github.com/peaBerberian/wasp-hls/releases),
they are respectively the files whose name starts by "worker" and "wasm", listed
in the files of the release you're using (on the bottom of the release note).

Once they are hosted on your server, you can now play your HLS content by
writing:

```js
import WaspHlsPlayer from "wasp-hls";

const player = new WaspHlsPlayer(videoElement);

player
  .initialize({
    workerUrl: WORKER_URL,
    wasmUrl: WASM_URL,
  })
  .then(
    () => {
      // We can now load a content
      player.load(HLS_MULTIVARIANT_PLAYLIST_URL);
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    }
  );
```

Where:

- `WORKER_URL` is the URL to the `WaspHlsPlayer`'s WebWorker file,
- `WASM_URL` is the URL of the `WaspHlsPlayer`'s WebAssembly file and
- `HLS_MULTIVARIANT_PLAYLIST_URL` is the URL to the main playlist (called
  either the MultiVariant Playlist or the Master Playlist) of your HLS content.

Of course, once `initialize` has succeeded, you can play any HLS content you
want on that `WaspHlsPlayer` instance.

## The documentation pages

Those documentation pages are splitted into multiple categories:

- You're here in the "Getting Started" category which provides tutorials and
  other resources allowing to help you with basic usage of the `WaspHlsPlayer`.

- You can also dive into the [API](../API/Overview.md), which specifies the
  behavior of everything that is possible with the `WaspHlsPlayer`.
