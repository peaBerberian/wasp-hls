# Worker code

This directory is where we run the JavaScript part of the player in a Web Worker.
It receives messages from the main thread, runs the Rust core through WebAssembly
and provides the JavaScript operations that core needs.

`index.ts` starts `MessageReceiver.ts`, which initializes WebAssembly and handles
incoming commands, playback observations and operation results.

Its job is to:

- Call the Rust core with commands and events, then perform the JavaScript
  actions it asks for: requests, timers, media buffer operations etc.

- Keep downloaded data in JavaScript memory behind resource identifiers. The
  core can inspect or push a resource without copying all its bytes into
  WebAssembly memory.

- Inspect segment metadata and timing, and prepare media data for buffering,
  including transmuxing when needed.

- Create and use media buffers in the worker when supported. Otherwise, send
  the operations to the main thread and communicate their results back to the
  core.

- Send content information, selection changes, errors and playback actions to
  the main thread.

`bindings.ts` implements the functions called by the Rust core. `globals.ts`
keeps the worker's player instance, current content, resources, pending requests
and codec support results. There is one player instance per worker.

`transmux.ts` handles probe transmuxer creation and media type conversion.
Playback transmuxer state belongs to Rust's SourceBuffer.
`isobmff-utils.ts` reads codec and timing information from MP4 data.
`postMessage.ts` sends messages to the main thread, and `utils.ts` contains
small helpers.

## Where it stops

This code owns the JavaScript state and resources kept in the worker. The Rust
core decides what to request and how playback should progress; this code performs
the requested actions and reports what happened.

It cannot access the page's media element directly. Those interactions go through
messages to the main thread, even when media buffers are managed in the worker.
Requests, timers and buffer operations may finish later, so their callbacks need
to refer to the right content and media source.
