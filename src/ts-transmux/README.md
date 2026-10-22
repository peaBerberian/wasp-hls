# Transmuxing segments

This directory contains the code converting MPEG-TS or ADTS AAC segments into
fragmented MP4 that can be pushed to media buffers. The entry point is
`Transmuxer` in `src/Transmuxer.ts`, exported through `index.ts`.

This code was initially forked from mux.js, then adapted to TypeScript and a
procedural approach. Its license is kept in `LICENSE`.

Playback now uses `src/rs-core/transmux`. This version remains a reference for
correctness tests and the opt-in transmux benchmark.

Its job is to:

- Read transport packets and extract audio/video data, or read AAC frames
  directly for AAC segments.

- Parse H.264 video and AAC audio information needed to build MP4 tracks and
  samples.

- Handle timestamps, including rollover and timing continuity between segments.

- Generate MP4 initialization and media data, and return segment timing
  information when available.

`Transmuxer` keeps the parsers, track information and timing state used between
calls. The caller can provide a decode-time starting value and ask to reset
state when the next segment should be handled separately from previous ones.

Most of the code is in `src/`, split between packet and frame parsers, timestamp
helpers and MP4 generators. `FullMp4SegmentConstructor.ts` puts the generated
audio and video data together. Track generators keep payload references and
small `moof` headers; `mp4-fragment.ts` writes the init segment, headers and
audio/video payloads into one final media-sized buffer. Payloads are borrowed
only until synchronous assembly finishes; the returned bytes own their storage.

## Where it stops

This code takes segment bytes and returns converted bytes, or no output if it
couldn't build a segment. It processes whole segments synchronously. It doesn't
fetch data, read playlists, choose what to play or push the result into browser
buffers.

Transmuxing changes the container, not the audio or video codec. It doesn't
decode or re-encode media, and isn't a general converter for every format or
codec. The caller has to manage the transmuxer instance and tell it when its
saved state should be reset.
