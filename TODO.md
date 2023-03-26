flush:

- empty-queue + `set_playback_r` a 0 avant

Score + bandwidth sorted in this order descending?

Text IMSC1? VTT?

Fast/smart switching:

- au changement de qualité/score re-check les segments actuels depuis base position et choisi une nouvelle position a redémarrer depuis (fast-switching)
- en général, si segment est déjà présent dans la même qualité ou mieux, skip le (smart-switching) et pousse le dans un vec `already_buffed_segments`
- clean régulièrement `already_buffed_segments` si la position courante passe devant la fin d'un des segments skipped.
- a chaque fois que l'on check le segment a télécharger, on re-check `already_buffed_segments`

- already_buffed_segments contient:
  - buffered_start
  - buffered_end

```
struct BufferedSegmentInfo {
  buffered_start: f64,
  buffered_end: f64,
}

struct Toto {
  /// Information on segments that were already present in the buffer and have led the
  /// `NextSegmentSelector` to skip segment(s) that would have unnecessarily replaced it.
  ///
  /// For example, let's say we're now loading 720p video segments. While iterating on the next
  /// chronological segment, we find out that a 1080p segment is already found for
  /// the same wanted positions. In such cases, that new 720p segment is skipped (it is not
  /// returned by the `NextSegmentSelector`) and the information on the corresponding 1080p
  /// segment is added to this property.
  ///
  /// Because they were not part of the current `NextSegmentSelector` iteration, members of that
  /// object may disappear from the buffer at any time, for example because a previous buffer
  /// cleaning operation to remove them was pending before and has now finished.
  /// To ensure that playback can still continue, the presence in the buffer of every members in that
  /// object should be re-checked regularly, if it disappears, a segment should be loaded to replace
  /// it.
  already_buffed_segments: Vec<BufferedSegmentInfo>;

  // CA OU REF au moment du call?
  still_loading_segments: Vec<(start, end, score?)>?

  // Time-range of segments that have been l
  unvalidated_ranges: Vec<(start, end, score)>;
}
```
