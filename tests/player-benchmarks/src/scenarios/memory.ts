import { checkMemoryBuffer, measureMemory } from "../memory.ts";
import type { ScenarioContext } from "./context.ts";
import { loadContent } from "./startup.ts";
import { Observations } from "../observations.ts";

export default async function memory(context: ScenarioContext) {
  const { video, healthy, metrics, memorySnapshots } = context;
  await healthy(
    new Promise<void>((resolve) => {
      const tick = () => {
        if (video.currentTime < 16) return;
        video.removeEventListener("timeupdate", tick);
        video.pause();
        resolve();
      };
      video.addEventListener("timeupdate", tick);
      tick();
    }),
  );
  await healthy(new Promise((resolve) => setTimeout(resolve, 1000)));
  for (let i = 0; i < 3; i++) {
    const snapshot = await healthy(measureMemory(`steady-${i}`, video));
    checkMemoryBuffer(snapshot);
    memorySnapshots.push(snapshot);
  }
  const values = memorySnapshots
    .filter((x) => x.phase.startsWith("steady-"))
    .map((x) => x.bytes)
    .sort((a, b) => a - b);
  metrics.steadyApplicationBytes = values[1];
  metrics.loadedMinusEmptyBytes =
    values[1] - memorySnapshots.find((x) => x.phase === "empty")!.bytes;
  metrics.loadedMinusInitializedBytes =
    values[1] - memorySnapshots.find((x) => x.phase === "initialized")!.bytes;
}

/** Post-GC checkpoints measure retention, not transient allocation peaks. */
export async function memoryChurn(context: ScenarioContext) {
  await memory(context);
  for (let i = 0; i < 3; i++) {
    const observations = new Observations(context.mark);
    try {
      const seeked = observations.event(context.video, "seeked");
      context.video.currentTime = 8;
      await context.healthy(seeked);
    } finally {
      observations.dispose();
    }
    context.memorySnapshots.push(
      await context.healthy(measureMemory(`seek-${i}`, context.video)),
    );
    await loadContent(context, "reload");
    context.video.pause();
    context.memorySnapshots.push(
      await context.healthy(measureMemory(`reload-${i}`, context.video)),
    );
  }
}
