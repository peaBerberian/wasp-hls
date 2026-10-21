import { Observations } from "../observations.ts";
import type { ScenarioContext } from "./context.ts";

/** Also used for reload: one definition of readiness across all players/cases. */
export async function loadContent(context: ScenarioContext, phase = "load") {
  const { video, player, url, mark, healthy, fail } = context;
  const observations = new Observations(mark);
  try {
    const load = observations.load(video);
    const start = performance.now();
    mark(`${phase}-call`);
    // Attach a rejection handler before waiting for any media event.
    const loading = player.load(url);
    void loading.catch(fail);
    await healthy(load.canPlay);
    mark("play-call");
    await healthy(video.play());
    await healthy(
      Promise.all([loading, load.metadata, load.playing, load.firstFrame]),
    );
    return {
      start,
      metadata: await load.metadata,
      canPlay: await load.canPlay,
      playing: await load.playing,
      firstFrame: await load.firstFrame,
    };
  } finally {
    observations.dispose();
  }
}

export default async function startup(context: ScenarioContext) {
  const times = await loadContent(context);
  Object.assign(context.metrics, {
    initializeToFirstFrameMs: times.firstFrame - context.initializeStartedAt,
    loadToFirstFrameMs: times.firstFrame - times.start,
    loadToMetadataMs: times.metadata - times.start,
    loadToCanPlayMs: times.canPlay - times.start,
    loadToPlayingMs: times.playing - times.start,
  });
}
