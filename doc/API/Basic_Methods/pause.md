# `pause` method

## Description

Pause the currently-loaded content.
Equivalent to a video element's `pause` method.

`pause` can only be called when the `WaspHlsPlayer` instance is [in the
`"Loaded"` state](./getPlayerState.md) and thus when a content is currently
loaded. Calling it in any other scenario leads to an error being thrown.

## Syntax

```js
player.pause();
```
