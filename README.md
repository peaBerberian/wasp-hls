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

Almost everything, for the moment it can just play the main renditions of the
first encountered variant for VoD contents only.
