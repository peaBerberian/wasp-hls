# `initialize` method

## Description

Initialize the `WaspHlsPlayer`, see the [API documentation page on
initialization](../Initialization.md) for more information.

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

     This object should have the following properties present:

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
success.

That Promise may also reject in case any of its step failed (such as the
fetching of the required resources), in which case the `WaspHlsPlayer` won't be
able to be used.
