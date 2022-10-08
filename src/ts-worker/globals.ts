import { numberIdGenerator } from "../ts-common/idGenerator";
import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import { Dispatcher } from "../wasm/wasp_hls";

export interface WorkerInfo {
  dispatcher: Dispatcher;
  content: ContentInfo | null;
}

class PlayerInstance {
  public hasWorkerMse: boolean | undefined;
  private  _instanceInfo: WorkerInfo | null;
  constructor() {
    this._instanceInfo = null;
    this.hasWorkerMse = undefined;
  }

  public start(hasWorkerMse: boolean) {
    this.hasWorkerMse = hasWorkerMse;
    this._instanceInfo = {
      dispatcher: new Dispatcher(),
      content: null,
    };
  }

  public dispose(): void {
    this._instanceInfo?.dispatcher.free();
    jsMemoryResources.freeEverything();
    requestsStore.freeEverything();
  }

  public changeContent(
    content: ContentInfo
  ) {
    if (this._instanceInfo === null) {
      // TODO
      console.error();
      return ;
    }
    jsMemoryResources.freeEverything();
    requestsStore.freeEverything();
    this._instanceInfo.content = content;
  }

  public getDispatcher(): Dispatcher | null {
    return this._instanceInfo === null ?
      null :
      this._instanceInfo.dispatcher;
  }

  public getContentInfo(): ContentInfo | null {
    return this._instanceInfo === null ?
      null :
      this._instanceInfo.content;
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
  sourceBuffer: HasMseInWorker extends true ?
    QueuedSourceBuffer :
    null;
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
  mediaSourceObj: WorkerMediaSourceInstanceInfo | MainMediaSourceInstanceInfo | null;
  observationsObj: {
    removeEventListeners: () => void;
    timeoutId: number | undefined;
  } | null;
}

export const playerInstance = new PlayerInstance();
export const jsMemoryResources = new GenericStore<Uint8Array>();
export const requestsStore = new GenericStore<RequestObject>();

export function getMediaSourceObj(
) : MainMediaSourceInstanceInfo | WorkerMediaSourceInstanceInfo | undefined {
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
