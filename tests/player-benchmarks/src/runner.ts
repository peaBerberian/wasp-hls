import { createAdapter, type Adapter, type PlayerName } from "./adapters.ts";
import type { Mark } from "./observations.ts";
import { scenarios, startup } from "./scenarios/index.ts";
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
  const selected = scenarios[scenario];
  if (!selected) throw new Error(`Unknown scenario ${scenario}`);
  const stopPressure = selected.pressure ? startPressure() : () => ({});
  let player: Adapter | undefined;
  const metrics: Record<string, number> = {};
  const bufferGoal = selected.bufferGoal;
  const fail = (error: unknown) => {
    failure ??= error;
  };
  try {
    const start = performance.now();
    mark("initialize-call");
    player = await healthy(createAdapter(name, video, fail, bufferGoal));
    metrics.initializeMs = performance.now() - start;
    const context = {
      video,
      player,
      url,
      initializeStartedAt: start,
      metrics,
      mark,
      healthy,
      fail,
    };
    await startup(context);
    await selected.afterStartup?.(context);
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
    stopPressure();
    try {
      if (player) await player.dispose();
    } finally {
      video.removeAttribute("src");
      video.load();
    }
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
