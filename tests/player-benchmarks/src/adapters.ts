import Hls from "hls.js";
import shaka from "shaka-player";
import Wasp from "../../../build/es6/ts-main/index.js";
import wasm from "../../../build/embedded/wasm.js";
import worker from "../../../build/embedded/worker.js";

export type PlayerName = "wasp" | "shaka" | "hlsjs";
export interface Adapter {
  load(url: string): Promise<unknown>;
  dispose(): void | Promise<unknown>;
}
interface AdapterOptions {
  bufferGoal?: number;
  retainBackBuffer?: boolean;
  segmentTimeoutMs?: number;
}

// Only initialization and public commands belong here. Measurements use the
// same media-element observations for all players, not their ready promises.
export async function createAdapter(
  name: PlayerName,
  video: HTMLVideoElement,
  fail: (error: unknown) => void,
  {
    bufferGoal,
    retainBackBuffer = false,
    segmentTimeoutMs,
  }: AdapterOptions = {},
): Promise<Adapter> {
  if (name === "wasp") {
    const player = new Wasp(video, {
      ...(bufferGoal === undefined ? {} : { bufferGoal }),
      ...(segmentTimeoutMs === undefined
        ? {}
        : { segmentRequestTimeout: segmentTimeoutMs }),
    });
    player.addEventListener("error", fail);
    try {
      await player.initialize({ wasmUrl: wasm, workerUrl: worker });
    } catch (error) {
      player.dispose();
      throw error;
    }
    return {
      load: async (url) => player.load(url),
      dispose: () => player.dispose(),
    };
  }
  if (name === "shaka") {
    shaka.polyfill.installAll();
    const player = new shaka.Player();
    if (bufferGoal !== undefined)
      player.configure({
        streaming: {
          bufferingGoal: bufferGoal,
          ...(retainBackBuffer ? { bufferBehind: Infinity } : {}),
        },
      });
    if (segmentTimeoutMs !== undefined)
      player.configure({
        streaming: {
          retryParameters: {
            timeout: segmentTimeoutMs,
            connectionTimeout: segmentTimeoutMs,
            stallTimeout: segmentTimeoutMs,
          },
        },
      });
    player.addEventListener("error", (event: Event & { detail?: unknown }) =>
      fail(event.detail),
    );
    try {
      await player.attach(video);
    } catch (error) {
      await player.destroy();
      throw error;
    }
    return { load: (url) => player.load(url), dispose: () => player.destroy() };
  }
  const player = new Hls({
    debug: false,
    // The ES-module distribution cannot inject its own UMD worker.
    workerPath: "/hls.worker.js",
    ...(retainBackBuffer ? { backBufferLength: Infinity } : {}),
    ...(segmentTimeoutMs !== undefined
      ? {
          fragLoadPolicy: {
            default: {
              ...Hls.DefaultConfig.fragLoadPolicy.default,
              maxTimeToFirstByteMs: segmentTimeoutMs,
              maxLoadTimeMs: segmentTimeoutMs,
            },
          },
        }
      : {}),
    ...(bufferGoal === undefined
      ? {}
      : {
          maxBufferLength: bufferGoal,
          maxMaxBufferLength: bufferGoal,
          maxBufferSize: 0,
        }),
  });
  player.on(Hls.Events.ERROR, (_event, data) => {
    if (data.fatal || data.event === "demuxerWorker") fail(data.details);
  });
  const attached = new Promise<void>((resolve) =>
    player.once(Hls.Events.MEDIA_ATTACHED, () => resolve()),
  );
  player.attachMedia(video);
  await attached;
  return {
    load: async (url) => player.loadSource(url),
    dispose: () => player.destroy(),
  };
}
