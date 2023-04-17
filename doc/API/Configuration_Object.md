# Player Configuration

## Overview

On [`WaspHlsPlayer` instantiation](./Instantiation.md), a configuration object
is created and associated to this instance.

It allows to configure many aspects of the player, from how much buffer you
would prefer to build to how many request retry should be performed if some
fails, and any of it can be updated at any time, even when a content impacted
by it is playing.

An initial config can optionally be given on instantiation (as indicated in the
corresponding API documentation page). If not set, a sane default configuration
will be generated instead by the `WaspHlsPlayer`.

Any property of that configuration may also be updated at any time through the
[`updateConfig`](./Basic_Methods/updateConfig.md) call.

For example, to update the `bufferGoal` property to `20` seconds, you can write:

```js
player.updateConfig({
  bufferGoal: 20,
});
```

Note that because most of that configuration's values are actually relied on by
code running in a worker, there's a necessary delay before it actually becomes
considered (it doesn't happen synchronously).

The last version of that configuration can also be recuperated through the
[`getConfig`](./Basic_Methods/getConfig.md) method.

The rest of this page is going to focus on each properties declared on this
configuration object.

## `bufferGoal`

_type: `number`_

Amount of buffer, in seconds, to "build" ahead of the currently wated position.

Once that amount is reached, we'll stop loading new data until we go under
again.

A lower value will mean less memory and network resources are generally taken,
but more risk of rebuffering. A higher value will mean the opposite.

A value in the `15`-`50` range is generally safe.

## `segmentMaxRetry`

_type: `number`_

Amount of times a failed segment request might be retried on errors that seem
temporary (such as an HTTP 404 for example):

- `1` meaning it will be retried once
- `2` twice
- `0` never retried etc.

To set to `-1` for infinite retry.

Do not be scared to put a high value if you would prefer to retry over stopping
on error. There's an "exponential backoff" mechanism whose main objective is to
avoid overloading servers and network resources. So retry never will happen in a
quick loop.

## `segmentRequestTimeout`

_type: `number`_

Number of milliseconds after which a segment request with no response will
be automatically cancelled due to a "timeout".

Depending on the configuration, the segment request might then be retried.

To set to `-1` for disabling segment request timeouts.

## `segmentBackoffBase`

_type: `number`_

If a segment request has to be retried, we will wait an amount of time
before restarting the request. That delay raises if the same segment
request fails multiple consecutive times, starting from around this value
in milliseconds to `segmentBackoffMax` milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.

## `segmentBackoffMax`

_type: `number`_

If a segment request has to be retried, we will wait an amount of time
before restarting the request. That delay raises if the same segment
request fails multiple consecutive times, starting from around
`segmentBackoffBase` milliseconds to this value in milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.

## `multiVariantPlaylistMaxRetry`

_type: `number`_

Amount of times a failed Multivariant Playlist request might be retried on
errors that seem temporary (such as an HTTP 404 for example):

- `1` meaning it will be retried once
- `2` twice
- `0` never retried etc.

To set to `-1` for infinite retry.

Do not be scared to put a high value if you would prefer to retry over stopping
on error. There's an "exponential backoff" mechanism whose main objective is to
avoid overloading servers and network resources. So retry never will happen in a
quick loop.

## `multiVariantPlaylistRequestTimeout`

_type: `number`_

Number of milliseconds after which a Multivariant Playlist request with no
response will be automatically cancelled due to a "timeout".

Depending on the configuration, the Multivariant Playlist request might then be
retried.

To set to `-1` for disabling Multivariant Playlist request timeouts.

## `multiVariantPlaylistBackoffBase`

_type: `number`_

If a Multivariant Playlist request has to be retried, we will wait an
amount of time before restarting the request. That delay raises if the same
request fails multiple consecutive times, starting from around this value
in milliseconds to `multiVariantPlaylistBackoffMax` milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.

## `multiVariantPlaylistBackoffMax`

_type: `number`_

If a Multivariant Playlist request has to be retried, we will wait an
amount of time before restarting the request. That delay raises if the
same request fails multiple consecutive times, starting from around
`multiVariantPlaylistBackoffBase` milliseconds to this value in milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.

## `mediaPlaylistMaxRetry`

_type: `number`_

Amount of times a failed Media Playlist request might be retried on
errors that seem temporary (such as an HTTP 404 for example):

- `1` meaning it will be retried once
- `2` twice
- `0` never retried etc.

To set to `-1` for infinite retry.

Do not be scared to put a high value if you would prefer to retry over stopping
on error. There's an "exponential backoff" mechanism whose main objective is to
avoid overloading servers and network resources. So retry never will happen in a
quick loop.

## `mediaPlaylistRequestTimeout`

_type: `number`_

Number of milliseconds after which a Media Playlist request with no
response will be automatically cancelled due to a "timeout".

Depending on the configuration, the Media Playlist request might then be
retried.

To set to `-1` for disabling Media Playlist request timeouts.

## `mediaPlaylistBackoffBase`

_type: `number`_

If a Media Playlist request has to be retried, we will wait an
amount of time before restarting the request. That delay raises if the same
request fails multiple consecutive times, starting from around this value
in milliseconds to `mediaPlaylistBackoffMax` milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.

## `mediaPlaylistBackoffMax`

_type: `number`_

If a Media Playlist request has to be retried, we will wait an
amount of time before restarting the request. That delay raises if the
same request fails multiple consecutive times, starting from around
`mediaPlaylistBackoffBase` milliseconds to this value in milliseconds.

The step at which it raises is not configurable here, but can be resumed
as a power of 2 raise on the previous value each time.
