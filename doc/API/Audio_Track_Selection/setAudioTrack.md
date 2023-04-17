# `setAudioTrack` method

## Description

Change the currently-loaded audio track (e.g. change the audio language, switch
to an audio track describing what's happening on the screen, switch to an audio
track relying on more channels and so on.).

This methods takes a `number` in argument which corresponds to the corresponding
track object's `id` property. The main location where you may find those objects
are through the [`getAudioTrackList` method](./getAudioTrackList.md) and the
`audioTrackListUpdate` [event](../Player_Events.md).

You can also set the argument to `null` to let the `WaspHlsPlayer` set a
default audio track.

## Influence on variants

updating the audio track through `setAudioTrack` may also changes the
list of available "variants" (groups of qualities) as returned for example by
the [`getVariantList` method](../Variant_Selection/getVariantList.md), in which
case you will also receive a corresponding `variantListUpdate`
[event](../Player_Events.md). This is because some audio tracks may only be
compatible with some variants but not others.

This also means that changing the audio track may also trigger an automatic
"unlocking" of a variant previously locked through a `lockVariant` call, in
which case you will receive the corresponding `variantLockUpdate`
[event](../Player_Events.md).

It should be noted however that such scenarios are rare and may only be seen in
the few HLS contents which enforce such rules.

## Note about its asynchronicity

As the `WaspHlsPlayer` relies on a Worker where most actions actually occur
asynchronously, the audio track will not be updated synchronously after this
call.

Likewise, calling the [`getCurrentAudioTrack` method](./getCurrentAudioTrack.md)
synchronously after calling `setAudioTrack` may not returns the characteristics
of the set audio track yet.

You will receive a `audioTrackUpdate` [event](../Player_Events.md) once the
audio track is known to be actively loaded by the `WaspHlsPlayer`.

## Syntax

```js
player.setAudioTrack(trackId);
```

- **arguments**:

  1. _trackId_ `number|null`: The wanted track's `id` property (see
     [`getAudioTrackList` method](./getAudioTrackList.md) and `audioTrackListUpdate`
     [event](../Player_Events.md).

     Can be set to `null` to let the `WaspHlsPlayer` set a default one instead.
