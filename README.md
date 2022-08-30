# Wasp-hls: A WebAssembly-based HLS Media Player

This repository is the home of the `Wasp-hls` media player, a
not-ready-at-all-yet personal project and proof of concept which tries to
implement a simple HLS (the adaptive streaming protocol) media player for
the web while relying the most possible on WebAssembly, by using the
[Rust](https://www.rust-lang.org/) language.


## Why?

I'm currently working as the lead developper of another, featureful adaptive
media player, the open-source [RxPlayer](https://github.com/canalplus/rx-player)
so this is not something totally out of the blue.

However even after stating that, `Wasp-hls` doesn't really answer any real
worthwile need I personally observed when working on the RxPlayer.

The real reasons why I started this project are more personal, mainly:

  - for fun

  - to see how IO-heavy logic (like we have here with many requests, media
    segments streaming, playback observation, network metrics etc.) using web
    APIs only exposed to JavaScript could be conjugated with WebAssembly and
    Rust.

  - to experiment with a big-enough WebAssembly-based library: how should it
    interact with applications written in JavaScript?

  - to learn more about the HLS streaming protocol, for which the RxPlayer does
    not provide first class support (yet?)

  - to see how I would write a complex beast like a Media player if restarting
    from scratch (though the situation is very different here, due to the
    difference in the language used).

  - to see if there's any real performance and/or memory related advantage (or
    disadvantage) in relying on WebAssembly for the core logic of a media
    player, in various situations (multiple players on the same page, large 4k
    segments, web workers, mse-in-worker).

  - to work on and improve my Rust skills

  - To try relying on [wasm_bindgen](https://github.com/rustwasm/wasm-bindgen)
    on a sufficiently complex library.

    We avoided using it in the RxPlayer - even if it also does contains Rust
    code compiled into WebAssembly - for multiple reasons: we wanted to
    understand and control exactly what happened at language boundaries due to
    very frequent FFI interactions, cases were relatively simple and we didn't
    really have and didn't want to include some complex webpack (or
    equivalent)-building process.

## What's left to do?

A lot:

Type of contents:
  - [x] Play HLS VoD contents
  - [x] Transcode mpeg-ts (thanks to mux.js for now)
  - [ ] Play HLS live contents _(high priority)_
  - [ ] Proper support of HLS low-latency contents

Adaptive BitRate:
  - [x] Choose variant based on throughtput-based estimates
  - [ ] Also choose variant based on buffer-based estimates

Request Scheduling:
  - [x] Lazy Media Playlist downloading
  - [x] Buffer goal implementation (as in: stop loading segment once enough to fill
    the buffer up to a certain point are loaded)
  - [x] Parallel audio and video segment loading
  - [ ] Synchronization between progress of audio and video segment requests (to
    e.g. stop doing audio segment requests when video ones become urgent).
  - [ ] Retry of failed requests with an exponential backoff

Buffers:
  - [x] End of stream support (as in: actually end when playback reached the end!)
  - [x] Multiple simultaneous type of buffers support (for now only audio and video)
  - [ ] Inventory storing which quality is where in the buffers, both for API reasons
    and for several optimizations (though quality identification seems more difficult
    to implement in HLS than in DASH due to the fact that HLS only link variants to
    bitrate, not the actual audio and video streams - but it should be doable).
  - [ ] Proper handling of `QuotaExceededError` after pushing segments (when low
    on memory)

Tracks:
  - [ ] Provide API to set an audio, video and/or text track
  - [ ] support of at least one text track format (didn't check which yet)
    _(low priority)_

Decryption:
  - [ ] Support content decryption _(very low priority)_

Miscellaneous:
  - [ ] Make usage of the upcoming MSE-in-worker API
  - [ ] Proper Error API (should be high priority but that does not look like
    fun for now!)
  - [ ] WebAssembly-based mpeg-ts transcoder (very low priority)
  - [ ] Delta playlist handling

## Architecture

The architecture of the project is as follow:
```
      +------------------------------------------------------------------------------+  |T
      |                                                                              |  |y
      |                                   JS API                                     |  |p
      |                                                                              |  |e
      +------------------------------------------------------------------------------+  |S
                         |                                                              |c
                         |                         +--------------------------------+   |r
                         |                         |                                |   |i
                         |                         |          TS bindings           |   |p
                         |                         |                                |   |t
                         |                         +--------------------------------+   |
                         |                                 ^                |
                         |                                 |                |
                         |                                 |                |
                         |                                 |                |
                         |                                 |                |
                         V                                 |                V
+-------------------------------------------------+      +-------------------+          |R
|                 Dispatcher                      |<-----|    Rs Bindings    |          |u
+-------------------------------------------------+      +-------------------+          |s
    |            |         |                   | |                ^                     |t
    |            |         |                   | |                |                     |
    |      +-----|---------|----------+------+-|-|----------------+                     |
    |      |     |         |          |      | | |                |                     |
    |      |     |         |          |      | | |                |                     |
    V      |     |         V          |      | | V                |                     |
+-----------+    |   +---------------------+ | | +---------------------------+    | M   |
| Requester |    |   | NextSegmentSelector | | | |  AdaptiveQualitySelector  |    | o   |
+-----------+    |   +---------------------+ | | +---------------------------+    | d   |
                 V                           | V                                  | u   |
    +----------------+                       | +------------------+               | l   |
    | ContentTracker |-----------------------+-| Other modules... |               | e   |
    +----------------+                         +----------------- +               | s   |
                                                                                  |     |
                                                                                  |     |
                                                                                  |     |
                                                                                        |
                                                                                        |
```

Here's a definition of terms and blocks in this schema:

  - **Typescript**: This upper part of the schema describes the Typescript
    code - that will be compiled to JavaScript - present in the `./ts`
    directory.

  - **JS API**: Implement the library's API, callable from JS applications. This
    is the only part visible to the outside.

  - **TS bindings**: Provide web APIs to the WebAssembly part, for example media
    buffering APIs. Also call event listeners on the Rust-side on various
    events.

  - **Rust**: This lower part of the schema describes the Rust code - that will
    be compiled to WebAssembly - present in the `./src` directory.

  - **Dispatcher**: Entry point of the Rust logic. Receive orders and/or events,
    and call the right modules in the right order.

    The Dispatcher is defined in the `./src/dispatcher` directory.

  - **Rs bindings**: Define both Typescript functions exposed by TS bindings but
    also "event listeners" (which are technically a part of the Dispatcher)
    which will be called by TS bindings on various events.

    Rs bindings are defined in the `./src/bindings` directory.

  - **Modules**: Specialized blocks of the Rust logic doing specific tasks,
    potentially calling Rs bindings when wanting to call web API.

  - **Requester**: Schedule playlist and segment requests.

    The Requester is defined in the `./src/requester.rs` file.

  - **NextSegmentSelector**: Keep track of the next segments that should be
    requested

    The NextSegmentSelector is defined in the `./src/segment_selector`
    directory.

  - **AdaptiveQualitySelector**: Implement Adaptive BitRate (a.k.a. ABR)
    management, such as calculating the network bandwidth, to be able to
    choose the best variant and media selected.

    The AdaptiveQualitySelector is defined in the `./src/adaptive`
    directory.

  - **ContentTracker**: Parses and stores the metadata of the current content as
    well as keep tracks of the current variant and media playlists selected.

    The ContentTracker is defined in the `./src/content.rs` file.
