# Player Events

## Overview

As it begins to load a content, the `WaspHlsPlayer` will send various events
allowing you:

- to let you know the current playback conditions
- what audio tracks are available and which one is selected
- what variants (i.e. qualities) are available and which one is selected
- and so on

This page will document every one of them.

## Listening to an event

The `WaspHlsPlayer` copies the same event listening API than the `EventTarget`
you're generally used to on the web. That is, you can use a method called
[`addEventlistener`](./Basic_Methods/addEventListener.md) to register a callback on an event
and [`removeEventListener`](./Basic_Methods/removeEventListener.md) to remove it.

For example, to add then immediately remove an event listener for the
`"playerStateChange"` event, you can write:

```js
const onPlayerStateChange = (state) => {
  console.log("new player state:", state);
};
player.addEventListener("playerStateChange", onPlayerStateChange);
player.removeEventListener("playerStateChange", onPlayerStateChange);
```

## Event: `playerStateChange`

The `"playerStateChange"` event is sent when the "state" of playback is
updpated, with a string describing that new state as a payload.

This is the event you want to listen to to be alerted when your [loaded](./Loading_a_content.md)
content can start to play, or when it was actually stopped.

The various values that this state can be set to are:

- `"Stopped"`: The content has just been stopped. No content is playing
  anymore.

- `"Loading"`: Set synchronously when the `load` method is called, to indicate
  that a new content is being loaded.

- `"Loaded"`: the last content loaded with `load` is now ready to play.

- `"Error"`: the last content loaded with `load` has been interrupted due to
  an error. An `"error"` event should also have been triggered. You can
  also know which error happened by calling the [`getError`](XXX TODO) method.

As such, for example to automatically play when the content is loaded, you can
combine this event with a call to [`resume`](XXX TODO), by writing:

```js
player.addEventListener("playerStateChange", (state) => {
  if (state === "Loaded") {
    // auto-play when loaded
    player.resume();
  }
});
```

You can also know the player's state at any time by calling the
[`getPlayerState`](XXX TODO) method.

## Event: `paused`

