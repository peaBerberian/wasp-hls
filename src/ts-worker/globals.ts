import { numberIdGenerator } from "../ts-common/idGenerator";
import logger from "../ts-common/logger";
import type QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import type { SourceBufferId, WaspHlsPlayerConfig } from "../ts-common/types";
import type Transmuxer from "../ts-transmux";
import { Dispatcher, type InitOutput, type MediaType } from "../wasm/wasp_hls";

export interface WorkerInitializationOptions {
  hasMseInWorker: boolean;
  canDemuxMpeg2Ts: boolean;
  /**
   * An initial bandwidth estimate which will be relied on initially, in bits
   * per second.
   */
  initialBandwidth: number;
}

class PlayerInstance {
  private _instanceInfo: WorkerInfo | null;
  constructor() {
    this._instanceInfo = null;
  }

  public hasMseInWorker(): boolean | undefined {
    return this._instanceInfo?.hasMseInWorker;
  }

  public canDemuxMpeg2Ts(): boolean | undefined {
    return this._instanceInfo?.canDemuxMpeg2Ts;
  }

  public start(
    wasm: InitOutput,
    config: WaspHlsPlayerConfig,
    opts: WorkerInitializationOptions,
  ) {
    const dispatcher = new Dispatcher(opts.initialBandwidth);
    updateDispatcherConfig(dispatcher, config);
    this._instanceInfo = {
      wasm,
      dispatcher,
      content: null,
      hasMseInWorker: opts.hasMseInWorker,
      canDemuxMpeg2Ts: opts.canDemuxMpeg2Ts,
    };
  }

  public dispose(): void {
    this._instanceInfo?.dispatcher.free();
    jsMemoryResources.freeEverything();
    requestsStore.freeEverything();
  }

  public changeContent(content: ContentInfo) {
    if (this._instanceInfo === null) {
      logger.error(
        "Worker: Cannot call `changeContent`: `PlayerInstance` not started.",
      );
      return;
    }
    jsMemoryResources.freeEverything();
    requestsStore.freeEverything();
    this._instanceInfo.content = content;
  }

  public getDispatcher(): Dispatcher | null {
    return this._instanceInfo?.dispatcher ?? null;
  }

  public getCurrentWasmMemory(): WebAssembly.Memory | null {
    return this._instanceInfo?.wasm.memory ?? null;
  }

  public getContentInfo(): ContentInfo | null {
    return this._instanceInfo?.content ?? null;
  }
}

class GenericStore<T> {
  private _store: Partial<Record<ResourceId, T>>;
  private _generateId: () => number;
  constructor() {
    this._generateId = numberIdGenerator();
    this._store = {};
  }

  public create(data: T): ResourceId {
    const id = this._generateId();
    this._store[id] = data;
    return id;
  }

  public delete(id: ResourceId): void {
    delete this._store[id];
  }

  public get(id: ResourceId): T | undefined {
    return this._store[id];
  }

  public freeEverything(): void {
    this._store = {};
  }
}

export interface MemoryResource {
  data: Uint8Array;
}

export interface RequestObject {
  abortController: AbortController;
}

export interface SourceBufferInstanceInfo<HasMseInWorker extends boolean> {
  id: SourceBufferId;
  mediaType: MediaType;
  lastInitTimescale: number | undefined;
  sourceBuffer: HasMseInWorker extends true ? QueuedSourceBuffer : null;
  transmuxer: null | Transmuxer;
}

export interface WorkerMediaSourceInstanceInfo {
  type: "worker";
  mediaSourceId: string;
  mediaSource: MediaSource;
  removeEventListeners: () => void;
  nextSourceBufferId: number;
  sourceBuffers: Array<SourceBufferInstanceInfo<true>>;
}

export interface MainMediaSourceInstanceInfo {
  type: "main";
  mediaSourceId: string;
  nextSourceBufferId: number;
  sourceBuffers: Array<SourceBufferInstanceInfo<false>>;
}

