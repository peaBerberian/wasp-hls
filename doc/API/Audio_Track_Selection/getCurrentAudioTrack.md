# `getCurrentAudioTrack` method

## Description

Returns the information on the currently loaded audio track.
Returns `undefined` if unknown, if no content is loaded or if the content
has no audio track.

Note that an audio track is only declared through this API if it is distinctly
selectable. If you currently rely on a muxed media playlists containing video
and audio combined, it generally will return `undefined`.

When set, the returned object has the following properties (same than for a
`getAudioTrackList` call):

- `id` (`number`): The identifier for that audio track. It is generally useful
  to for example set the audio track though a [`setAudioTrack`](./setAudioTrack.md)
  call.

- `language` (`string | undefined`): The primary language used in this audio
  track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).

  `undefined` if unknown or if there's no language involved.

- `assocLanguage` (`string | undefined`): A secondary language associated to the
  audio track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).
  Such language is often used in a different role than the language specified
  through the `language` property (e.g., written versus spoken, or a fallback
  dialect).

  `undefined` if unknown or if there's no language involved.

- `name` (`string | undefined`): Human-readable description of the audio track,
  or `undefined` when absent.
  If the `language` property is set, it should generally be in that language.

- `channels` (`number | undefined`): If set, it is the count of audio channels,
  indicating the maximum number of independent and simultaneous audio channels
  present in any media data in that audio track.

  For example, an AC-3 5.1 Rendition would have a `channels` attribute set to `6`.

- `characteristics` (`Array.<string> | undefined`): Semantic characteristics
  linked to that audio track, generally expressed as Uniform Type Identifiers
  such as accessibility-related metadata or commentary flags.

- `bitDepth` (`number | undefined`): If set, the bit depth in bits of the audio
  samples for every rendition grouped into that audio track.

- `sampleRate` (`number | undefined`): If set, the sample rate in hertz of the
  audio samples for every rendition grouped into that audio track.

- `bitDepths` (`Array.<number> | undefined`): Distinct bit depths in bits found
  among the renditions grouped into that audio track.

- `sampleRates` (`Array.<number> | undefined`): Distinct sample rates in hertz
  found among the renditions grouped into that audio track.

The current audio track should be known once the `audioTrackUpdate`
[event](../Player_Events.md) is sent for the currently-loaded content, which
should happen at least once before the content is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) (and thus before playback starts).

## Syntax

```js
const currentAudioTrack = player.getCurrentAudioTrack();
```

- **return value**:

`Object`: Characteristics of the currently loaded audio track (see previous
chapter). `undefined` if no content is loaded, if there's no audio tracks in
the loaded content or if its characteristics are unknown.
