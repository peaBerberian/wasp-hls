# `getCurrentBufferGap` method

## Description

`getCurrentBufferGap` returns the time difference in seconds between the end of
the current buffered range of data and the current position.

Returns `0` if data around the current position is not loaded or if no content
is loaded.

That is, it is the amount of media data to play in seconds before we might enter
rebuffering if no new segment is loaded (unless the range finishes at the
content's end and unless a seek is performed since then).

That amount of time, called the "buffer gap" is generally evolving. It might be
used in an application to give a visual indicator of until when media data is
loaded in the current content.

Note that the `WaspHlsPlayer` won't indefinitely load new data in front of the
current position to increase that buffer gap. How much data is loaded depends
mostly on the `bufferGoal` [configuration](../Configuration_Object.md).

## Syntax

```js
const bufferGap = player.getCurrentBufferGap();
```

- **return value**:

`number`: Returns the time difference in seconds between the end of the current
buffered range of data and the current position.
`0` if data around the current position is not loaded or if no content is
loaded.
