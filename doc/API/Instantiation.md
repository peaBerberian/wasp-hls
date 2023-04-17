# WaspHlsPlayer instantiation

## Description

instantiating a `WaspHlsPlayer` is a necessary step before using most of its API,
like `load` to load a new HLS content.

It is here that the `HTMLVideoElement` (the `<video />` HTML element) on which
the content will play is given to the `WaspHlsPlayer`:

```js
import WaspHlsPlayer from "wasp-hls";

const videoElement = document.querySelector("video");
const player = new WaspHlsPlayer(videoElement);
```

_Note: For a more predictable behavior, it is best that you then use the
`WaspHlsPlayer`'s API instead of any of the `HTMLVideoElement`'s own
methods, attributes and events, excepted when the opposite is explicitely
advised by this documentation (such as with [audio volume
management](./Audio_Volume_Management.md))._

Optionally, you can give a second argument on instantiation, the
`WaspHlsPlayer`'s original [configuration object](./Configuration_Object.md):

```js
const config = {
  // Try to reach a buffer size of 20 seconds when playing
  bufferGoal: 20,

  // Re-do a segment request if it takes more than 10 seconds
  segmentRequestTimeout: 10,

  // ...
};
const player = new WaspHlsPlayer(videoElement, config);
```

You may here only define a subset of those keys, in which case the
`WaspHlsPlayer` will set sane default values for the others.
You can also either not set it, set it to `undefined` or to an empty object, in
which cases the `WaspHlsPlayerConfig` will use its inner default configuration
instead.

Note that this configuration object can be updated at any time through the
[`updateConfig`](./Basic_Methods/updateConfig.md) method.

## Syntax

```js
const player = new WaspHlsPlayer(videoElement);

// or, with a default configuration object
const player = new WaspHlsPlayer(videoElement, configObject);
```

- **arguments**:

  1. _videoElement_ `HTMLVideoElement`: The `<video />` element on the page on
     which the content will play.

  2. _config_ (optional) `Object|undefined`: Default configuration object.