The `"paused"` event is sent when a loaded content (which is a content which
currently is in the [`"Loaded"` state](XXX TODO) was just paused, generally
due to a previous call to the [`pause` method](XXX TOOD).

This event doesn't have a payload.

Example of usage:

```js
player.addEventListener("paused", () => {
  console.log("Playback is now effectively paused.");
});
```

You can also know whether playback is currently paused at any time by calling
the [`isPaused`](XXX TODO) method.

## Event: `playing`

The `"playing"` event is sent when a loaded content (which is a content which
currently is in the [`"Loaded"` state](XXX TODO) went out of a "paused" status,
generally due to a previous call to the [`resume` method](XXX TOOD).

XXX TODO what about when ended then calling play? To check

This event doesn't have a payload.

Example of usage:

```js
player.addEventListener("playing", () => {
  console.log("Playback is now effectively playing.");
});
```

You can also know whether playback is currently playing at any time by calling
the [`isPlaying`](XXX TODO) method.

## Event: `ended`

The `"ended"` event is sent when playback reached the end of the content.

Playback is now paused, generally at the last video frame visible of the
content.

XXX TODO is the "paused" event also sent? To check.

This event doesn't have a payload.

Example of usage:

```js
player.addEventListener("ended", () => {
  console.log("Playback is now ended.");
});
```

You can also know whether the end is currently reached at any time by calling
the [`isEnded`](XXX TODO) method.

## Event: `error`

The `"error"` event is sent when an error interrupted playback of the last
content.

The corresponding Error object is sent as a payload.
For more information on the potential errors see the [Errors and warnings
page](XXX TODO).

Example of a callback registered to that event:

```js
player.addEventListener("error", (error) => {
  console.error("An error just stopped playback:", error);
});
```

Just before the `"error"` event is sent, the player's start (as returned by the
[getPlayerState](XXX TODO) method and emitted by the `playerStateChange` event
are set to `"Error"`.

Also, the [`getError`](XXX TODO) method should now return the same error object
than the one emitted as a payload of the `error` event, until another content is
loaded or until [`stop`](XXX TODO) is called, whichever comes first.

## Event: `warning`

The `"warning"` event is sent when a minor error happened, though unlike the
`error` event, it hasn't resulted in playback interruption.

The corresponding Error object is sent as a payload.
For more information on the potential errors see the [Errors and warnings
page](XXX TODO).

Example of a callback registered to that event:

```js
player.addEventListener("warning", (error) => {
  console.warn("A minor error just happened:", error);
});
```

## Event: `rebufferingStarted`

The `"rebufferingStarted"` event is sent when a loaded content (which is a
content which currently is in the [`"Loaded"` state](XXX TODO) just began
rebuffering.

_Rebuffering is a period during which playback is paused to build back buffer,
in that condition, playback will only restart (by itself) once enough buffer
is loaded._

_Rebuffering can for example happen if the network bandwidth is currently too
low to play sustainably the current content or due to some other event like
a [`seek`](XXX TODO)._

_Note that rebuffering can also happen when playback is paused. It just means
that there's not enough media data to begin playback._

This event doesn't have a payload.

Example of usage:

```js
player.addEventListener("rebufferingStarted", () => {
  console.log("Playback is now paused due to a started rebuffering period");
});
```

You can also know whether playback is currently rebuffering at any time by
calling the [`isRebuffering`](XXX TODO) method.

## Event: `rebufferingEnded`

The `"rebufferingEnded"` event is sent when a loaded content (which is a
content which currently is in the [`"Loaded"` state](XXX TODO) just exited a
rebuffering period.

You should have previously received a `"rebufferingStarted"` event when that
rebuffering period had started.

_Rebuffering is a period during which playback is paused to build back buffer,
in that condition, playback will only restart (by itself) once enough buffer
is loaded._

_Rebuffering can for example happen if the network bandwidth is currently too
low to play sustainably the current content or due to some other event like
a [`seek`](XXX TODO)._

_Note that rebuffering can also happen when playback is paused. It just means
that there's not enough media data to begin playback._

This event doesn't have a payload.

Example of usage:

```js
player.addEventListener("rebufferingEnded", () => {
  console.log("Playback can now restart as we're exited a rebuffering period.");
});
```

You can also know whether playback is currently rebuffering at any time by
calling the [`isRebuffering`](XXX TODO) method.

## Event: `variantUpdate`

The `"variantUpdate` event is sent when the currently-loaded variant, which
basically represents the video and audio qualities, has changed.

Note that the `variantUpdate` is only about the variant being loaded, which may
be different than the one being currently played (you're generally playing
already-loaded content).

The payload of that event contains the information available on that variant if
known, or `undefined` if the characteristics of the variant is unknown.
When set to an object, it should contain the following keys:

- `id` (`number`): The identifier for that variant. Might be useful for
  example when wanting to lock that variant in place through the
  [`lockVariant`](XXX TODO) method.

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

You can also know at any time the same characteristics of the current variant
by calling the [`getCurrentVariant`](XXX TODO) method.

## Event: `variantLockUpdate`

The `"variantLockUpdate` event is sent when the current variant has been
"locked" or unlocked.

A variant lock is the result of calling the [`lockVariant`](XXX TODO) method,
which allows to force a given variant (e.g. manually forcing 1080p video
content).
When a single variant is forced, we say that it is "locked", when the
`WaspHlsPlayer` actually chooses its variant amongst the pool of
currenly-available ones, we say that it is "unlocked".

This variant may be locked due to various events. For example due to a
[`unlockVariant`](XXX TODO) call or due to a change of track (e.g. through the
[`setAudioTrack`](XXX TODO) method) incompatible with the locked variant.

When the variant is actually "unlocked", the payload of that event will be
`null`.

When the variant is now "locked", it is set to an object representing that
variant's characteristics. That object will have the following keys (same than
for the `variantUpdate` event):

- `id` (`number`): The identifier for that variant. Might be useful for
  example when wanting to lock that variant in place through the
  [`lockVariant`](XXX TODO) method.

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

If that change of lock status led to a change of currently-loaded variant,
you'll also receive a `variantUpdate` event.

You can know at any time whether a variant is currently locked and which one
from the [`getLockedVariant`](XXX TODO) method.

## Event: `variantListUpdate`

The `"variantListUpdate` event is sent when the list of available variants,
which basically represent the video and audio qualities, has changed.

The payload of that event contains an array object, each object containing the
information available for a particular variant.

Each object should contain the following keys (same than for the
`variantUpdate` event):

- `id` (`number`): The identifier for that variant. Might be useful for
  example when wanting to lock that variant in place through the
  [`lockVariant`](XXX TODO) method.

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

You can also know at any time the list of available variants by calling the
[`getVariantList`](XXX TODO) method.

## Event: `audioTrackUpdate`

The `"audioTrackUpdate` event is sent when the currently-loaded audio track has
changed.

Note that what we call "audio track" here may actually be a set of multiple
audio qualities (generally dispatched in various variants) all with the same
characteristics (same language, same name, same accessibility, same number of
channels etc.).

The payload of that event contains the information available on that audio track
if known, or `undefined` either if the characteristics of the audio track is
unknown or if no audio track is active.
When set to an object, it should contain the following keys:

- `id` (`number`): The identifier for that audio track. It is generally useful
  to for example set the audio track though a [`setAudioTrack`](XXX TODO) call.

- `language` (`string | undefined`): The primary language used in this audio
  track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).

  `undefined` if unknown or if there's no language involved.

- `assocLanguage` (`string | undefined`): A secondary language associated to the
  audio track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).
  Such language is often used in a different role than the language specified
  through the `language` property (e.g., written versus spoken, or a fallback
  dialect).

  `undefined` if unknown or if there's no language involved.

- `name` (`string`): Human-readable description of the audio track.
  If the `language` property is set, it should generally be in that language.

- `channels` (`number | undefined`): If set, it is the count of audio channels,
  indicating the maximum number of independent and simultaneous audio channels
  present in any media data in that audio track.

  For example, an AC-3 5.1 Rendition would have a CHANNELS="6" attribute.

You can also know at any time the same characteristics of the current audio
track by calling the [`getCurrentAudioTrack`](XXX TODO) method.

## Event: `audioTrackListUpdate`

The `"audioTrackListUpdate` event is sent when the list of available audio
tracks has changed.

The payload of that event contains an array object, each object containing the
information available for a particular audio track.

Each object should contain the following keys (same than for the
`audioTrackUpdate` event):

- `id` (`number`): The identifier for that audio track. It is generally useful
  to for example set the audio track though a [`setAudioTrack`](XXX TODO) call.

- `language` (`string | undefined`): The primary language used in this audio
  track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).

  `undefined` if unknown or if there's no language involved.

- `assocLanguage` (`string | undefined`): A secondary language associated to the
  audio track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).
  Such language is often used in a different role than the language specified
  through the `language` property (e.g., written versus spoken, or a fallback
  dialect).

  `undefined` if unknown or if there's no language involved.

- `name` (`string`): Human-readable description of the audio track.
  If the `language` property is set, it should generally be in that language.

- `channels` (`number | undefined`): If set, it is the count of audio channels,
  indicating the maximum number of independent and simultaneous audio channels
  present in any media data in that audio track.

  For example, an AC-3 5.1 Rendition would have a CHANNELS="6" attribute.

You can also know at any time the list of available audio tracks by calling the
[`getAudioTrackList`](xxx todo) method.
