# Subtitle rendering

This directory handles subtitle cue installation, removal and rendering on the
main thread. Rust supplies timestamped cue batches; rendering follows the media
element clock. Content and selection identifiers prevent stale batches from
changing the active track.
