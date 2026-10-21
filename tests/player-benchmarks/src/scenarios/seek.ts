import { Observations } from "../observations.ts";
import type { ScenarioContext } from "./context.ts";

export default async function seek({
  video,
  mark,
  healthy,
  metrics,
}: ScenarioContext) {
  const target = 1;
  const buffered = Array.from({ length: video.buffered.length }, (_, i) => [
    video.buffered.start(i),
    video.buffered.end(i),
  ]);
  mark("seek-call", { target, buffered });
  if (!buffered.some(([start, end]) => start <= target && target < end))
    throw new Error("Buffered seek target must be buffered in every player");
  const observations = new Observations(mark);
  try {
    const start = performance.now();
    const seeked = observations.event(video, "seeked");
    const displayed = observations.frame(
      video,
      (metadata) =>
        !video.seeking && Math.abs(metadata.mediaTime - target) < 0.5,
    );
    video.currentTime = target;
    await healthy(Promise.all([seeked, displayed]));
    metrics.seekToFrameMs = (await displayed) - start;
  } finally {
    observations.dispose();
  }
}
