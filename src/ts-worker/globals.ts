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
  private _store: Partial<Record<number, T>>;
  constructor() {
    this._store = {};
  }

  public create(id: number, data: T) {
    this._store[id] = data;
  }

  public delete(id: number): void {
    delete this._store[id];
  }

  public get(id: number): T | undefined {
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
  sourceBuffer: HasMseInWorker extends true ?
    SourceBuffer :
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

export type TimerId = number;
export type RequestId = number;
export type SourceBufferId = number;
export type ResourceId = number;
