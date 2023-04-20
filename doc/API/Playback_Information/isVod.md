# `isVod` method

## Description

Returns `true` when the currently-loaded content can be considered a "VOD"
content.

Returns `false` either when not playing a VOD content or when no content is
loaded yet.

A content is considered as VOD if all of its media playlists have the `VOD`
playlist type.
Consequently, a VOD content will never have new segments added or old segments
removed, which also means that its ["minimum position"](../Position_Control/getMinimumPosition.md)
and ["maximum position"](../Position_Control/getMinimumPosition.md) won't
evolve since the content is loaded.

Note that a content can start as a non-VOD content but may end as one.

You can be warned when `isVod`'s value might have changed by listening to the
`ContentInfoUpdate` [event](../Player_Events.md). You can also know when it is
first set after a [`load`](../Basic_Methods/load.md) call either by listening to
this same `ContentInfoUpdate` event or by calling `isLive` when reaching the
`"Loaded"` [state](../Basic_Methods/getPlayerState.md).

## Syntax

```js
const isVod = player.isVod();
```

- **return value**:

`boolean`: Return `true` if the current content may be considered a VOD
content.
