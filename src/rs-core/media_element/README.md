# Media element interaction

This directory is where we keep the Rust-side view of the media element and its
media buffers, alongside the logic to act on them. The main entry point is
`MediaElementReference` in `mod.rs`.

Its job is to:

- Attach and remove a MediaSource, create audio/video SourceBuffers and signal
  end of stream once the buffers are finished with their pending operations.

- Keep the last playback observation, the wanted speed and a pending seek if
  seeking isn't possible yet (e.g. startup before HTML5 `readyState` 1). It
  also handles entering and leaving rebuffering according to the available
  buffer.

- Keep the offset between playlist positions and media element positions. Those
  aren't necessarily the same, and the offset is only known once we have timing
  information from a pushed segment.

- Push and remove media data, and keep an inventory of which segments appear to
  still be buffered, with their timing and quality information.

`source_buffers.rs` handles each buffer's pending operations, flush behavior and
timing continuity hints for transmuxing. `segment_inventory.rs` keeps segment
metadata and reconciles it with the reported buffered ranges, including after
data has been removed or garbage collected by the browser.

A media push is done in two steps: first announce it to the inventory, then
actually push it with the returned data. This lets the caller prepare its next
actions before doing potentially expensive work. Completion callbacks and
playback observations then let us reconcile that inventory with what actually
made it into the buffer.

## Where it stops

This code owns the Rust-side buffer wrappers, inventories and playback state
described above. The actual media element, MediaSource and SourceBuffers live
on the JavaScript side; actions here go through JavaScript calls and may finish
asynchronously. The caller has to feed back observations, MediaSource state
changes and buffer operation results.

It doesn't choose tracks, qualities or which segments to download, perform
downloads, or parse/transmux segment bytes itself. It acts on the data and
metadata it is given. The inventory is an estimate of the buffer's contents,
not a guarantee that the browser can decode everything described in it.
