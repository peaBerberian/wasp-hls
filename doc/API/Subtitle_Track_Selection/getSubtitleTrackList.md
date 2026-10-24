# `getSubtitleTrackList` method

## Description

Returns the available subtitle tracks for the loaded content, or an empty array
when no tracks are known. Each track groups HLS subtitle renditions with the same
characteristics.

Each object contains:

- `id` (`number`): Track identifier, scoped to the loaded content.
- `name` (`string | undefined`): Human-readable description, or `undefined`
  when absent.
- `language` (`string | undefined`): Primary language tag.
- `assocLanguage` (`string | undefined`): Associated language tag.
- `forced` (`boolean`): Whether this rendition contains forced subtitles.
- `characteristics` (`Array.<string>`): Semantic characteristics, including
  accessibility roles.

The `subtitleTrackListUpdate` [event](../Player_Events.md) reports list changes.

## Syntax

```js
const subtitleTracks = player.getSubtitleTrackList();
```

- **return value**: `Array.<Object>` containing the available tracks.
