import { createAdapter, type Adapter, type PlayerName } from "./adapters.ts";
import { Observations, type Mark } from "./observations.ts";
import { startPressure } from "./workload.ts";
import { installDiagnostics } from "./diagnostics.js";

const params = new URLSearchParams(location.search);
const marks: Mark[] = [];
const started = performance.now();
const mark = (name: string, details?: unknown) =>
  marks.push({ name, time: performance.now() - started, details });
let failure: unknown;
let activeDiagnostics: unknown[] = [];
window.addEventListener("error", (event) => {
  failure ??= event.error ?? event.message;
});
window.addEventListener("unhandledrejection", (event) => {
  failure ??= event.reason;
});

async function healthy<T>(promise: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setInterval>;
  const check = new Promise<never>((_resolve, reject) => {
    timer = setInterval(() => {
      if (failure)
        reject(
          new Error(
            JSON.stringify(
              failure,
              Object.getOwnPropertyNames(Object(failure)),
            ),
          ),
        );
    }, 25);
  });
  try {
    return await Promise.race([promise, check]);
  } finally {
    clearInterval(timer!);
  }
}

async function main() {
  const name = params.get("player") as PlayerName;
  const scenario = params.get("scenario")!;
  const url = params.get("content")!;
  const video = document.querySelector("video")!;
  video.muted = true;
  video.playsInline = true;
  const diagnostics = params.has("diagnostic") ? installDiagnostics() : [];
  activeDiagnostics = diagnostics;
  const stopPressure = scenario === "pressure" ? startPressure() : () => ({});
  let player: Adapter | undefined;
  let observations = new Observations(mark);
  const metrics: Record<string, number> = {};
  const sustained = scenario === "playback" || scenario === "pressure";
  const bufferGoal = sustained ? 4 : undefined;
  try {
    const load = observations.load(video);
    const start = performance.now();
    mark("initialize-call");
    player = await healthy(
      createAdapter(
        name,
        video,
        (error) => {
          failure ??= error;
        },
        bufferGoal,
      ),
    );
    metrics.initializeMs = performance.now() - start;
    const loadStart = performance.now();
    mark("load-call");
    const loading = player.load(url);
    // Observe load rejection immediately, even if canplay has not fired.
    void loading.catch((error) => {
      failure ??= error;
    });
    await healthy(load.canPlay);
    mark("play-call");
    await healthy(video.play());
    await healthy(
      Promise.all([loading, load.metadata, load.playing, load.firstFrame]),
    );
    metrics.initializeToFirstFrameMs = (await load.firstFrame) - start;
    metrics.loadToFirstFrameMs = (await load.firstFrame) - loadStart;
    metrics.loadToMetadataMs = (await load.metadata) - loadStart;
    metrics.loadToCanPlayMs = (await load.canPlay) - loadStart;
    metrics.loadToPlayingMs = (await load.playing) - loadStart;
    observations.dispose();
    observations = new Observations(mark);
    if (scenario === "seek") {
      const target = 1;
      if (!Number.isFinite(target) || target < 1)
        throw new Error("Invalid seek target");
      const buffered = Array.from({ length: video.buffered.length }, (_, i) => [
        video.buffered.start(i),
        video.buffered.end(i),
      ]);
      mark("seek-call", { target, buffered });
      if (!buffered.some(([start, end]) => start <= target && target < end))
        throw new Error(
          "Buffered seek target must be buffered in every player",
        );
      const seekStart = performance.now();
      const seeked = observations.event(video, "seeked");
      const displayed = observations.frame(
        video,
        (metadata) =>
          !video.seeking && Math.abs(metadata.mediaTime - target) < 0.5,
      );
      video.currentTime = target;
      await healthy(Promise.all([seeked, displayed]));
      metrics.seekToFrameMs = (await displayed) - seekStart;
    }
    if (scenario === "reload") {
      const reloadStart = performance.now();
      video.pause();
      const reloaded = observations.load(video);
      const loadingAgain = player.load(url);
      void loadingAgain.catch((error) => {
        failure ??= error;
      });
      await healthy(reloaded.canPlay);
      await healthy(video.play());
      await healthy(
        Promise.all([
          loadingAgain,
          reloaded.firstFrame,
          reloaded.metadata,
          reloaded.playing,
        ]),
      );
      metrics.reloadToFrameMs = (await reloaded.firstFrame) - reloadStart;
    }
    if (sustained) {
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
      video.addEventListener("waiting", onWaiting);
      video.addEventListener("playing", onPlaying);
      const initialQuality = video.getVideoPlaybackQuality();
      const initialTime = video.currentTime;
      const bufferedAhead = () => {
        for (let i = 0; i < video.buffered.length; i++) {
          if (
            video.buffered.start(i) <= video.currentTime &&
            video.buffered.end(i) >= video.currentTime
          )
            return video.buffered.end(i) - video.currentTime;
        }
        return 0;
      };
      const bufferedAheadStartSeconds = bufferedAhead();
      const wallStart = performance.now();
      await healthy(new Promise((resolve) => setTimeout(resolve, 16000)));
      if (waitingAt !== undefined) stalledMs += performance.now() - waitingAt;
      const quality = video.getVideoPlaybackQuality();
      Object.assign(metrics, {
        playbackWallMs: performance.now() - wallStart,
        playbackProgressSeconds: video.currentTime - initialTime,
        bufferedAheadStartSeconds,
        bufferedAheadEndSeconds: bufferedAhead(),
        waitingCount,
        stalledMs,
        droppedFrames:
          quality.droppedVideoFrames - initialQuality.droppedVideoFrames,
        totalFrames: quality.totalVideoFrames - initialQuality.totalVideoFrames,
      });
    }
    return {
      player: name,
      contentUrl: url,
      scenario,
      policy: {
        bufferGoalSeconds: bufferGoal ?? "player-default",
        cache: "no-store",
        scriptEvaluation: "excluded",
      },
      metrics,
      marks,
      workload: stopPressure(),
      diagnostics: diagnostics.map((entry: { time: number }) => ({
        ...entry,
        time: entry.time - performance.timeOrigin - started,
      })),
      resources: performance
        .getEntriesByType("resource")
        .map((item) => item.toJSON()),
    };
  } finally {
    observations.dispose();
    stopPressure();
    if (player) await player.dispose();
    video.removeAttribute("src");
    video.load();
  }
}

main().then(
  (result) =>
    fetch("/result", {
      method: "POST",
      body: JSON.stringify({ token: params.get("token"), result }),
    }),
  (error) =>
    fetch("/result", {
      method: "POST",
      body: JSON.stringify({
        token: params.get("token"),
        error: String(error),
        marks,
        diagnostics: activeDiagnostics,
      }),
    }),
);
