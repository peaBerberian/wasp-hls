import type { ScenarioContext } from "./context.ts";

export default async function playback({
  video,
  healthy,
  metrics,
}: ScenarioContext) {
  let waitingCount = 0;
  let stalledMs = 0;
  let waitingAt: number | undefined;
  const onWaiting = () => {
    waitingCount++;
    waitingAt ??= performance.now();
  };
  const onPlaying = () => {
    if (waitingAt !== undefined) {
      stalledMs += performance.now() - waitingAt;
      waitingAt = undefined;
    }
  };
  const bufferedAhead = () => {
    for (let i = 0; i < video.buffered.length; i++)
      if (
        video.buffered.start(i) <= video.currentTime &&
        video.buffered.end(i) >= video.currentTime
      )
        return video.buffered.end(i) - video.currentTime;
    return 0;
  };
  video.addEventListener("waiting", onWaiting);
  video.addEventListener("playing", onPlaying);
  const initialQuality = video.getVideoPlaybackQuality();
  const initialTime = video.currentTime;
  const bufferedAheadStartSeconds = bufferedAhead();
  const start = performance.now();
  try {
    await healthy(new Promise((resolve) => setTimeout(resolve, 16000)));
    if (waitingAt !== undefined) stalledMs += performance.now() - waitingAt;
    const quality = video.getVideoPlaybackQuality();
    Object.assign(metrics, {
      playbackWallMs: performance.now() - start,
      playbackProgressSeconds: video.currentTime - initialTime,
      bufferedAheadStartSeconds,
      bufferedAheadEndSeconds: bufferedAhead(),
      waitingCount,
      stalledMs,
      droppedFrames:
        quality.droppedVideoFrames - initialQuality.droppedVideoFrames,
      totalFrames: quality.totalVideoFrames - initialQuality.totalVideoFrames,
    });
  } finally {
    video.removeEventListener("waiting", onWaiting);
    video.removeEventListener("playing", onPlaying);
  }
}
