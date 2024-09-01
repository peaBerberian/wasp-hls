# WaspHlsPlayer Initialization

## Description

Once it has been [Instantiated](./Instantiation.md), the `WaspHlsPlayer` needs
to be "initialized".

That initialization task is the step during which the two external parts of
the `WaspHlsPlayer`, namely its worker file and WebAssembly file, are
setup.

Both of those files can be retrieved in [in the release page](https://github.com/peaBerberian/wasp-hls/releases),
note that you should chose the files linked to the `WaspHlsPlayer`'s version used
by your application.

After recuperating both of those files, you need to host them, to then provide
their URL to the `WaspHlsPlayer`'s `initialize` method:

```js
player
  .initialize({
    // URL to the worker file
    workerUrl: "https://www.example.com/worker.js",

    // URL to the WebAssembly file
    wasmUrl: "https://www.example.com/wasp_hls_bg.wasm",

    // Optional initial bandwidth estimate, in bits per seconds.
    // Will be relied on before the `WaspHlsPlayer` is able to produce its own
    // precize estimate.
    // Can be unset or undefined to let the `WaspHlsPlayer` define its own,
    // poor, initial value.
    initialBandwidth: 200000,
  })
  .then(
    () => {
      console.log("WaspHlsPlayer initialized with success!");
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    },
  );

// we can now use the player (we don't need to await the Promise here)
```

### Preventing the need to serve those files separately

Note that if you don't want the supplementary step of serving both those
files for now, the `WaspHlsPlayer` also provides embedded versions of both.
With them, the code would be written as:

```js
import EmbeddedWasm from "wasp-hls/wasm";
import EmbeddedWorker from "wasp-hls/worker";

player
  .initialize({
    workerUrl: EmbeddedWorker,
    wasmUrl: EmbeddedWasm,
    initialBandwidth: 200000,
  })
  .then(
    () => {
      console.log("WaspHlsPlayer initialized with success!");
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    },
  );

// we can now use the player (we don't need to await the Promise here)
```

However I don't recommend relying on embedded versions for production:
Those versions lead to a huge file size (though which is drastically
reduced when compressed) and to some small inefficencies on initialization (as
those JavaScript files have to first be interpreted in the main thread).

### Be notified of when initialization succeeds (or fails)

As you can see, the `initialize` method returns a promise, which is only
resolved once the initialization process succeeded.

That promise might also reject in the following situations:

- The provided files could not be requested.
- An issue happened while trying to run and/or compile the given files

In those cases, the promise returned by `initialize` will reject.
That promise might also reject if the player was disposed (through its `dispose`
method) before initialization finished with success.

_Note that you don't have to wait for that condition before using most of the
`WaspHlsPlayer`'s methods, if you for example load a content with the
[`load`](./Loading_a_content.md) method before initialization succeeds, it
will automatically be loaded once initialization is finished"._

You can check the status of the initialization at any time by looking
at the `WaspHlsPlayer`'s `initializationStatus` property:

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

  case "errored":
    console.log("The WaspHlsPlayer's initialization has failed.");
    break;

  case "disposed":
    console.log("The WaspHlsPlayer's instance has been disposed.");
    break;
}
```

### Note for the hosting of the WebAssembly file

It is generally recommended for performance reasons to serve WebAssembly files
with the a `Content-Type` HTTP(S) response header set to `application-wasm`.

Note however that this is not an obligation and that the actual performance
impact is relatively small.
