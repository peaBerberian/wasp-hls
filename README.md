<p align="center">
  <img height="200px" src="https://user-images.githubusercontent.com/8694124/188496034-3b9bde98-58f0-49d0-9744-f3355cd2236e.png#gh-light-mode-only" alt="Wasp-hls's logo"/>
  <img height="200px" src="https://user-images.githubusercontent.com/8694124/188496177-e02ac9f2-ecc5-4d79-a7ce-624eaa71a55b.png#gh-dark-mode-only" alt="Wasp-hls's logo"/>
</p>

Wasp-hls is an [HLS](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) media
player for the web which:

1. Relies the most possible on WebAssembly (Written initially in the
   [Rust](https://www.rust-lang.org/) language)

2. Runs mostly in a Web Worker (even for media buffering when APIs are
   available), to reduce the influence an heavy UI can have on playback (and
   in some situations vice-versa).

Note that this is only a personal project as well as a proof of concept and it
is still heavily in development.

## Why starting this project?

I'm currently working as the lead developper of another, featureful adaptive
media player, the open-source [RxPlayer](https://github.com/canalplus/rx-player)
so this is not something totally out of the blue.

The reasons why I started this project are mainly:

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

- To try relying on [wasm_bindgen](https://github.com/rustwasm/wasm-bindgen)
  on a sufficiently complex library.

## What's left to do?

It already has a lot of features but there's still some left work:

Type of contents:

- [x] Play HLS VoD contents
- [x] Play HLS live contents _(for now require presence of
      `EXT-X-PROGRAM-DATE-TIME` tag in media playlist)_
- [ ] Proper support of HLS low-latency contents.
      _Priority: average_

Worker-related features:

- [x] Load content-related resources and run main logic loop in worker
- [x] Use MSE-in-Worker when available
- [x] Rely on main thread for MSE when MSE-in-Worker is not available

Adaptive BitRate:

- [x] Choose variant based on throughtput-based estimates
- [x] Allow application to list and select its own variant (quality) and know
      the current one
- [x] Automatically filter out codecs not supported by the current environment.
- [x] Urgent/non-urgent quality switches (Some quality switches lead to request
      for segments of the previous quality to be immediately interrupted, others
      await them before actually switching).
- [x] Fast-switching (Push on top of already-loaded segments if they prove to be
      of higher quality - and are sufficiently far from playback to lead to
      rebuffering).
- [x] Smart-switching (I just made-up the name here :D, but basically it's for
      the opposite situation than the one in which fast-switching is active:
      don't re-load segments who're already loaded / being pushed with a higher
      quality).
- [ ] Also choose variant based on buffer-based estimates.
      _Priority: average_
- [ ] Logic to detect sudden large fall in bandwidth before the end of a current
      request.
      _Priority: average_

Request Scheduling:

- [x] Lazy Media Playlist downloading (only fetch and refresh them once they ar
      needed)
- [x] Media Playlist refreshing for live contents
- [x] Buffer goal implementation (as in: stop loading segments once enough to
      fill the buffer up to a certain - configurable - point are loaded)
- [x] Parallel audio and video segment loading
- [x] Priorization between audio and video segment requests (to e.g. stop
      doing audio segment requests when video ones become urgent).
- [x] Retry of failed requests with an exponential backoff.
- [x] Perform range requests for segments if needed
- [ ] Parallel initialization segment and first media segment loading.
      _Priority: average_

Media demuxing/decoding, MSE API and buffer handling:

- [x] Transmux mpeg-ts (thanks to mux.js on the worker for now)
- [x] End of stream support (as in: actually end when playback reaches the end!)
- [x] Multiple simultaneous type of buffers support (for now only audio and
      video, through MSE `SourceBuffer`s)
- [x] Lazy buffer memory management: Don't manually remove old buffers' media
      data if the browser thinks it's fine. Many players clean it up
      progressively as it also simplifies the logic (e.g. browser GC detection
      might become unneeded) but I like the idea of keeping it to e.g. allow
      seek-back without rebuffering if the current device allows it.
- [x] Detect browser Garbage Collection of segments and re-load GCed segment if
      they are needed again.
- [ ] Discontinuity handling.
      _Priority: average_
- [ ] Proper handling of `QuotaExceededError` after pushing segments (when low
      on memory).
      This is generally not needed as the browser should already handle some kind of
      garbage collection but some platforms still may have issues when memory is
      constrained.
      _Priority: low_
- [ ] WebAssembly-based mpeg-ts transcoder.
      _Priority: low_

Tracks:

- [x] Provide API to set an audio track
- [ ] Provide API to set a video track
      _Priority: low_
- [ ] Allow text track selection and support at least one text track format
      (TTML IMSC1 or webVTT) - through a JS library first?
      _Priority: low_

Miscellaneous:

- [x] Error API
- [ ] Initial position API
      _Priority: average_
- [ ] Delta playlist handling.
      _Priority: low_
- [ ] Content Steering handling.
      _Priority: low_
- [ ] Support content decryption.
      _Priority: very low_

Playlist tags specifically considered (unchecked ones are mainly just ignored,
most do not prevent playback):

- [x] EXT-X-ENDLIST: Parsed to know if a playlist needs to be refreshed or
      not, but also to detect if we're playing an unfinished live content to
      play close to the live edge by default.
- [x] EXTINF: Only the indicated duration of a segment is considered, not
      the title for which there's no use for now. Both integer and float
      durations should be handled.
- [x] EXT-X-PROGRAM-DATE-TIME: Used to determine the starting position of
      segments as stored by the player - which may be different than the actual
      media time once the corresponding segment is pushed.
      The units/scale indicated by this tag will be preferred over the real media
      time in the player APIs.
- [x] EXT-X-BYTERANGE: Used for range requests
- [x] EXT-X-PLAYLIST-TYPE: Used To know if a Playlist may be refreshed
- [x] EXT-X-TARGETDURATION: Useful for heuristics for playlist refresh
- [x] EXT-X-START: Relied on for the default starting position.
- EXT-X-MAP:
  - [x] URI: Used to fetch the initialization segment if one is present
  - [x] BYTERANGE: To perform a range request for the initialization segment
- EXT-X-MEDIA:
  - [x] TYPE: Both AUDIO and VIDEO are handled. SUBTITLES and CLOSED-CAPTIONS
        are just ignored for now.
  - [x] URI
  - [x] GROUP-ID
  - [x] DEFAULT
  - [x] AUTOSELECT
        URI is relied on if it does not exists) if one is linked to it, but also
        may be re-used for defining a unique track identifier in some API.
  - [x] LANGUAGE: In audio track selection API
  - [x] ASSOC-LANGUAGE: In audio track selection API
  - [x] NAME: In audio track selection API
  - [x] CHANNELS: In audio track selection API
  - [ ] CHARACTERISTICS: Soon....
  - [ ] FORCED: As the SUBTITLES TYPE is not handled yet, we don't have to use
        this one
  - [ ] INSTREAM-ID: As the CLOSED-CAPTIONS TYPE is not handled yet, we don't
        have to use this one.
  - [ ] STABLE-RENDITION-ID: Not really needed for now (only for content steering?)
- EXT-X-STREAM-INF:
  - [x] BANDWIDTH: Used to select the right variant in function of the
        bandwidth
  - [x] CODECS: Used for checking support (and filtering out if that's not the
        case, and for initializing buffers with the right info).
  - [x] AUDIO
        audio media playlist is then considered
  - [x] VIDEO: As no video track selection API exist yet, only the most
        prioritized video media playlist is considered
  - [x] RESOLUTION: Used to describe variant in variant selection API
  - [x] FRAME-RATE: Used to describe variant in variant selection API
  - [x] SCORE: Considered both to select a variant and to determine if a quality
        is better when "fast-switching".
  - [ ] STABLE-VARIANT-ID: Not really needed for now (only for content steering?)
        used.
  - [ ] AVERAGE-BANDWIDTH: Not used yet. I don't know if it's useful yet.
  - [ ] SUPPLEMENTAL-CODECS: In our web use case, I'm not sure if this is only
        useful for track selection API or if filtering also needs to be done based
        on this.
  - [ ] SUBTITLES: No subtitles support for now
  - [ ] CLOSED-CAPTIONS: that one is just ignored for now
  - [ ] PATHWAY-ID: Content Steering not handled yet
  - [ ] HDCP-LEVEL: DRM are not handled for now
  - [ ] ALLOWED-CPC: DRM are not handled for now
- [ ] EXT-X-GAP
- [ ] EXT-X-VERSION: Not specifically considered for now, most differences
      handled until now had compatible behaviors from version to version
- [ ] EXT-X-INDEPENDENT-SEGMENTS: Might needs to be considered once we're
      doing some manual cleaning?
- [ ] EXT-X-DEFINE: Seems rare enough, so may be supported if the time is
      taken...
- [ ] EXT-X-MEDIA-SEQUENCE: Not sure of what this allows. To check...
- [ ] EXT-X-I-FRAMES-ONLY: To handle one day, perhaps (very low priority)
- [ ] EXT-X-PART: low-latency related
- [ ] EXT-X-PART-INF: low-latency related
- [ ] EXT-X-SERVER-CONTROL: low-latency related?
- [ ] EXT-X-BITRATE
- [ ] EXT-X-DATERANGE: Might be used for an event emitting API?
- [ ] EXT-X-SKIP
- [ ] EXT-X-PRELOAD-HINT
- [ ] EXT-X-RENDITION-REPORT
- [ ] EXT-X-I-FRAME-STREAM-INF
- [ ] EXT-X-SESSION-DATA
- [ ] EXT-X-SESSION-KEY
- [ ] EXT-X-CONTENT-STEERING
- [ ] EXT-X-KEY: decryption and related tags are very low priority
- [ ] EXT-X-DISCONTINUITY: I'm under the impression in our scenario that it
      only is useful to increment discontinuity sequences, which we have no
      need for...
- [ ] EXT-X-DISCONTINUITY-SEQUENCE: I don't think we need this, at least I
      didn't encounter a case for it now that isn't handled by other tags.

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
|                   Dispatcher                    |      |    Rs Bindings    |
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

- **Rs bindings**: Define JavaScript functions exposed by TS bindings.

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

- **PlaylistStore**: Parses and stores the metadata of the current content as
  well as keep tracks of the current variant and media playlists selected.

  The PlaylistStore is defined in the `src/rs-core/playlist_store/`
  directory.

- **MediaElementReference**: Interface to interact with the media element in
  the web page, as well as to buffer media.

  The MediaElementReference is defined in the `src/rs-core/media_element/`
  directory.

- **Modules**: Specialized blocks of the Rust logic doing specific tasks,
  potentially calling Rs bindings when wanting to call web API.
