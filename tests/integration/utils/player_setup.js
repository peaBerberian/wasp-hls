import { afterAll, afterEach, beforeAll, beforeEach } from "vitest";
import WaspHlsPlayer from "../../../build/es6/ts-main/index.js";
import EmbeddedWorker from "../../../build/embedded/worker.js";
import EmbeddedWasm from "../../../build/embedded/wasm.js";
import { createLivePackagerClient } from "../../utils/live_packager.js";
import sleep from "../../utils/sleep.js";
import { trackPlayerDiagnostics } from "../../utils/player_test_tools.js";

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
  { packageLiveContent, playerConfig, createWorker, concurrent = false } = {
    packageLiveContent: false,
    playerConfig: undefined,
    createWorker: undefined,
  },
) {
  const liveClient = createLivePackagerClient(
    concurrent
      ? packageLiveContent?.emitProgramDateTime
        ? 3002
        : 3001
      : undefined,
  );
  const contexts = new WeakMap();
  const ctx = {
    player: /** @type {WaspHlsPlayer} */ (null),
    videoElement: document.createElement("video"),
    lastPlayerError: null,
    liveInfo: null,
    workerHandle: null,
  };

  beforeAll(
    async () => {
      if (!concurrent) document.body.appendChild(ctx.videoElement);
      if (packageLiveContent) {
        if (packageLiveContent === true) {
          await liveClient.startLivePackager();
        } else {
          await liveClient.startLivePackagerWithOptions(packageLiveContent);
        }
        const readyInfos = await liveClient.waitForPackagerReady();
        ctx.liveInfo = { ...readyInfos };
        await sleep(10000);
      }
    },
    packageLiveContent ? (3600 / 2) * 1000 : undefined,
  );

  afterAll(async () => {
    if (!concurrent) document.body.removeChild(ctx.videoElement);
    if (packageLiveContent) {
      await liveClient.stopLivePackager();
    }
  });

  beforeEach((testContext) => {
    const target = concurrent
      ? { ...ctx, videoElement: document.createElement("video") }
      : ctx;
    contexts.set(testContext, target);
    if (concurrent) document.body.appendChild(target.videoElement);
    target.lastPlayerError = null;
    target.player = new WaspHlsPlayer(target.videoElement, playerConfig);
    target.workerHandle = createWorker?.() ?? { url: EmbeddedWorker };
    target.player.initialize({
      workerUrl: target.workerHandle.url,
      wasmUrl: EmbeddedWasm,
    });
    target.player.addEventListener("error", (error) => {
      target.lastPlayerError = error;
    });
    const diagnostics = trackPlayerDiagnostics(
      target.player,
      target.videoElement,
      () => target.lastPlayerError,
    );
    let report;
    target.saveDiagnostics = () => {
      report = diagnostics.finish();
    };
    testContext.onTestFailed(() => {
      console.error(
        "Player diagnostics: " +
          JSON.stringify({
            test: testContext.task.name,
            liveInfo: target.liveInfo,
            ...report,
          }),
      );
    });
  });

  afterEach((testContext) => {
    const target = contexts.get(testContext);
    if (!target) return;
    target.saveDiagnostics();
    target.player.dispose();
    target.videoElement.removeAttribute("src");
    target.workerHandle?.dispose?.();
    target.workerHandle = null;
    if (concurrent) target.videoElement.remove();
    contexts.delete(testContext);
  });

  return Object.assign(ctx, {
    forTest: (testContext) => contexts.get(testContext) ?? ctx,
  });
}
