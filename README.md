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
  - [ ] Retry of failed requests with an exponential backoff 
  - [ ] Some sort of synchronization between audio requests and video requests (to
    e.g. stop doing audio segment requests when video ones become urgent, to free
    some bandwidth)

Buffers:
  - [x] End of stream support (as in: actually end when playback reached the end!)
  - [x] Multiple simultaneous type of buffers support (for now only audio and video)
  - [ ] Inventory storing which quality is where in the buffers, both for API reasons
    and for several optimizations (though quality identification seems more difficult
    to implement in HLS than in DASH due to the fact that HLS only link variants to
    bitrate, not the actual audio and video streams - but it should be doable).

Tracks:
  - [ ] Provide API to set an audio, video or text track
  - [ ] support of at least one format (didn't check which yet)
    _(low priority)_

Decryption:
  - [ ] Support content decryption _(very low priority)_

Miscellaneous:
  - [ ] Make usage of the upcoming MSE-in-worker API
  - [ ] WebAssembly-based mpeg-ts transcoder (very low priority)
  - [ ] Proper Error API (should be high priority but that does not look like
    fun for now!)
