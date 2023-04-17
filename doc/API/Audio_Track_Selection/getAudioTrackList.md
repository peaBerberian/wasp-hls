# `getAudioTrackList` method

## Description

Returns the list of available "audio tracks" for the currently loaded content.

Audio tracks are one or multiple renditions (e.g. when there's multiple audio
qualities) associated to a given set of characteristics: a language,
accessibility concepts etc..

This method will returns an array of objects, each object containing the
information available for a particular audio track.

Each of those objects should contain the following keys (same than for the
`audioTrackListUpdate` event):

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

- `name` (`string`): Human-readable description of the audio track.
  If the `language` property is set, it should generally be in that language.

- `channels` (`number | undefined`): If set, it is the count of audio channels,
  indicating the maximum number of independent and simultaneous audio channels
  present in any media data in that audio track.

  For example, an AC-3 5.1 Rendition would have a CHANNELS="6" attribute.

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
