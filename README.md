<p align="center">
  <img height="200px" src="https://user-images.githubusercontent.com/8694124/188496034-3b9bde98-58f0-49d0-9744-f3355cd2236e.png#gh-light-mode-only" alt="Wasp-hls's logo"/>
  <img height="200px" src="https://user-images.githubusercontent.com/8694124/188496177-e02ac9f2-ecc5-4d79-a7ce-624eaa71a55b.png#gh-dark-mode-only" alt="Wasp-hls's logo"/>
</p>

Wasp-hls is an [HLS](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) media
player for the web which:

  1. Relies the most possible on WebAssembly (Written initially in the
     [Rust](https://www.rust-lang.org/) language)

  2. Runs mostly in a Web Worker (even for media buffering when APIs are
     available), to reduce the influence an heavy UI can have on playback.

Note that this is only a personal project as well as a proof of concept and it
is still heavily in development. 

## Why starting this project?

I'm currently working as the lead developper of another, featureful adaptive
media player, the open-source [RxPlayer](https://github.com/canalplus/rx-player)
so this is not something totally out of the blue.

However even after stating that, `Wasp-hls` doesn't really answer any real
worthwile need I personally observed when working on the RxPlayer.

The real reasons why I started this project are more personal, mainly:

  - to see how IO-heavy logic (like we have here with many requests, media
    segments streaming, playback observation, network metrics etc.) using web
    APIs only exposed to JavaScript could be conjugated with WebAssembly and
    Rust.

  - to experiment with a big-enough WebAssembly and Web Worker-based library:
    how should it interact with applications written in JavaScript?

  - to learn more about the HLS streaming protocol, for which the RxPlayer does
    not provide first class support (yet?)

  - to see how I would write a complex beast like a Media player if restarting
    from scratch (though the situation is very different here, due to the
    difference in the language used).

  - to see if there's any real performance and/or memory related advantage (or
    disadvantage) in relying on WebAssembly for the core logic of a media
    player, in various situations (multiple players on the same page, large 4k
    segments, web workers, mse-in-worker).

  - to play with a Web Worker-based media player, and find out its influence
    in terms of API definition, synchronization difficulties, performance issues
    etc.

  - to work on and improve my Rust skills

  - To try relying on [wasm\_bindgen](https://github.com/rustwasm/wasm-bindgen)
    on a sufficiently complex library.

## What's left to do?

A lot:

Type of contents:
  - [x] Play HLS VoD contents
  - [x] Transcode mpeg-ts (thanks to mux.js for now)
  - [x] Play HLS live contents _(for now require presence of
        `EXT-X-PROGRAM-DATE-TIME` tag in media playlist)_
  - [ ] Proper support of HLS low-latency contents

Worker-related features:
  - [x] Load content-related resources and run main logic loop in worker
  - [x] Use MSE-in-Worker when available
  - [x] Rely on main thread for MSE when MSE-in-Worker is not available

Adaptive BitRate:
  - [x] Choose variant based on throughtput-based estimates
  - [ ] Also choose variant based on buffer-based estimates

Request Scheduling:
  - [x] Lazy Media Playlist downloading
  - [x] Buffer goal implementation (as in: stop loading segment once enough to fill
    the buffer up to a certain point are loaded)
  - [x] Parallel audio and video segment loading
  - [X] Priorization between audio and video segment requests (to e.g. stop
    doing audio segment requests when video ones become urgent).
  - [ ] Range requests
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
  - [ ] Proper Error API (should be high priority but that does not look like
    fun for now!)
  - [ ] WebAssembly-based mpeg-ts transcoder (very low priority)
  - [ ] Delta playlist handling
  - [ ] Content Steering handling

Playlist tags specifically considered (unchecked ones are mainly just ignored,
most do not prevent playback):
  - [X] EXT-X-ENDLIST: Parsed to know if a playlist needs to be refreshed or
    not, but also to detect if we're playing an unfinished live content to
    play close to the live edge by default.
  - [X] EXTINF: Only the indicated duration of a segment is considered, not
    the title for which we have no use for now. Both integer and float durations
    should be handled.
  - [X] EXT-X-PROGRAM-DATE-TIME: Used to determine the starting position of
    segments as stored by the player - which may be different than the actual
    media time once the corresponding segment is pushed.
    The units/scale indicated by this tag will be preferred over the real media
    time in the player APIs.
  - EXT-X-MAP:
    - [X] URI: Used to fetch the initialization segment if one is present
    - [ ] BYTERANGE: No Range request implementation for now, though this should
      not be hard.
  - EXT-X-MEDIA:
    - [X] TYPE: Both AUDIO and VIDEO are handled. SUBTITLES and CLOSED-CAPTIONS
      are just ignored for now.
    - [X] URI
    - [X] GROUP-ID
    - [X] DEFAULT
    - [X] AUTOSELECT
    - [ ] LANGUAGE: No track selection API yet
    - [ ] ASSOC-LANGUAGE: No track selection API yet
    - [ ] NAME: No track selection API yet
    - [ ] STABLE-RENDITION-ID: Not sure if it will be useful in some way.
    - [ ] FORCED: As the SUBTITLES TYPE is not handled yet, we don't have to use
      this one
    - [ ] INSTREAM-ID: As the CLOSED-CAPTIONS TYPE is not handled yet, we don't
      have to use this one
    - [ ] CHARACTERISTICS: No track selection API yet
    - [ ] CHANNELS: No track selection API yet
  - EXT-X-STREAM-INF:
    - [x] BANDWIDTH: Used to select the right variant in function of the
      bandwidth
    - [X] CODECS: Considered when pushing the segment but NOT to filter only
      compatible renditions yet. Should probably also be used for that in the
      future.
    - [X] AUDIO: As no track selection API exist yet, only the most prioritized
      audio media playlsit is then considered
    - [X] VIDEO: As no track selection API exist yet, only the most prioritized
      video media playlsit is then considered
    - [ ] AVERAGE-BANDWIDTH: Not used yet. I don't know if it's useful for us
      here yet.
    - [ ] SCORE: Not considered yet, but should be used alongside BANDWIDTH to
      select a variant. It does not seem hard to implement...
    - [ ] SUPPLEMENTAL-CODECS: In our web use case, I'm not sure if this is only
      useful for track selection API or if filtering also needs to be done based
      on this.
    - [ ] RESOLUTION: No track selection API yet
    - [ ] FRAME-RATE: No track selection API yet
    - [ ] HDCP-LEVEL: DRM are not handled for now
    - [ ] ALLOWED-CPC: DRM are not handled for now
    - [ ] STABLE-VARIANT-ID: Not sure if this will be useful in some way
    - [ ] SUBTITLES: No subtitles support for now
    - [ ] CLOSED-CAPTIONS: We just ignore that one for now
    - [ ] PATHWAY-ID: Content Steering not handled yet
  - [ ] EXT-X-VERSION: Not specifically considered for now, most differences
    handled until now had compatible behaviors from version to version
  - [ ] EXT-X-INDEPENDENT-SEGMENTS: Might needs to be considered once we're
    doing some flushing?
  - [ ] EXT-X-START: Should be relied on for the default starting position.
    For now we just play at `0` for VoD and at `live-edge - 10` for live
  - [ ] EXT-X-DEFINE: Seems rare enough, so may be supported if the time is
    taken...
  - [ ] EXT-X-TARGETDURATION: Might be useful for heuristics for playlist
    refresh, or for predicting future segments. To see...
  - [ ] EXT-X-MEDIA-SEQUENCE: Not sure of what this allows. To check...
  - [ ] EXT-X-DISCONTINUITY-SEQUENCE
  - [ ] EXT-X-PLAYLIST-TYPE: Not sure if there's an advantage compared to the
    presence of an ENDLIST tag, to check...
  - [ ] EXT-X-I-FRAMES-ONLY: To handle one day, perhaps (very low priority)
  - [ ] EXT-X-PART-INF
  - [ ] EXT-X-SERVER-CONTROL
  - [ ] EXT-X-BYTERANGE: Should probably be handled soon
  - [ ] EXT-X-DISCONTINUITY
  - [ ] EXT-X-KEY: decryption and related tags are very low priority
  - [ ] EXT-X-GAP
  - [ ] EXT-X-BITRATE
  - [ ] EXT-X-PART
  - [ ] EXT-X-DATERANGE: Might be used for an event emitting API?
  - [ ] EXT-X-SKIP
  - [ ] EXT-X-PRELOAD-HINT
  - [ ] EXT-X-RENDITION-REPORT
  - [ ] EXT-X-I-FRAME-STREAM-INF
  - [ ] EXT-X-SESSION-DATA
  - [ ] EXT-X-SESSION-KEY
  - [ ] EXT-X-CONTENT-STEERING

## Architecture

The architecture of the project is as follow:
```
      +------------------------------------------------------------------------------+
      |                                                                              |
      |                                   JS API                                     |
      |                                                                              |
      +------------------------------------------------------------------------------+
  ^                    |   ^                                        ^
  | Main thread        |   |                                        |
-----------------------|---|----------------------------------------|-------------------
  | Web Worker         |   |                                        |
  V                    V   |                                        |
      +-----------------------------------+        +--------------------------------+
      |                                   |        |                                |
      |          MessageReceiver          |        |           TS bindings          |
      |                                   |        |                                |
      +-----------------------------------+        +--------------------------------+
                        |                                 ^                |
  ^ JavaScript          |                                 |                |
  | (once compiled)     |                                 |                |
---------------------------------------------------------------------------------------
  |  WebAssembly        |                                 |                |
  V  (once compiled)    |                                 |                |
                        V                                 |                V
+-------------------------------------------------+      +-------------------+
|                   Dispatcher                    |<-----|    Rs Bindings    |
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
    +----------------+                       | +-----------------------+          | l
    | ContentTracker |-----------------------+-| MediaElementReference |          | e
    +----------------+                         +---------------------- +          | s
                                                                                  |
                       (and other modules...)                                     |
                                                                                  |
```

Here's a definition of terms and blocks in this schema, from left to right and
top to bottom:

  - **JS API**: Implement the library's API, callable from JS applications.

    The JS API is defined in the `src/ts-main/api.ts` file.

  - **Main thread**: On top of the corresponding line is the code running in the
    main thread, alongside your application.

    It hosts the API as well as everything that needs to interact with the page,
    such as handling the video element.

    The corresponding code runs in the `src/ts-main/` directory.

  - **Web Worker**: Below the corresponding line is the code running in a single
    Web Worker. In it runs the main logic of the application.

    That way, actions such as heavy UI interactions will only have a minimal
    impact on media buffering, reducing the risk of rebuffering and improving
    the user experience.

    The corresponding code runs in:
      - the `src/ts-worker/` directory for the TypeScript part of the code
        (compiled into JavaScript once the library is built).
      - the `src/rs-core/` directory for the Rust part of the code (compiled
        into WebAssembly once the library is built).

  - **MessageReceiver**: Entry point of the Web Worker, to which the API post
    messages to.

    The MessageReceiver is defined in the `src/ts-worker/MessageReceiver.ts`
    file.

  - **TS bindings**: Provide web APIs to the WebAssembly part, for example media
    buffering APIs. Also call event listeners on the Rust-side on various
    events.

    Some Web API can only be called on the main thread, which is why the TS
    bindings sometimes need to post messages back to the API.

    The TS bindings are defined in the `src/ts-worker/bindings.ts` file.

  - **Javascript (once compiled)**: higher than the corresponding line is the
    code written in TypeScript, which is then compiled to JavaScript.

    That code is present in the following directories:
      - `src/ts-main/` for the code running in the main thread, such as the
        API
      - `src/ts-worker/` for the code running on the WebWorker

  - **WebAssembly (once compiled)**: Lower than the corresponding line is the
    of code written in Rust - that will be compiled to WebAssembly - present in
    the `src/rs-core` directory.

  - **Dispatcher**: Entry point of the Rust logic. Receive orders and/or events,
    and call the right modules in the right order.

    The Dispatcher is defined in the `src/rs-core/dispatcher/` directory.

  - **Rs bindings**: Define both TypeScript functions exposed by TS bindings but
    also "event listeners" (which are technically a part of the Dispatcher)
    which will be called by TS bindings on various events.

    Rs bindings are defined in the `src/rs-core/bindings/` directory.

  - **Requester**: Schedule playlist and segment requests.

    The Requester is defined in the `src/rs-core/requester/` directory.

  - **NextSegmentSelector**: Keep track of the next segments that should be
    requested

    The NextSegmentSelector is defined in the `src/rs-core/segment_selector/`
    directory.

  - **AdaptiveQualitySelector**: Implement Adaptive BitRate (a.k.a. ABR)
    management, such as calculating the network bandwidth, to be able to
    choose the best variant and media selected.

    The AdaptiveQualitySelector is defined in the `src/rs-core/adaptive/`
    directory.

  - **ContentTracker**: Parses and stores the metadata of the current content as
    well as keep tracks of the current variant and media playlists selected.

    The ContentTracker is defined in the `src/rs-core/content_tracker/`
    directory.

  - **MediaElementReference**: Interface to interact with the media element in
    the web page, as well as to buffer media.

    The MediaElementReference is defined in the `src/rs-core/media_element/`
    directory.

  - **Modules**: Specialized blocks of the Rust logic doing specific tasks,
    potentially calling Rs bindings when wanting to call web API.
