import startup from "./startup.ts";
import seek from "./seek.ts";
import reload from "./reload.ts";
import playback from "./playback.ts";
import memory, { memoryChurn } from "./memory.ts";
import type { ScenarioContext } from "./context.ts";

// This local registry is intentionally small, not a public scenario API.
export const scenarios: Record<
  string,
  {
    afterStartup?(context: ScenarioContext): Promise<void>;
    bufferGoal?: number;
    pressure?: boolean;
    memory?: boolean;
  }
> = {
  startup: {},
  seek: { afterStartup: seek },
  reload: { afterStartup: reload },
  playback: { afterStartup: playback, bufferGoal: 4 },
  pressure: { afterStartup: playback, bufferGoal: 4, pressure: true },
  "large-playlist": {},
  memory: { afterStartup: memory, bufferGoal: 4, memory: true },
  "memory-large-playlist": {
    afterStartup: memory,
    bufferGoal: 4,
    memory: true,
  },
  "memory-churn": { afterStartup: memoryChurn, bufferGoal: 4, memory: true },
};
export { startup };
