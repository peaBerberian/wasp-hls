# Adaptive quality selection

This directory is where we try to choose an appropriate HLS variant according to
the current playback conditions. The entry point is `AdaptiveQualitySelector` in
`mod.rs`.

It keeps an estimate of the available bandwidth from completed downloads, then
uses that estimate alongside the current buffer and the available variants to
recommend what quality to load next. The recommendation distinguishes between
adding new data to the buffer and replacing data already there: we can afford to
be more adventurous (higher than calculated throughput) with the former.

The other files here support that estimate. `bandwidth_estimator.rs` collects
download samples, and `ewma.rs` provides the moving average it uses.

## Where it stops

The selector only makes recommendations. It does not download segments or
manage the media buffer itself.
