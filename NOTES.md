# Notes

## Ideas

- Add an `initialSpeed` option to the `load` method

- Either actually change audio track list when a new variant we changed to is
  associated to more or less audio tracks than the one before it or update the
  API documentation to indicate that this is not the case.

  When I think about it we might never want to change the audio track list,
  though a track change will lead to an incompatible "locked" variant being
  unlocked. Update documentation to reflect this.

- Either actually change variant list when setting a new audio track with more
  or less variants associated to it than the one before it or update the
  API documentation to indicate that this is not the case.

  When I think about it we might never want to change the variant list,
  though a locked variant will lead to an incompatible set audio track being
  changed. Update documentation to reflect this.

- Add `isLive` method for when playing a live contents (i.e. no `END-LIST` in
  playlist and not a `VOD` or `EVENT` PLAYLIST-TYPE)

- Document `isLive` in the API documentation.

- Add `isVod` method for when playing a VOD contents (i.e a `VOD`
  `PLAYLIST-TYPE` in the playlist)

- Document `isVod` in the API documentation.

- Add `getMediaDuration` method, returning the HTMLMediaElement's `duration` in
  playlist time in seconds.

- Use `getMediaDuration` in the demo and document it in the API documentation.

- Add `getCurrentBufferGap` method, returning the remaining amount of buffer
  ahead of the current position.

- Use `getCurrentBufferGap` in the demo and document it in the API
  documentation.

- add `playlistInfoUpdate` event for when either `getMinimumPosition`,
  `getMaximumPosition`, `isLive` or `isVod` might be updated.