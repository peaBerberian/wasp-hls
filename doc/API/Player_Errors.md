# Player Errors

## Overview

Player errors are `Error` objects which are sent an issue linked to the playback
of the last-[loaded](./Loading_a_content.md) content arised.

All errors in the `WaspHlsPlayer` follow a common format, with a `name`,
`message`, `code` and `globalCode` property. Some more specific errors have more
descriptive properties.

All of those are defined in this documentation page.

## Fatal errors and warnings

There is two distincts types of errors:

- "Fatal" errors, which are major errors led to playback interruption.

  Such errors are sent through a `"error"` [event](./Player_Events.md) and are
  then returned by the [`getError`](./Basic_Methods/getError.md) method once
  the `"error"` event has been triggered for the current content.

- warnings, which are minor errors, sent through the `"warning"`
  [event](./Player_Events.md).

Thus you can know if you received a fatal error (which thus interrupted
playback) or a warning (for which the `WaspHlsPlayer` is capable of staying
resilient to) based on the event and/or API which sent it.

## Common structure of a `WaspHlsPlayer`'s error

All fatal errors and warnings follow the same structure. They extend the
JavaScript Error Object and add multiple properties giving more indication over
the issue encountered.

All the following properties are common to all errors (sent through `"warning"` and
`"error"` events, as well as returned by the `getError` method):

- `name` (`string`): The type of error received. The `name` property allows
  you to detect XXX TDOO

- `message` (`string`): A human-readable description of the Error,
  This `message` property might change from version to version.

- `code` (`string`): An error code identifying the exact error encountered.
  Unlike `message`, the `code` property is intented to be stable and as such
  may be used programatically for error detection.

  The code should be sufficient to identify a particular error in your
  application. The `name` property only allowing to identify the particular
  type of Error instance you're currently handling if you want to check the
  error Object's properties. More details on that below.

- `globalCode` (`string`): Contains the exact same value as the `code`
  property.

  We provide both for technical TypeScript-related reasons. `globalCode` is
  typed as an union of all potential code strings that may be returned by the
  `WaspHlsPlayer` in general, whereas `code` is typed as an union of code
  strings that may be linked to error with the same `name`.

  Both are correct in terms of type-checking, `code` being typed as a still
  valid sub-set of the `globalCode` property. The main difference will be when
  you rely on TypeScript to handle that error: In code handling all error
  objects without first checking the `name` property, TypeScript will give
  better autocompletion results when using `globalCode`. However if you
  filtered by `name` in your code, then `code` will be the most precize one.

## Error type: `WaspMultivariantPlaylistRequestError`

`WaspMultivariantPlaylistRequestError` are errors triggered when an error arised
while doing the Multivariant Playlist HTTP(S) request.

Its `name` property is set to `"WaspMultivariantPlaylistRequestError"`. For
example to catch when a fatal error was due to a failure to request the
Multivariant Playlist, you can write:

```js
player.addEventlistener("error", (error) => {
  if (error.name === "WaspMultivariantPlaylistRequestError") {
    // This error was due to a request failure of the Multivariant Playlist");
  }
});
```

`WaspMultivariantPlaylistRequestError` also may have the following property:

- `status` (`number|undefined`: The failed request's HTTP(S) response status.

  `undefined` if unknown, not checked or if not yet received (e.g. in case of a
  timeout).

### Error codes

A `WaspMultivariantPlaylistRequestError`'s `code` property can be set to any
of the following values:

- `"MultivariantPlaylistBadHttpStatus"`:
  The HTTP(s) status on the response indicated that the Multivariant Playlist
  could not be fetched.

- `"MultivariantPlaylistRequestTimeout"`:
  The HTTP(s) request for the Multivariant Playlist timeouted according to
  the current configuration.

- `"MultivariantPlaylistRequestError"`:
  The HTTP(s) request itself failed to be performed (might be because we're
  offline, might be because of security policies etc.) for the Multivariant
  Playlist.

- `"MultivariantPlaylistRequestOtherError"`:
  The HTTP(s) request itself failed to be performed for another, unknown,
  reason for the Multivariant Playlist.

## Error type: `WaspMultivariantPlaylistParsingError`

## Error type: `WaspMediaPlaylistRequestError`

`WaspMediaPlaylistRequestError` are errors triggered when an error arised
while doing a Media Playlist HTTP(S) request.

Its `name` property is set to `"WaspMediaPlaylistRequestError"`. For
example to catch when a fatal error was due to a failure to request a
Media Playlist, you can write:

```js
player.addEventlistener("error", (error) => {
  if (error.name === "WaspMediaPlaylistRequestError") {
    // This error was due to a request failure of a Media Playlist");
  }
});
```

`WaspMediaPlaylistRequestError` also may have the following property:

- `status` (`number|undefined`: The failed request's HTTP(S) response status.

  `undefined` if unknown, not checked or if not yet received (e.g. in case of a
  timeout).

- `mediaType` (`string`): The "media type" associated to that Media Playlist.
  Examples of media types are `"Audio"` for a Media Playlist linked to
  resources containing just audio media content, and `"Video"` for a Media
  Playlist which contains video content (it may be only video, video with audio,
  with captions etc.).

### Error codes

A `WaspMediaPlaylistRequestError`'s `code` property can be set to any
of the following values:

- `"MediaPlaylistBadHttpStatus"`:
  The HTTP(s) status on the response indicated that the Media Playlist
  could not be fetched.

- `"MediaPlaylistRequestTimeout"`:
  The HTTP(s) request for the Media Playlist timeouted according to
  the current configuration.

- `"MediaPlaylistRequestError"`:
  The HTTP(s) request itself failed to be performed (might be because we're
  offline, might be because of security policies etc.) for the Media
  Playlist.

- `"MediaPlaylistRequestOtherError"`:
  The HTTP(s) request itself failed to be performed for another, unknown,
  reason for the Media Playlist.

## Error type: `WaspMediaPlaylistParsingError`

## Error type: `WaspSegmentRequestError`

`WaspSegmentRequestError` are errors triggered when an error arised
while doing a segment HTTP(S) request.

Its `name` property is set to `"WaspSegmentRequestError"`. For
example to catch when a fatal error was due to a failure to request a
segment, you can write:

```js
player.addEventlistener("error", (error) => {
  if (error.name === "WaspSegmentRequestError") {
    // This error was due to a request failure of a segment");
  }
});
```

`WaspSegmentRequestError` also may have the following property:

- `isInit` (`boolean|undefined`): If `true`, the error concerns an
  initialization segment (a segment without media data, intented for lower-level
  media decoders initialization).

  If `false`, it concerns a media segment (a segment with media data).
   
  If `undefined` it is not known whether it concerns an initialization or
  media segment.

### Error codes

A `WaspSegmentRequestError`'s `code` property can be set to any
of the following values:

- `"SegmentBadHttpStatus"`:
  The HTTP(s) status on the response indicated that the segment could not be
  fetched.

- `"SegmentRequestTimeout"`:
  The HTTP(s) request for the segment timeouted according to the current
  configuration.

- `"SegmentRequestError"`:
  The HTTP(s) request itself failed to be performed (might be because we're
  offline, might be because of security policies etc.) for the Media
  Playlist.

- `"SegmentRequestOtherError"`:
  The HTTP(s) request itself failed to be performed for another, unknown,
  reason for the segment.

## Error type: `WaspSegmentParsingError`

## Error type: `WaspSourceBufferCreationError`

## Error type: `WaspSourceBufferError`

## Error type: `WaspOtherError`

XXX TODO
