# `getLockedVariant` method

## Description

Returns the locked HLS variant on the currently-loaded content,
or `null` if either no content is currently loaded or if there is but no variant
is currently locked.

A variant in HLS is basically a set of media rendition in a given quality. The
`WaspHlsPlayer` may regularly switch between variant, for example because of
changing network conditions unless the current variant has been [locked](./lockVariant.md).
When a variant has been locked, `getLockedVariant` will return the
characteristics of that locked variant.

Those characteristics are the same than for most other variant API, namely:

- `id` (`number`): The identifier for that variant. Might be useful for
  example when wanting to lock that variant in place through the
  [`lockVariant`](./lockVariant.md) method.

- `width` (`number | undefined`): The optimal width at which the video media
  data linked to that variant is displayed, in pixel.

  `undefined` if unknown or if there's no video data.

- `height` (`number | undefined`): The optimal height at which the video media
  data linked to that variant is displayed, in pixel.

  `undefined` if unknown or if there's no video data.

- `frameRate` (`number | undefined`): The maximum frame for the video media data
  linked to that variant.

  `undefined` if unknown or if there's no video data.

- `bandwidth` (`number | undefined`): The peak segment bit rate of any media
  combination in that variant, in bits per second.

  `undefined` if unknown,

Note that `getLockedVariant` won't return its new value synchronously after a
`lockVariant` call as it is is first processed by the `WaspHlsPlayer`'s
WebWorker, an inherently asynchronous process. If you want to know when and if
a locked variant through the `lockVariant` is actually really being considered,
you can listen to the `variantLockUpdate` [event](../Player_Events.md).

## Syntax

```js
const variant = player.getLockedVariant();
```

- **return value**:

`Object|null`: If set, the characteristics of the locked variant. If no variant
is currently locked, returns `null`.