export interface ContentInfo {
  contentId: string;
  mediaSourceObj:
    | WorkerMediaSourceInstanceInfo
    | MainMediaSourceInstanceInfo
    | null;
  observationsObj: {
    removeEventListeners: () => void;
    timeoutId: number | undefined;
  } | null;
}

export const cachedCodecsSupport: Map<string, boolean> = new Map();
export const playerInstance = new PlayerInstance();
export const jsMemoryResources = new GenericStore<Uint8Array>();
export const requestsStore = new GenericStore<RequestObject>();

const I32_MAX_VALUE = 2147483647;

export function updateDispatcherConfig(
  dispatcher: Dispatcher,
  config: Partial<WaspHlsPlayerConfig>,
): void {
  if (config.bufferGoal !== undefined) {
    dispatcher.set_buffer_goal(config.bufferGoal);
  }
  if (config.segmentMaxRetry !== undefined) {
    let maxRetry = Math.min(config.segmentMaxRetry, I32_MAX_VALUE);
    maxRetry = Math.max(config.segmentMaxRetry, -1);
    dispatcher.set_segment_request_max_retry(maxRetry);
  }
  if (config.segmentRequestTimeout !== undefined) {
    dispatcher.set_segment_request_timeout(config.segmentRequestTimeout);
  }
  if (config.segmentBackoffBase !== undefined) {
    dispatcher.set_segment_backoff_base(config.segmentBackoffBase);
  }
  if (config.segmentBackoffMax !== undefined) {
    dispatcher.set_segment_backoff_max(config.segmentBackoffMax);
  }
  if (config.multiVariantPlaylistMaxRetry !== undefined) {
    let maxRetry = Math.min(config.multiVariantPlaylistMaxRetry, I32_MAX_VALUE);
    maxRetry = Math.max(config.multiVariantPlaylistMaxRetry, -1);
    dispatcher.set_multi_variant_playlist_request_max_retry(maxRetry);
  }
  if (config.multiVariantPlaylistRequestTimeout !== undefined) {
    dispatcher.set_multi_variant_playlist_request_timeout(
      config.multiVariantPlaylistRequestTimeout,
    );
  }
  if (config.multiVariantPlaylistBackoffBase !== undefined) {
    dispatcher.set_multi_variant_playlist_backoff_base(
      config.multiVariantPlaylistBackoffBase,
    );
  }
  if (config.multiVariantPlaylistBackoffMax !== undefined) {
    dispatcher.set_multi_variant_playlist_backoff_max(
      config.multiVariantPlaylistBackoffMax,
    );
  }
  if (config.mediaPlaylistMaxRetry !== undefined) {
    let maxRetry = Math.min(config.mediaPlaylistMaxRetry, I32_MAX_VALUE);
    maxRetry = Math.max(config.mediaPlaylistMaxRetry, -1);
    dispatcher.set_media_playlist_request_max_retry(maxRetry);
  }
  if (config.mediaPlaylistRequestTimeout !== undefined) {
    dispatcher.set_media_playlist_request_timeout(
      config.mediaPlaylistRequestTimeout,
    );
  }
  if (config.mediaPlaylistBackoffBase !== undefined) {
    dispatcher.set_media_playlist_backoff_base(config.mediaPlaylistBackoffBase);
  }
  if (config.mediaPlaylistBackoffMax !== undefined) {
    dispatcher.set_media_playlist_backoff_max(config.mediaPlaylistBackoffMax);
  }
}

export function getMediaSourceObj():
  | MainMediaSourceInstanceInfo
  | WorkerMediaSourceInstanceInfo
  | undefined {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return undefined;
  }
  const { mediaSourceObj } = contentInfo;
  if (mediaSourceObj === null) {
    return undefined;
  }
  return mediaSourceObj;
}

export type TimerId = number;
export type RequestId = number;
export type ResourceId = number;

interface WorkerInfo {
  wasm: InitOutput;
  dispatcher: Dispatcher;
  content: ContentInfo | null;
  hasMseInWorker: boolean;
  canDemuxMpeg2Ts: boolean;
}
