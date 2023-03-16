import { numberIdGenerator } from "../ts-common/idGenerator";
import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import { WaspHlsPlayerConfig } from "../ts-common/types";
import { Dispatcher, InitOutput } from "../wasm/wasp_hls";

export interface WorkerInfo {
  wasm: InitOutput;
  dispatcher: Dispatcher;
  content: ContentInfo | null;
}

class PlayerInstance {
  public hasWorkerMse: boolean | undefined;
  private _instanceInfo: WorkerInfo | null;
  constructor() {
    this._instanceInfo = null;
    this.hasWorkerMse = undefined;
  }

  public start(
    hasWorkerMse: boolean,
    config: WaspHlsPlayerConfig,
    wasm: InitOutput
  ) {
    this.hasWorkerMse = hasWorkerMse;
    const dispatcher = new Dispatcher();
    updateDispatcherConfig(dispatcher, config);
    this._instanceInfo = {
      wasm,
      dispatcher,
      content: null,
    };
  }

  public dispose(): void {
    this._instanceInfo?.dispatcher.free();
    jsMemoryResources.freeEverything();
    requestsStore.freeEverything();
  }

  public changeContent(content: ContentInfo) {
    if (this._instanceInfo === null) {
      // TODO log error
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
  lastInitTimescale: number | undefined;
  sourceBuffer: HasMseInWorker extends true ? QueuedSourceBuffer : null;
  transmuxer: null | ((input: Uint8Array) => Uint8Array | null);
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

export const playerInstance = new PlayerInstance();
export const jsMemoryResources = new GenericStore<Uint8Array>();
export const requestsStore = new GenericStore<RequestObject>();

export function updateDispatcherConfig(
  dispatcher: Dispatcher,
  config: Partial<WaspHlsPlayerConfig>
): void {
  if (config.bufferGoal !== undefined) {
    dispatcher.set_buffer_goal(config.bufferGoal);
  }
  if (config.segmentRequestTimeout !== undefined) {
    dispatcher.set_segment_request_timeout(
      config.segmentRequestTimeout ?? undefined
    );
  }
  if (config.segmentBackoffBase !== undefined) {
    dispatcher.set_segment_backoff_base(config.segmentBackoffBase);
  }
  if (config.segmentBackoffMax !== undefined) {
    dispatcher.set_segment_backoff_max(config.segmentBackoffMax);
  }
  if (config.multiVariantPlaylistRequestTimeout !== undefined) {
    dispatcher.set_multi_variant_playlist_request_timeout(
      config.multiVariantPlaylistRequestTimeout ?? undefined
    );
  }
  if (config.multiVariantPlaylistBackoffBase !== undefined) {
    dispatcher.set_multi_variant_playlist_backoff_base(
      config.multiVariantPlaylistBackoffBase
    );
  }
  if (config.multiVariantPlaylistBackoffMax !== undefined) {
    dispatcher.set_multi_variant_playlist_backoff_max(
      config.multiVariantPlaylistBackoffMax
    );
  }
  if (config.multiVariantPlaylistRequestTimeout !== undefined) {
    dispatcher.set_media_playlist_request_timeout(
      config.multiVariantPlaylistRequestTimeout ?? undefined
    );
  }
  if (config.multiVariantPlaylistBackoffBase !== undefined) {
    dispatcher.set_media_playlist_backoff_base(
      config.multiVariantPlaylistBackoffBase
    );
  }
  if (config.multiVariantPlaylistBackoffMax !== undefined) {
    dispatcher.set_media_playlist_backoff_max(
      config.multiVariantPlaylistBackoffMax
    );
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
export type SourceBufferId = number;
export type ResourceId = number;
