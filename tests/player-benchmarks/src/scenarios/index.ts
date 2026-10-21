import startup from "./startup.ts";
import seek from "./seek.ts";
import reload from "./reload.ts";
import playback from "./playback.ts";
import type { ScenarioContext } from "./context.ts";

// This local registry is intentionally small, not a public scenario API.
export const scenarios: Record<
  string,
  {
    afterStartup?(context: ScenarioContext): Promise<void>;
    bufferGoal?: number;
    pressure?: boolean;
  }
> = {
  startup: {},
  seek: { afterStartup: seek },
  reload: { afterStartup: reload },
  playback: { afterStartup: playback, bufferGoal: 4 },
  pressure: { afterStartup: playback, bufferGoal: 4, pressure: true },
  "large-playlist": {},
};
export { startup };
