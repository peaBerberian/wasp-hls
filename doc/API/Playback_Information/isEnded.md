# `isEnded` method

## Description

Returns `true` when playback of the current content has ended.

This `ended` status is a situation where playback is paused on the last
frame. Calling `resume` at that point will restart playing the content from its
begginning.

You can also know when playback ends by listening to the `ended`
[event](../Player_Events.md).

## Syntax

```js
const isEnded = player.isEnded();
```

- **return value**:

`boolean`: Return `true` if playback is considered ended.
