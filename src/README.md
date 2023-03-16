# wasp-hls source code

This directory is the home of the source code of the wasp-hls library.

The code is splitted into several directories:

- **rs-core**: The player core logic, written in Rust.

  This is the part deciding what to request and when, reacting to events
  (media-originated, user-originated, request-originated etc.), supervising
  playback and so on.

- **ts-main** The player's API, usable directly from an user interface
  application. It is written in TypeScript.

  This directory also contains logic that can only run in the main thread,
  like code about the page's HTMLMediaElement, as well as code to
  communicate with the worker part of the player.

- **ts-worker**: The player logic running in a WebWorker, written in
  TypeScript.

  It interacts both with **ts-main** and **rs-core**, allowing to bind
  application-facing to the resulting core logic and allowing the core logic
  to call API only normally exposed through JavaScript (like the MSE API,
  used for media buffering).

- **ts-common**: Code that may be used both by **ts-main** and/or by
  **ts-worker**. Those are either util functions or logic that may run in
  either the main thread or the worker based on the browser capabilities.
