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

## Architecture

The architecture of the project is as follow:

```
      +------------------------------------------------------------------------------+
      |                                                                              |
      |                                   JS API                                     |
      |                                                                              |
      +------------------------------------------------------------------------------+
  ^                    |   ^                                  ^
  | Main thread        |   |                                  |
-----------------------|---|----------------------------------|-----------------------
  | Web Worker         |   |                                  |
  V                    V   |                                  |
      +-----------------------------------+   +--------------------------------+
      |                                   |   |                                |
      |          MessageReceiver          |   |           TS bindings          |
      |                                   |   |                                |
      +-----------------------------------+   +--------------------------------+
                        |                       |                  ^
  ^ JavaScript          |                       |                  |
  | (once compiled)     |                       |                  |
------------------------|-----------------------|------------------|-------------------
  |  WebAssembly        |                       |                  |
  V  (once compiled)    |                       |                  |
                        V                       V                  |
+-------------------------------------------------+      +-------------------+
|                   Dispatcher                    | ---> |    Rs Bindings    |
+-------------------------------------------------+      +-------------------+
    |            |         |                   | |                ^
    |            |         |                   | |                |
    |      +-----|---------|----------+------+-|-|----------------+
    |      |     |         |          |      | | |                |
    |      |     |         |          |      | | |                |
    V      |     |         V          |      | | V                |
+-----------+    |   +---------------------+ | | +---------------------------+    | M
| Requester |    |   | NextSegmentSelector | | | |  AdaptiveQualitySelector  |    | o
+-----------+    |   +---------------------+ | | +---------------------------+    | d
                 V                           | V                                  | u
     +---------------+                       | +-----------------------+          | l
     | PlaylistStore |-----------------------+-| MediaElementReference |          | e
     +---------------+                         +---------------------- +          | s
                                                                                  |
                       (and other modules...)                                     |
                                                                                  |
```

Here's a definition of terms and blocks in this schema, from left to right and
top to bottom:

- **JS API**: Implement the library's API, callable from JS applications.

  The JS API is defined in the `./ts-main/api.ts` file.

- **Main thread**: On top of the corresponding line is the code running in the
  main thread, alongside your application.

  It hosts the API as well as everything that needs to interact with the page,
  such as handling the video element.

  The corresponding code runs in the `./ts-main/` directory.

- **Web Worker**: Below the corresponding line is the code running in a single
  Web Worker. In it runs the main logic of the application.

  That way, actions such as heavy UI interactions will only have a minimal
  impact on media buffering, reducing the risk of rebuffering and improving
  the user experience.

  The corresponding code runs in:

  - the `./ts-worker/` directory for the TypeScript part of the code
    (compiled into JavaScript once the library is built).
  - the `./rs-core/` directory for the Rust part of the code (compiled
    into WebAssembly once the library is built).

- **MessageReceiver**: Entry point of the Web Worker, to which the API post
  messages to.

  The MessageReceiver is defined in the `./ts-worker/MessageReceiver.ts`
  file.

- **TS bindings**: Provide web APIs to the WebAssembly part, for example media
  buffering APIs. Also call event listeners on the Rust-side on various
  events.

  Some Web API can only be called on the main thread, which is why the TS
  bindings sometimes need to post messages back to the API.

  The TS bindings are defined in the `./ts-worker/bindings.ts` file.

- **Javascript (once compiled)**: higher than the corresponding line is the
  code written in TypeScript, which is then compiled to JavaScript.

  That code is present in the following directories:

  - `./ts-main/` for the code running in the main thread, such as the
    API
  - `./ts-worker/` for the code running on the WebWorker

- **WebAssembly (once compiled)**: Lower than the corresponding line is the
  of code written in Rust - that will be compiled to WebAssembly - present in
  the `./rs-core` directory.

- **Dispatcher**: Entry point of the Rust logic. Receive orders and/or events,
  and call the right modules in the right order.

  The Dispatcher is defined in the `./rs-core/dispatcher/` directory.

- **Rs bindings**: Define JavaScript functions exposed by TS bindings.

  Rs bindings are defined in the `./rs-core/bindings/` directory.

- **Requester**: Schedule playlist and segment requests.

  The Requester is defined in the `./rs-core/requester/` directory.

- **NextSegmentSelector**: Keep track of the next segments that should be
  requested

  The NextSegmentSelector is defined in the `./rs-core/segment_selector/`
  directory.

- **AdaptiveQualitySelector**: Implement Adaptive BitRate (a.k.a. ABR)
  management, such as calculating the network bandwidth, to be able to
  choose the best variant and media selected.

  The AdaptiveQualitySelector is defined in the `./rs-core/adaptive/`
  directory.

- **PlaylistStore**: Parses and stores the metadata of the current content as
  well as keep tracks of the current variant and media playlists selected.

  The PlaylistStore is defined in the `./rs-core/playlist_store/`
  directory.

- **MediaElementReference**: Interface to interact with the media element in
  the web page, as well as to buffer media.

  The MediaElementReference is defined in the `./rs-core/media_element/`
  directory.

- **Modules**: Specialized blocks of the Rust logic doing specific tasks,
  potentially calling Rs bindings when wanting to call web API.
