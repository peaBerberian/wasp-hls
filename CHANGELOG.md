# Changelog

## Unreleased

### Features

- Add buffer-based adaptive algorithm (BOLA)

### Bug fixes

- Actually send `audioTrackUpdate` if the audio track switched from one audio track to another (or none) on a `lockVariant` call
- Don't needlessly send `audioTrackUpdate` if the audio track did not change on a `lockVariant` call

### Other

- Make `bufferGoal` `30` by default.

## v0.5.0 (2025-06-01)

### Changes

- The too specific `MultivariantPlaylistMediaTagMissingType`, `MultivariantPlaylistMediaTagMissingName` and `MultivariantPlaylistMediaTagMissingGroupId` error codes have been collapsed into a new `MultivariantPlaylistMissingRequiredAttribute` error code
- A top-level resource missing `#EXTM3U` now surfaces as a `WaspMultivariantPlaylistParsingError` with the `NotAPlaylist` error code; the dead `MultivariantPlaylistMissingExtM3uHeader` public error code has been removed.

### Features

- Add `initialAudioTrack` option to `loadContent`
- It is now possible to play media playlists directly instead of always going through a multivariant playlist URL
- It is now possible to play Multivariant Playlists that do not announce their variants' `CODECS` attributes
- Add optional `bitDepth` and `sampleRate` properties to `getAudioTrackList`, `getCurrentAudioTrack`methods plus `audioTrackUpdate` and `audioTrackListUpdate` events reflecting respectively the bit depth of audio samples and the audio audio sample rate for the corresponding audio tracks if they're invariant for all renditions of that track.
- Add optional `bitDephs` and `sampleRates` array properties to `getAudioTrackList`, `getCurrentAudioTrack`methods plus `audioTrackUpdate` and `audioTrackListUpdate` events listing respectively the known various bit depts and rates of audio samples in all renditions of that track.
- Add `characteristics` property to `getAudioTrackList`, `getCurrentAudioTrack`methods plus `audioTrackUpdate` and `audioTrackListUpdate` events reflecting the `CHARACTERISTICS` of the original playlist, e.g. for audio-description audio tracks.
- Add optional `videoRange` property to `getVariantList`, `getLockedVariant`, `getCurrentVariant` as well as `variantUpdate`, `variantLockUpdate` and `variantListUpdate` events to reflect its video's dynamic range
- Add `NotAPlaylist` error code for when the given resource URL does not seem to be a valid HLS playlist URL
- Add `WaspContentCompatibilityError` with the `NoSupportedVariant` error code for contents with no playable variant on the current platform
- Add `MultivariantPlaylistVariableDefinitionError` and `MediaPlaylistVariableDefinitionError` error codes for when an `#EXT-X-DEFINE` tag or usage is not compliant to the HLS specification respectively on the MultiVariant Playlist or the Media Playlist.
- Add `MultivariantPlaylistDuplicateTag`, `MultivariantPlaylistConflictingTagTypes`, `MediaPlaylistDuplicateTag`, and `MediaPlaylistConflictingTagTypes` error codes for duplicate singleton tags and invalid mixes of playlist tag classes.
- Add `SourceBufferEmptyMimeType`, `SourceBufferMediaSourceIsClosed`, `SourceBufferNoMediaSourceAttached` and `SourceBufferQuotaExceededError` error codes on a `WaspSourceBufferCreationError` error type.
- Add `getSeekableMinimumPosition` and `getSeekableMaximumPosition` methods to expose the currently known seekable bounds in playlist time
- Add `usesProgramDateTime` and `positionToDate` methods to let applications detect and convert `EXT-X-PROGRAM-DATE-TIME`-based playlist timelines
- Light handling of QuotaExceededError by removing data from buffers if/when it happens. More advanced handling would be to reduce buffer goal adaptively (not done yet)
- Add parsing of `#EXT-X-DEFINE` tags
- Handle `#EXT-X-MEDIA-SEQUENCE` tags and use it both for segment staleness detection and contiguity checks
- Handle `#EXT-X-DISCONTINUITY` and `#EXT-X-DISCONTINUITY-SEQUENCE` tags by resetting transmuxer state on discontinuities

### Bug fixes

- `SourceBufferAlreadyCreatedWithSameType` mistakenly also regrouped other kinds of SourceBuffer-related bug than what's documented. This is now fixed.
- Better handle edge speed settings: negative speeds, non-finite speeds, `NaN` speed
- Better handling of gap jumping, previous behavior could skip legitimate data
- `getMinimumPosition` and `getMaximumPosition` now progress linearly between playlist updates when it makes sense, while `EVENT` playlists only do so for the maximum position
- Don't advertise audio tracks which are not selectable

### Other

- Remove `wasm-bindgen` dependency for Rust<->JS bindings, to better control base browser/ES version support

## 0.4.2 (2023-04-27)

- Better detect mimetype from segment's extension by stripping query and fragment components from its URL
- Relative Playlist URLs starting with a "/" now are relative from the Playlist's domain, not its path
- Fix issue in GOP (group of pictures) creation code in the mpeg-ts to fmp4 transmuxer. The real impact on playback in unclear (none was noticed).

## 0.4.1 (2023-04-22)

### Bug fixes

- Fix HTTP Range requests (by prepending the forgotten `bytes=` string)

## 0.4.0 (2023-04-21)

(First public release)

### Features

- Add TypeScript declaration files for embedded wasm and worker files

## 0.3.0 (2023-04-21)

### Features

- Add embedded wasm and worker

## 0.2.0 (2023-04-21)

### Features

- Emit TypeScript declaration files and add more types

## 0.1.1 (2023-04-21)

### Bug fixes

- Fix-up export paths in the package published on npm

## 0.1.0 (2023-04-21)

Initial release
