# Dispatcher

This directory regroups the code receiving commands and events, then calling the
right methods in the right order. The entry point is `Dispatcher` in `mod.rs`.
It owns the current content's state and the objects used to manage its playback.

Its job is to:

- Receive commands to load or stop a content, change tracks or variants and
  update playback settings.

- Handle the steps needed to start playback: loading playlist information,
  inspecting segments when needed, checking codec support and creating media
  buffers. Segments downloaded for inspection can be kept to push them later,
  avoiding another download.

- React to playback observations, completed requests, buffer updates and timers.
  Those events can lead to new segment requests, playlist refreshes, buffer
  pushes, seeks or changes in selection.

- Apply quality recommendations and user choices, check if pending requests are
  still needed, and keep audio and video choices compatible.

- Report content information, selection changes and errors, and clean up the
  current content when stopping.

`api/mod.rs` contains the methods called to control playback and their
WebAssembly exports.
`event_listeners/mod.rs` receives callbacks and defines the playback observation
and resource wrappers used here. `JsMemoryBlob` owns a handle to data kept in
JavaScript memory and frees that resource when dropped.

`core/mod.rs` contains the main logic, with the steps to start playback in
`core/startup.rs`. `playlist_refresh_manager.rs` keeps track of when to reload
the playlists currently needed and manages their timers.
`segment_request_contexts.rs` stores information about pending segment requests,
so we know what to do with them once they finish.

## Where it stops

This code decides what to do next and keeps the state needed between events.
Playlist parsing, bandwidth estimation and tracking buffered segments are left
to the objects it calls.

It needs commands and callbacks from JavaScript to keep going. Browser operations
and segment inspection happen through JavaScript calls, and their results may
come later.
