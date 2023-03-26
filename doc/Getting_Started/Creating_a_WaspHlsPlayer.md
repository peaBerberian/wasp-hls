# Creating a new WaspHlsPlayer

## Instantiation

Each `WaspHlsPlayer` allows to play HLS content on a single video element.

That video element has to be provided on instanciation like this:

```js
const player = new WaspHlsPlayer(videoElement);
```

More information on the WaspHlsPlayer's constructor can be found [in the
API documentation page presenting the instantiation step](../API/Instantiation.md).

Note that the `WaspHlsPlayer`'s constructor optionally can take a second
argument, which allows to overwrite its initial configuration.
More information on this object is available [in the API documentation](XXX TODO).

Before being ready to load contents on that new instance, we now have to
"initialize" it, which is an operation described in the next chapter.

## WaspHlsPlayer initialization

Before it can actually load a content, the `WaspHlsPlayer` needs to let it have
access to two files:

1. The worker file, which contains code which will run concurrently to your
   application.

2. The WebAssembly file, used by the worker file to run efficiently its
   internal logic.

Both of those files can be retrieved [in the release page](https://github.com/peaBerberian/wasp-hls/releases)
(you MUST choose the one linked to your actual `WaspHlsPlayer`'s version).
They then have to be served via HTTP(S) (through a solution of your choosing),
and can be communicated to the `WaspHlsPlayer` through its `initialize` method:

```js
player
  .initialize({
    workerUrl: "https://www.example.com/worker.js",
    wasmUrl: "https://www.example.com/wasp_hls_bg.wasm",
  })
  .then(
    () => {
      // we can now use the player
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    }
  );
```

For more information on this "initialization" step, you can consult [the API
documentation page dedicated to it, here](../API/Initialization.md).
