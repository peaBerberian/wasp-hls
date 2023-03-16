for live contents:

- EXT-X-PROGRAM-DATE-TIME: as f64 time of segment
- notion of a media_offset which would convert between media time to segment time. Only in video_element_ref ?
- Always rely on video_element_ref to know about the currently wanted position:

```rs
let wanted_media_pos = self.awaiting_pos
  .unwrap_or(self.last_observation.map_or(|x| { x.position
match self.media_offset {
  Some(offset) => position - media_offset,
  None =>
}
```

media_offset + playlist_time == media_time
playlist_time = media_time - media_offset

media_offset:

1. Just before Push data -> SourceBuffer
2. transmuxFn/prepareSegment which output timeInfo based on the transmuxed ISOBMFF's tfdt-based start time and trun-duration (second one needed?)
3. Used in some ways by video_element_ref for calculating the `media_offset` (compared to the expected start time)
4. Segment pushed

JS_BLOB has also media_type:
"mpeg-ts" | "mp4"
