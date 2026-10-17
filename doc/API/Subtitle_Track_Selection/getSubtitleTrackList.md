# `getSubtitleTrackList` method

## Description

Returns the available subtitle tracks for the loaded content, or an empty array
when no tracks are known. Each track corresponds to an HLS subtitle rendition.

Each object contains:

- `id` (`number`): Track identifier, scoped to the loaded content.
- `name` (`string`): Human-readable description.
- `groupId` (`string`): HLS rendition group identifier.
- `language` (`string | undefined`): Primary language tag.
- `assocLanguage` (`string | undefined`): Associated language tag.
- `default` (`boolean`): Whether the content marks this rendition as default.
- `autoselect` (`boolean`): Whether automatic selection is allowed.
- `forced` (`boolean`): Whether this rendition contains forced subtitles.
- `characteristics` (`Array.<string>`): Semantic characteristics, including
  accessibility roles.

The `subtitleTrackListUpdate` [event](../Player_Events.md) reports list changes.

## Syntax

```js
const subtitleTracks = player.getSubtitleTrackList();
```

- **return value**: `Array.<Object>` containing the available tracks.
