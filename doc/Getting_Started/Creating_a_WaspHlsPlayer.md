# Creating a new WaspHlsPlayer

## Instanciation

Each WaspHlsPlayer allows to play HLS content on a single video element.

That video element has to be provided on instanciation like this:
```js
const player = new WaspHlsPlayer(videoElement);
```

Before being ready to load contents on that new instance, we now have to
"initialize" it, which is an operation described in the next chapter.

## WaspHlsPlayer initialization

Before it can actually load a content, the `WaspHlsPlayer` needs to have access
to two files :

  1. The worker file, which contains code which will run concurrently to your
     application.

  2. The WebAssembly file, used by the worker file to run efficiently its
     internal logic.

Both of those files have to be served via HTTP(S) (through a solution of your
choosing), and can be communicated to the `WaspHlsPlayer` through its
`initialize` method:
```js
player.initialize({
  workerUrl: "./worker.js",
  wasmUrl: "./wasp_hls_bg.wasm",
}).then(() => {
  // we can now use the player
}, (err) => {
  console.error("Could not initialize WaspHlsPlayer:", err);
});
```

As you can see, the `initialize` method returns a promise, which is only
resolved once the initialization process succeeded. You have to wait for
that condition before using most of the WaspHlsPlayer's methods.

That promise might also reject in the following situations:
  - The provided files could not be requested.
  - An issue happened while trying to run and/or compile the given files

In those cases, the promise returned by `initialize` will reject.
That promise might also reject if the player was disposed (through its `dispose`
method) before initialization finished with success.

Note that you can check the status of the initialization at any time by looking
at the WaspHlsPlayer's `initializationStatus` property:
```js
switch (player.initializationStatus) {
  case "Uninitialized":
    console.log("The WaspHlsPlayer has never been initialized.");
    break;

  case "Initializing":
    console.log("The WaspHlsPlayer is currently initializing.");
    break;

  case "Initialized":
    console.log("The WaspHlsPlayer has been initialized with success.");
    break;

  case "errorred":
    console.log("The WaspHlsPlayer's initialization has failed.");
    break;

  case "disposed":
    console.log("The WaspHlsPlayer's instance has been disposed.");
    break;
}
```

### Note for the WebAssembly file

It is generally recommended for performance reasons to serve WebAssembly files
with the a `Content-Type` HTTP(S) response header set to `application-wasm`.

Note however that this is not an obligation and that the actual performance
impact is relatively small.
