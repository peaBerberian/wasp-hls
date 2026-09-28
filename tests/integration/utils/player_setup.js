import { afterAll, afterEach, beforeAll, beforeEach } from "vitest";
import WaspHlsPlayer from "../../../build/es6/ts-main/index.js";
import EmbeddedWorker from "../../../build/embedded/worker.js";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import {
  startLivePackager,
  startLivePackagerWithOptions,
  stopLivePackager,
  waitForPackagerReady,
} from "../../utils/live_packager.js";
import sleep from "../../utils/sleep.js";

/**
 * Registers standard beforeAll/afterAll/beforeEach/afterEach hooks for tests
 * and returns a context object whose properties are kept up-to-date by those
 * hooks.
 *
 * Usage:
 *   const ctx = setupPlayer();
 *   it("my test", () => { ctx.player.load(...) });
 */
export default function setupPlayer(
  { packageLiveContent, playerConfig, createWorker } = {
    packageLiveContent: false,
    playerConfig: undefined,
    createWorker: undefined,
  },
) {
  const ctx = {
    player: /** @type {WaspHlsPlayer} */ (null),
    videoElement: document.createElement("video"),
    lastPlayerError: null,
    liveInfo: null,
    workerHandle: null,
  };

  beforeAll(
    async () => {
      document.body.appendChild(ctx.videoElement);
      if (packageLiveContent) {
        if (packageLiveContent === true) {
          await startLivePackager();
        } else {
          await startLivePackagerWithOptions(packageLiveContent);
        }
        const readyInfos = await waitForPackagerReady();
        ctx.liveInfo = { ...readyInfos };
        await sleep(10000);
      }
    },
    packageLiveContent ? (3600 / 2) * 1000 : undefined,
  );

  afterAll(async () => {
    document.body.removeChild(ctx.videoElement);
    if (packageLiveContent) {
      await stopLivePackager();
    }
  });

  beforeEach(() => {
    ctx.lastPlayerError = null;
    ctx.player = new WaspHlsPlayer(ctx.videoElement, playerConfig);
    ctx.workerHandle = createWorker?.() ?? { url: EmbeddedWorker };
    ctx.player.initialize({
      workerUrl: ctx.workerHandle.url,
      wasmUrl: EmbeddedWasm,
    });
    ctx.player.addEventListener("error", (error) => {
      ctx.lastPlayerError = error;
    });
  });

  afterEach(() => {
    ctx.player.dispose();
    ctx.videoElement.removeAttribute("src");
    ctx.workerHandle?.dispose?.();
    ctx.workerHandle = null;
  });

  return ctx;
}
