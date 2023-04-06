# `resume` method

## Description

Un-pause the currently-loaded content.
Equivalent to a video element's `play` method.

`play` can only be called when the `WaspHlsPlayer` instance is [in the
`"Loaded"`](XXX TODO) state.

Note that when initially loaded, the content will be in the paused state. If
you want to begin playback, you will thus have to call `resume`.

The returned promise may reject in cases where playing is blocked, such as when
"auto-playing" is disabled on the page.

You can see [this MDN
page](https://developer.mozilla.org/en-US/docs/Web/Media/Autoplay_guide) for
more information on why this promise will reject.

## Syntax

```js
const promise = player.resume();
promise.catch((err) => {
  console.warn("Impossible to play:", err);
});
```

- **return value**:

`Promise`: Result of calling `play` on the media element.

That Promise may reject in cases where playing is blocked. You can see [this
MDN page](https://developer.mozilla.org/en-US/docs/Web/Media/Autoplay_guide) for
more information on why this promise will reject.
