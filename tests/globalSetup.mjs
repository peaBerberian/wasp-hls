import { fileURLToPath } from "url";
import createContentServer from "./contents/server.mjs";
import { ensureDefaultVodFixtures } from "./contents/vod_fixtures.mjs";

let contentServer;
let isolatedLiveServers = [];

let started = false;

/**
 * Peform actions we want to setup before tests.
 * TODO Share with performance tests?
 */
export async function setup() {
  if (started) {
    return; // already started
  }
  started = true;
  contentServer = createContentServer();
  if (process.env.WASP_HLS_PARALLEL_LIVE === "1") {
    isolatedLiveServers = [1, 2].map((id) =>
      createContentServer({
        port: 3000 + id,
        liveOutputDir: fileURLToPath(
          new URL(`../tmp/testcontents/live-${id}`, import.meta.url),
        ),
        packagerBasePort: 35951 + id * 10,
      }),
    );
  }
  await Promise.all([
    contentServer.listeningPromise,
    ...isolatedLiveServers.map((server) => server.listeningPromise),
  ]);
  await ensureDefaultVodFixtures();
}

/**
 * Peform actions to clean-up after tests.
 */
export async function teardown() {
  await Promise.all([
    contentServer?.close(),
    ...isolatedLiveServers.map((server) => server.close()),
  ]);
  isolatedLiveServers = [];
  started = false;
}
