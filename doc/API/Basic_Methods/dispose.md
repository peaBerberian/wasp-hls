# `dispose` method

## Description

Stop the current content if one and free all resources taken by the
`WaspHlsPlayer`.

Note that because the `WaspHlsPlayer` relies on a Worker where most actions
actually occur asynchronously, the content will probably not be yet stopped
synchronously after this call.

## Syntax

```js
player.dispose();
```
