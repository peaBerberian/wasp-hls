# `unlockVariant` method

## Description

"Remove" a variant lock previously set with the [`lockVariant` method](./lockVariant.md).

Once this method has been called, the `WaspHlsPlayer` will go back to choosing
the most appropriate variant, e.g. by taking into account network conditions.

`unlockVariant` can only be called when the `WaspHlsPlayer` instance is [in the
`"Loaded"` state](../Basic_Methods/getPlayerState.md) and thus when a content is
currently loaded. Calling it in any other scenario leads to an error being
thrown.

## Note about its asynchronicity

As the `WaspHlsPlayer` relies on a Worker where most actions actually occur
asynchronously, the variant will not be locked synchronously after this
call.

Likewise, calling the [`getLockedVariant` method](./getLockedVariant.md)
synchronously after calling `unlockVariant` may not return `null` yet.

You will receive a `variantLockUpdate` [event](../Player_Events.md) with a
payload set to `null` once the variant is effectively unlocked.

## Syntax

```js
player.unlockVariant();
```
