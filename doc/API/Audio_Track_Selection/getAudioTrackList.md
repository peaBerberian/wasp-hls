# `getAudioTrackList` method

## Description

Returns the list of available "audio tracks" for the currently loaded content.

Audio tracks group one or more renditions sharing the same characteristics:
language, accessibility role, etc. Multiple renditions may exist under one track
when, for example, several audio quality levels are available.

These correspond to distinctly selectable tracks, not just to the presence of
audio samples in the content: muxed media playlists containing video and audio
combined will generally not be exposed as a separate audio track.

This method will return an array of objects, each object containing the
information available for a particular audio track.

Each of those objects should contain the following keys (same than for the
`audioTrackListUpdate` event):

- `id` (`number`): The identifier for that audio track. It is generally useful
  to for example set the audio track though a [`setAudioTrack`](./setAudioTrack.md)
  call.

- `language` (`string | undefined`): The primary language used in this audio
  track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).

  `undefined` if unknown or if there's no language involved.

- `assocLanguage` (`string | undefined`): A secondary language associated with
  the audio track, as a [language tag](https://datatracker.ietf.org/doc/html/rfc5646).
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

- `characteristics` (`Array.<string>`): Characteristics of the audio track,
  such as `public.accessibility.describes-video` for audio description.

  An empty array if no such characteristics are announced.

- `bitDepth` (`number | undefined`): If set, the bit depth in bits of the audio
  samples for every rendition grouped into that audio track.

  `undefined` if unknown or if grouped renditions do not all share the same bit
  depth.

- `sampleRate` (`number | undefined`): If set, the sample rate in hertz of the
  audio samples for every rendition grouped into that audio track.

  `undefined` if unknown or if grouped renditions do not all share the same
  sample rate.

- `bitDepths` (`Array.<number> | undefined`): Distinct bit depths in bits found
  among the renditions grouped into that audio track.

- `sampleRates` (`Array.<number> | undefined`): Distinct sample rates in hertz
  found among the renditions grouped into that audio track.

That list of audio tracks is known once the `audioTrackListUpdate`
[event](../Player_Events.md) is sent for the currently-loaded content, which
should happen at least once before the content is in the `"Loaded"`
[state](../Basic_Methods/getPlayerState.md) (and thus before playback starts).

If no content is currently loaded or if there is but audio tracks
characteristics are either inexistant or unknown, this method will return an
empty array (`[]`).

## Syntax

```js
const audioTracks = player.getAudioTrackList();
```

- **return value**:

`Array.<Object>`: Characteristics of the currently available audio tracks (see
previous chapter). Empty if no content is loaded, if there's no audio tracks in
the loaded content or if its characteristics are unknown.
