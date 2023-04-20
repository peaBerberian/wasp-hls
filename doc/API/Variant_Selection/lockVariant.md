# `lockVariant` method

## Description

"Lock" a specific HLS variant (i.e. quality) to prevent the `WaspHlsPlayer` from
switching from a variant to another due to e.g. changing network conditions

This methods takes a `number` in argument which corresponds to the corresponding
variant object's `id` property. The main location where you may find those
objects are through the [`getVariantList` method](./getVariantList.md) and the
`variantListUpdate` [event](../Player_Events.md).

Once locked, it is possible to `unlock` the variant for the currently-loaded
content by calling the [`unlockVariant`](./unlockVariant.md) method.

`lockVariant` can only be called when the `WaspHlsPlayer` instance is [in the
`"Loaded"` state](../Basic_Methods/getPlayerState.md) and thus when a content is
currently loaded. Calling it in any other scenario leads to an error being
thrown.

## Influence on audio tracks

Locking a variant through `lockVariant` may also trigger an automatic audio
track change previously set through a `setAudioTrack` call, in which case you
will receive the corresponding `audioTrackUpdate` [event](../Player_Events.md).

This is because some audio tracks may only be compatible with some variants
but not others.

It should be noted however that such scenarios are rare and may only be seen in
the few HLS contents which enforce such rules.

## Note about its asynchronicity

As the `WaspHlsPlayer` relies on a Worker where most actions actually occur
asynchronously, the variant will not be locked synchronously after this
call.

Likewise, calling the [`getLockedVariant` method](./getLockedVariant.md)
synchronously after calling `lockVariant` may not returns the characteristics
of the locked variant yet.

You will receive a `variantLockUpdate` [event](../Player_Events.md) once the
variant is known to be actively locked by the `WaspHlsPlayer`.

## Syntax

```js
player.lockVariant(variantId);
```

- **arguments**:

  1. _variantId_ `number`: The wanted variant's `id` property (see
     [`getVariantList` method](./getVariantList.md) and `variantListUpdate`
     [event](../Player_Events.md).
