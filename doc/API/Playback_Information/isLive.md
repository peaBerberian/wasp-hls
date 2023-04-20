# `isLive` method

## Description

Returns `true` when the currently-loaded content can be considered a "live"
content.

Returns `false` either when not playing a live content or when no content is
loaded yet.

A content is considered as live if at least one of its media playlist:

- may continue to have future segments (i.e. no `EXT-X-ENDLIST` tag)
- is neither of the `VOD` nor `EVENT` playlist type (e.g. no
  `EXT-X-PLAYLIST-TYPE` tag)

Written in another way, a live content is a content whose old segments might be
removed and for which new segments may be generated in the future.

Consequently its ["minimum position"](../Position_Control/getMinimumPosition.md)
and ["maximum position"](../Position_Control/getMinimumPosition.md) may evolve
over time.

Note that a content can stop being a live content, e.g. when the live is
finished. In that case, `isLive` will return `false`.

You can be warned when `isLive`'s value might have changed by listening to the
`ContentInfoUpdate` [event](../Player_Events.md). You can also know when it is
first set after a [`load`](../Basic_Methods/load.md) call either by listening to
this same `ContentInfoUpdate` event or by calling `isLive` when reaching the
`"Loaded"` [state](../Basic_Methods/getPlayerState.md).

## Syntax

```js
const isLive = player.isLive();
```

- **return value**:

`boolean`: Return `true` if the current content may be considered a live
content.
