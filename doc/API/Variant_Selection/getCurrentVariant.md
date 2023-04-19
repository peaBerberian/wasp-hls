# `getCurrentVariant` method

## Description

Returns the information on the currently considered HLS variant.
Returns `undefined` if unknown or if no content is loaded.

A variant in HLS is basically a set of media rendition in a given quality. The
`WaspHlsPlayer` may regularly switch between variant, for example because of
changing network conditions unless the current variant has been [locked](./lockVariant.md).

When set, the returned object has the following properties (same than for a
`getVariantList` call):

- `id` (`number`): The identifier for that variant. Might be useful for
  example when wanting to lock that variant in place through the
  [`lockVariant`](./Variant_Selection/lockVariant.md) method.

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

The current variant should be known once the `variantUpdate`
[event](../Player_Events.md) is sent for the currently-loaded content, which
should happen at least once before the content is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) (and thus before playback starts).

## Syntax

```js
const currentVariant = player.getCurrentVariant();
```

- **return value**:

`Object`: Characteristics of the currently considered variant (see previous
chapter). `undefined` if no content is loaded or if the current variant is
unknown.
