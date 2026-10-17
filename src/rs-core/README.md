# Rust player core

This directory is the home of the player logic written in Rust, compiled to
WebAssembly. It keeps the current content's state, decides what to load and
reacts to commands and events from JavaScript.

The main entry point is `Dispatcher`, in `dispatcher/`. `lib.rs` declares the
modules and provides functions to allocate and free memory used by JavaScript.

The code is split into:

- `dispatcher/`: receives commands and events, then calls the right methods in
  the right order. It keeps the current content's state and handles the steps
  to load, play and stop it.

- `parser/`: translates HLS playlists into usable structures, including their
  segment timelines and relationships between variants and tracks.

- `playlist_store/`: keeps loaded playlists, selected variants and tracks, and
  the metadata needed to know whether those choices are usable.

- `adaptive/`: estimates available bandwidth and recommends qualities according
  to playback conditions.

- `segment_selector/`: figures out which segments are needed to fill the
  buffer or replace lower-quality media already there.

- `requester/`: manages playlist and segment requests, including priorities,
  retries and cancellation.

- `media_element/`: keeps the Rust-side playback and buffer state, acts on media
  buffers and tracks which segments appear to still be present.

- `bindings/`: defines the JavaScript functions callable from Rust and converts
  values between the Rust and JavaScript representations.

- `text/`: parses subtitle segments and maps cue timestamps to playback time.

- `utils/`: shared helpers.

Each directory has its own README describing its responsibilities and limits.

## Where it stops

Here we decide what to request and what to do with the results. JavaScript
performs the actual network requests, runs timers, calls browser media APIs and
inspects or transmuxes segments. Downloaded segments can stay in JavaScript
memory, with Rust only keeping an identifier for them. This avoids copying data
when Rust doesn't need to read it.

The core needs commands and callbacks from JavaScript to keep going. It keeps
its own playback state, updated from observations and operation results sent by
JavaScript. It doesn't define the JavaScript API used by applications or display
a player interface.
