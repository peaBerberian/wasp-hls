# Requesting resources

This directory is where we manage playlist and segment requests. The main entry
point is `Requester` in `mod.rs`.

Its job is to:

- Start requests for playlists, initialization segments and media segments,
  including when only a byte range of the resource is wanted.

- Keep track of pending requests and queued segments, with enough metadata for
  the caller to recover what a completed request was for. Segment requests carry
  an opaque caller-provided identifier and an audio, video or probe lane tag.

- Prioritize media segment requests according to their distance from a
  caller-provided base position, normally the wanted playback position. Segments
  further ahead may wait for more urgent requests to finish. A lock lets the
  caller queue several candidates before letting that scheduling happen.

- Retry eligible failures, when allowed by the caller and configuration, after
  an exponential backoff with some randomness. It also handles aborting requests
  and dropping queued work when it is no longer wanted.

Playlist requests and explicit immediate segment requests start directly, without
going through the media segment waiting queue. The scheduling here decides when
to start queued work; it doesn't interrupt a running request just because a more
urgent one comes along.

One distinction to keep in mind: a pending request isn't necessarily downloading.
Requests waiting for a retry timer stay registered as pending, separately from
segments waiting for their priority to allow them to start.

`configuration.rs` defines the timeouts, retry limits and backoff settings for
the different resource types.

## Where it stops

This code owns request metadata, the segment waiting queue, retry timer mappings
and its configuration. The actual requests and timers run through JavaScript
calls. The caller has to communicate request successes, failures and elapsed
timers back here so that work can progress.

It doesn't choose which playlists or segments are needed, schedule periodic
playlist refreshes, parse downloaded data or push it into media buffers. It
reports completion metadata and retry/failure outcomes for the caller to act on.
