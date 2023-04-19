# `getVariantList` method

## Description

Returns the list of available HLS variants for the currently loaded content.

A variant in HLS is basically a set of media rendition in a given quality. The
`WaspHlsPlayer` may regularly switch between variant, for example because of
changing network conditions unless the current variant has been [locked](./lockVariant.md).

This method will returns an array of objects, each object containing the
information available for a particular variant.

Each of those objects should contain the following keys:

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

That list of variants is known once the `variantListUpdate`
[event](../Player_Events.md) is sent for the currently-loaded content, which
should happen at least once before the content is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) (and thus before playback starts).

If no content is currently loaded or if there is but variants
characteristics are either inexistant or unknown, this method will return an
empty array (`[]`).

## Syntax

```js
const variants = player.getVariantList();
```

- **return value**:

`Array.<Object>`: Characteristics of the currently available variants (see
previous chapter). Empty if no content is loaded, if there's no if variants'
characteristics are unknown.
