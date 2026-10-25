import { loadContent } from "./startup.ts";
import type { ScenarioContext } from "./context.ts";

export default async function reload(context: ScenarioContext) {
  context.video.pause();
  const times = await loadContent(context, "reload");
  context.metrics.reloadToFrameMs = times.firstFrame - times.start;
}
