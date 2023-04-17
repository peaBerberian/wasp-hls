# `stop` method

## Description

Requests that the last loaded content stops, thus also stopping playback and
emptying buffers.

Note that because the `WaspHlsPlayer` relies on a Worker where most actions
actually occur asynchronously, the content will probably not be yet stopped
synchronously after this call.

You can know when the content is stopped by listening to when the
`playerStateChange` [event](../Player_Events.md) switches to the `"Stopped"`
state.

## Syntax

```js
player.stop();
```
