import createContentServer from "./contents/server.mjs";
import { ensureDefaultVodFixtures } from "./contents/vod_fixtures.mjs";

let contentServer;

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
  await contentServer.listeningPromise;
  await ensureDefaultVodFixtures();
}

/**
 * Peform actions to clean-up after tests.
 */
export async function teardown() {
  await contentServer?.close();
  started = false;
}
