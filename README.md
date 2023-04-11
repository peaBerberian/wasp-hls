<p align="center">
  <img height="100px" src="https://user-images.githubusercontent.com/8694124/231414483-eb38f21a-68bf-41b9-98f7-2c8012e2d52e.png#gh-light-mode-only" alt="Wasp-hls's logo"/>
  <img height="100px" src="https://user-images.githubusercontent.com/8694124/231414514-9742e81b-bc4f-4af2-99be-35154c0281cc.png#gh-dark-mode-only" alt="Wasp-hls's logo"/>
</p>

Wasp-hls is an [HLS](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) media
player (the library, streaming engine part, and not the UI/application part for
which I just developped a modest demo) for the web which:

1. Relies the most possible on [WebAssembly](https://webassembly.org/) (Written
   in the [Rust](https://www.rust-lang.org/) language before being compiled).

2. Runs mostly in a [Web Worker](https://en.wikipedia.org/wiki/Web_worker) (even
   for media buffering when APIs are available), to reduce the influence an
   heavy UI can have on playback (and in some situations vice-versa).

Note that this is only a personal project as well as a proof of concept and it
is still heavily in development.

## What's this exactly? HLS?

### Streaming protocols and HLS

To provide media contents at large scale to their customers, most streaming
actors (Netflix, Amazon Prime Video, YouTube, Twitch, Canal+, Disney+ etc. you
got it), rely on the few same HTTP-based adaptive bitrate streaming protocols:
majoritarily [MPEG-DASH](https://en.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP)
and Apple's [HLS](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) (some rely
on only one like Twitch with HLS, others may rely on both depending on the case).

Those protocols all have similar concepts: all expose a central file listing the
characteristics of the content (for example, the video qualities, audio tracks,
the available subtitles etc.) and allow a client to request the media through
small chunks, each containing only the wanted quality and track for a specific
time period.

![de18e941-81de-482f-843d-834a4dd3aa71](https://user-images.githubusercontent.com/8694124/229379027-443a68ee-b818-4fdf-b9fe-db64fd88b5d8.png)
_Schema of how HLS basically works, found on Apple's Website which is the one
behind HLS_

This architecture allows to:

- only load the wanted media data (thus not e.g. also loading all unwanted
  audio tracks with it)
- allows efficient seeking on the content, by allowing to load the content
  non-sequentially (e.g. you can directly load the data corresponding to the end
  of the content if you want to).
- facilitate live streaming by continuously encoding those chunk and adding it
  to the central file progressively
- profit from all the goodies of relying on HTTP(S) for content distribution
  (compatibility with the web, firewall traversal, lots of tools available etc.)

For HLS specifically, this so-called central file is called the "Multivariant
Playlist" (a.k.a. "Master Playlist") and is in the [`M3U8` file
format](https://en.wikipedia.org/wiki/M3U).
HLS also has the concept of secondary "Media Playlists", also as `M3U8` files,
which describe specific tracks and qualities.

### The Media Source Extensions™

To allow the implementation of such adaptive streaming media players on the web
(and thus with JavaScript), a W3C recommendation was written: the [Media Source
Extensions™ recommendation](https://www.w3.org/TR/media-source/), generally just
abbreviated to "MSE".

[Basically](https://www.w3.org/TR/media-source/#introduction), it builds on top
of the HTML5 `<video>` element, adding a new set of browser API accessible from
JavaScript to create media buffers, called `SourceBuffer`s, and allowing to push
aforementioned small chunks of media data to it for later decoding.

### The media player library

The role of an HLS media player library like this one is thus to load the
Multivariant Playlist, detect which characteristics (bandwidth, quality,
codecs, preferred language, accessibility etc.) are wanted and to load the right
media data at the right time, then communicating it to the browser through the
MSE APIs so it can be decoded.

![twitch-wasp](https://user-images.githubusercontent.com/8694124/229379280-c9d7d810-eb32-415c-bdb2-6888a6accd4e.png)
_The Wasp-hls player reading a Multivariant playlist from Twitch. You can see
on the top right the requests performed - mostly of media segments, and some
logs on the bottom right. You can also see a cog logo before the requests' url
indicating that they are all performed in a WebWorker._

This may look relatively simple at first, but there's a lot of potential
optimizations to make and special cases to handle which make quasi-mandatory
the need to develop separately the media streaming library (the part
"understanding" the streaming protocol and pushing chunks) and the application
(the part relying on the library and implementing an UI and the business logic
on top).

Most people generally mean the latter when they talk about a "player", here,
I'm "only" implementing the former and the application has to be developped
separately.

The [demo page](https://peaberberian.github.io/wasp-hls/) do also implement a UI
part, but this is just to showcase the library and is not actually what's
exposed by this package, only the library is (though you can copy the demo's
code if you want to).

### What's WebWorker and WebAssembly doing with all that?

Amongst the main characteristics of this player is that it relies on a
WebWorker, to run concurrently with the application, and WebAssembly, to do so
optimally.
It thus allows to have a theoretically efficient media player, allowing to
avoid stalling in the content if the UI do some heavy lifting and vice-versa.

This is even more important when playing what is called "low-latency contents"
(contents with a small-ish latency between the recording and the watch-ing, in
the few seconds range) which have amongst its characteristics the fact that only
very small data buffers are constructed - allowing to play closer to live but
more exposed to rebuffering risk if the segment loading pipeline takes more
time than expected).

Playing low-latency contents is one of the main goal of this project. On that
matter as an amusing note, playing low-latency contents through a media player
with a WebWorker + WebAssembly combination is exactly what Twitch is doing,
though their players isn't open-source. This one is (it's also able to play
Twitch contents if you succeed to work-around their CORS policy, only not with
low-latency for now)!

### Generating an HLS content

Because this is just the player part, the HLS content has to be prepared -
through what we call the packaging step - separately by using what's called a
"packager".

For example the [shaka-packager](https://github.com/shaka-project/shaka-packager)
is a relatively easy to use packager. With it and [FFmpeg](https://ffmpeg.org/),
you should have all the tools you need to produce any HLS contents you want.

## Why starting this project?

I'm currently working as the lead developper of another, featureful adaptive
media player library, the open-source [RxPlayer](https://github.com/canalplus/rx-player)
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

## What's done

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
      of higher quality - and are sufficiently far from playback to prevent
      rebuffering).
- [x] Smart-switching (I just made-up the name here :D, but basically it's for
      the opposite situation than the one in which fast-switching is active:
      don't re-load segments who're already loaded or being pushed with a higher
      quality).
- [ ] Also choose variant based on buffer-based estimates.
      _Priority: average_
- [ ] Logic to detect sudden large fall in bandwidth before the end of a current
      request.
      _Priority: average_

Request Scheduling:

- [x] Lazy Media Playlist downloading (only fetch and refresh them once they are
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

- [x] Transmux mpeg2-ts (thanks to mux.js on the worker for now)
- [x] End of stream support (as in: actually end when playback reaches the end!)
- [x] Multiple simultaneous type of buffers support (for now only audio and
      video, through MSE `SourceBuffer`s)
- [x] One and multiple initialization segments handling per rendition
- [x] Lazy buffer memory management: Don't manually remove old buffers' media
      data if the browser thinks it's fine. Many players clean it up
      progressively as it also simplifies the logic (e.g. browser GC detection
      might become unneeded) but I like the idea of keeping it to e.g. allow
      seek-back without rebuffering if the current device allows it.
- [x] Detect browser Garbage Collection of buffered media and re-load GCed
      segments if they are needed again.
- [x] Discontinuity handling: Automatically skip "holes" in the buffer where
      it is known that no segment will be pushed to fill them.
- [ ] Freezing handling: Detect when the browser is not making progress in the
      content despite having media data to play and try to unstuck it.
      _Priority: average_
- [ ] Proper handling of `QuotaExceededError` after pushing segments (when low
      on memory).
      This is generally not needed as the browser should already handle some kind of
      garbage collection but some platforms still may have issues when memory is
      constrained.
      _Priority: low_
- [ ] WebAssembly-based mpeg2-ts transmuxer.
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
- [x] Export embedded versions of the WebAssembly and Worker files to facilitate
      application's development code.
- [x] Initial position API
- [ ] Delta playlist handling.
      _Priority: low_
- [ ] Content Steering handling.
      _Priority: low_
- [ ] Support content decryption.
      _Priority: very low_

Playlist tags specifically considered (unchecked ones are mainly just ignored,
most of them are not needed for playback):

- [x] EXT-X-ENDLIST: Parsed to know if a playlist needs to be refreshed or
      not, but also to detect if we're playing an unfinished live content to
      play close to the live edge by default.
- [x] EXTINF: Only the indicated duration of a segment is considered, not
      the title for which there's no use for now. Both integer and float
      durations should be handled.
- [x] EXT-X-PROGRAM-DATE-TIME: Used to determine the starting position of
      segments as stored by the player - which may be different than the actual
      media time once the corresponding segment is pushed.
      The units/scale indicated by this tag will be preferred over the real
      media time in the player APIs.
- [x] EXT-X-BYTERANGE: Used for range requests
- [x] EXT-X-PLAYLIST-TYPE: Used To know if a Playlist may be refreshed
- [x] EXT-X-TARGETDURATION: Useful for heuristics for playlist refresh
- [x] EXT-X-START: Used to determine a default start time in the content.
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
  - [x] LANGUAGE: In audio track selection API
  - [x] ASSOC-LANGUAGE: In audio track selection API
  - [x] NAME: In audio track selection API
  - [ ] CHANNELS: Not so hard to implement, but I've been too lazy to parse that
        specific format from the Multivariant Playlist for now
  - [ ] CHARACTERISTICS: Soon...
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
  - [x] VIDEO: As no video track selection API exist yet, only the most
        prioritized video media playlist is considered
  - [x] RESOLUTION: Used to describe variant in variant selection API
  - [x] FRAME-RATE: Used to describe variant in variant selection API
  - [x] SCORE: Considered both to select a variant and to determine if a quality
        is better when "fast-switching".
  - [ ] STABLE-VARIANT-ID: Not really needed for now (only for content steering?)
  - [ ] AVERAGE-BANDWIDTH: Not used yet. I don't know if it's useful yet for us.
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

## Setup

If you want to contribute or build the Wasp-hls locally, you will need to have
[nodejs](https://nodejs.org/) and [rust](https://www.rust-lang.org/tools/install)
installed.

Then you need to install node dependencies by calling in your shell:

```sh
# Install all node dependencies (needs npm, generally installed with nodejs)
npm install
```

You also need to add the rust wasm32 target and some rust dependencies:

```sh
# Add wasm32 target (needs rustup that you most likely took with rust)
rustup target add wasm32-unknown-unknown

# Add wasm-bindgen CLI (needs cargo, generally installed with rust)
cargo install wasm-bindgen-cli

# Optionally, you may also need clippy, for checking Rust code mistakes
rustup component add clippy
```

## Build

The building of the Wasp-hls player may be performed by module, if you just
updated one area of the code (the Rust code for example), or as a whole.

### The Rust (WebAssembly) code

To build only the Rust code in `src/rs-core/` to its destination WebAssembly
file (`build/wasp_hls_bg.wasm`), you can run any of the following commands:

```sh
# Build in debug mode, which leads to a bigger file and slower code, but is
# more useful and quicker to build when developping
npm run build:wasm

# Build in release mode, which is the actual delivered result
npm run build:wasm:release
```

### The TypeScript Worker code

To build only the Worker code in `src/ts-worker` to its destination JavaScript
file (`build/worker.js`), you can run any of the following commands:

```sh
# Build in debug mode, which leads to a bigger file though much easier to debug
npm run build:worker

# Build in release mode, which is the actual delivered minified result
npm run build:worker:release
```

### The Main (API) code

To build only the code running in the main thread present in `src/ts-main` to
its destination JavaScript file (`build/main.js`), you can run any of the
following commands:

```sh
# Build in debug mode, which leads to a bigger file though much easier to debug
npm run build:main

# Build in release mode, which is the actual delivered minified result
npm run build:main:release
```

### The Demo

To build only the demo application showcasing the Wasp-hls player, whose code
is present in the (`demo/`) directory, to its destination JavaScript file
(`build/demo.js`), you can run any of the following commands:

```sh
# Build in debug mode, which leads to a bigger file though much easier to debug
npm run build:demo

# Build in debug mode, with a "watcher" rebuilding each time one of its files
# changes
npm run build:demo:watch

# Build in release mode, which is the actual delivered minified result
npm run build:demo:release
```

Then to perform your tests, you generally want to serve the demo. You can do so
with:

```sh
npm run serve
```

### Combinations

If you just want to build the whole Wasp-hls player code, without the demo, you
may call:

```sh
npm run build:all
```

If you want to build all that code AND the demo:

```sh
npm run build:all && npm run build:demo
```

Though what you most likely want to do here is build the full code used by
the demo to perform your tests, here just write:

```sh
npm run build:all:demo
```

That last script bypass the generation of the `build/main.js` file, as the demo
file (`build/demo.js`) already includes the content of that file anyway.

To build everything in release mode, for an actual release or for tests in
production conditions, write:

```sh
# WebAssembly + Worker + Main in release mode
npm run build:release

# If you also want the demo in release mode
npm run build:demo:release
```

### The Documentation

The documentation, written in `doc/` may also be built to its final directory
(`doc/generated`), through the following command:

```sh
npm run doc
```

It may then be served, so it can be read on a web browser, through:

```sh
npm run serve:doc
```

## Update the code

You're welcome to read the code which should be hopefully documented enough and
readable enough to dive into. The source code of the player is in the `src`
directory, if you would prefer to work on the demo, it's in the `demo`
directory, as for the documentation, it's in the `demo` directory.

## Check the code

To check the TypeScript types of TypeScript files and their code style with
the `eslint` package, you can run:

```sh
# Check all TypeScript files in the project
npm run check

# OR, check only the Worker code
npm run check:worker

# OR, check only the Main code
npm run check:main

# OR, check only the Demo code
npm run check:demo
```

To check the Rust code, with clippy, you can run:

```sh
# Check all Rust files in the project
npm run clippy
```

You also might want to format automatically the code before commiting:

```sh
# Format all TypeScript, JavaScript, Markdown and HTML files in the project
npm run fmtt

# Format all Rust code
npm run fmtr

# Do both
npm run fmt
```
