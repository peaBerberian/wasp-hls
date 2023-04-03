# WaspHlsPlayer Initialization

## Description

Once it has been [Instantiated](./Instantiation.md), the `WaspHlsPlayer` needs
to be "initialized".

That initialization task is the step during which the two external parts of
the `WaspHlsPlayer`, namely its worker file and WebAssembly file are
setup.

Both of those files can be retrieved in [in the release page](https://github.com/peaBerberian/wasp-hls/releases),
note that you MUST chose the files linked to the `WaspHlsPlayer`'s version used
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
      // we can now use the player
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    }
  );
```

### Preventing the need to serve those files separately

Note that if you don't want to have the supplementary step of serving both those
files when developping (as opposed to when it is "in production"), the
`WaspHlsPlayer` also provides embedded versions of both.
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
      // we can now use the player
    },
    (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    }
  );
```

However I don't recommend relying on embedded versions for production:
Those versions lead to a huge file size (though which is drastically
reduced when compressed) and to some small inefficencies on initialization (as
those JavaScript files have to first be interpreted in the main thread).

### Be notified of when initialization succeeds (or fails)

As you can see, the `initialize` method returns a promise, which is only
resolved once the initialization process succeeded. You have to wait for
that condition before using most of the `WaspHlsPlayer`'s methods.

That promise might also reject in the following situations:

- The provided files could not be requested.
- An issue happened while trying to run and/or compile the given files

In those cases, the promise returned by `initialize` will reject.
That promise might also reject if the player was disposed (through its `dispose`
method) before initialization finished with success.

Note that you can check the status of the initialization at any time by looking
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

## Syntax

```js
// Without an initial bandwidth setup:
const initializationPromise = player.initialize({
  workerUrl,
  wasmUrl,
});

// With an initial bandwidth, generally to start playing with an appropriate
// quality directly:
const initializationPromise = player.initialize({
  workerUrl,
  wasmUrl,
  initialBandwidth,
});
```

- **arguments**:

  1. initObject `Object`: The properties required for initialization.

     This object MUST have the following properties present:

     - _workerUrl_ (`string`): URL to the Worker file, that you have hosted.

     - _wasmUrl_ (`string`): URL to the WebAssembly file, that you have
       hosted.

     It has one additional optional property:

     - _initialBandwidth_ (`number|undefined`): An initial bandwidth estimate,
       in bits per second, which will be relied on initially when starting to
       load the first content. If `undefined` or not set, the `WaspHlsPlayer`
       will define its own initial bandwidth, generally of a poor quality (but
       will be able to provide a better estimate soon enough).

- **return value**:

`Promise`: Promise resolving when and if the initialization step finished with
success. It is at this point that you can begin loading a content through a
`load` call.

That Promise may also reject in case any of its step failed (such as the
fetching of the required resources), in which case the `WaspHlsPlayer` won't be
able to be used.
