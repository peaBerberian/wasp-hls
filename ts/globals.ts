import { PlayerFrontEnd } from "../wasm/wasp_hls";

class JsMemoryResourceStore {
  private _store: Partial<Record<ResourceId, MemoryResource>>;
  constructor() {
    this._store = {};
  }

  public create(resourceId: ResourceId, playerId: PlayerId, data: Uint8Array) {
    this._store[resourceId] = { playerId, data };
  }

  public delete(resourceId: ResourceId): void {
    delete this._store[resourceId];
  }

  public get(resourceId: ResourceId): Uint8Array | undefined {
    return this._store[resourceId]?.data;
  }

  public freeForPlayer(playerId: PlayerId): void {
    Object.entries(this._store).forEach(([resourceId, resource]) => {
      if (resource?.playerId === playerId) {
        // Should work without the `Number` but TypeScript is happier with it,
        // so well...
        delete this._store[Number(resourceId)];
      }
    });
  }
}

class PlayersStore {
  private _store: Partial<Record<PlayerId, PlayerInstanceInfo>>;
  constructor() {
    this._store = {};
  }

  public create(playerId: PlayerId, instanceInfo: PlayerInstanceInfo): void {
    this._store[playerId] = instanceInfo;
  }


  public get(playerId: PlayerId): PlayerInstanceInfo | undefined {
    return this._store[playerId];
  }

  public dispose(playerId: PlayerId): void {
    jsMemoryResources.freeForPlayer(playerId);
    requestsStore.freeForPlayer(playerId);
    delete this._store[playerId];
  }
}

class RequestsStore {
  private _store: Partial<Record<RequestId, RequestObject>>;
  constructor() {
    this._store = {};
  }

  public create(requestId: RequestId, requestObj: RequestObject): void {
    this._store[requestId] = requestObj;
  }


  public get(requestId: RequestId): RequestObject | undefined {
    return this._store[requestId];
  }

  public delete(requestId: RequestId): void {
    delete this._store[requestId];
  }

  public freeForPlayer(playerId: PlayerId): void {
    Object.entries(this._store).forEach(([requestId, request]) => {
      if (request?.playerId === playerId) {
        request.abortController.abort();

        // Should work without the `Number` but TypeScript is happier with it,
        // so well...
        delete this._store[Number(requestId)];
      }
    });
  }
}

export const jsMemoryResources = new JsMemoryResourceStore();
export const playersStore = new PlayersStore();
export const requestsStore = new RequestsStore();

export type PlayerId = number;
export type RequestId = number;
export type SourceBufferId = number;
export type ResourceId = number;

export interface MemoryResource {
  data: Uint8Array;
  playerId: PlayerId;
}

export interface RequestObject {
  playerId: PlayerId;
  abortController: AbortController;
}

export interface SourceBufferInstanceInfo {
  id: SourceBufferId;
  sourceBuffer: SourceBuffer;
  transmuxer: null | ((input: Uint8Array) => Uint8Array | null);
}

export interface MediaSourceInstanceInfo {
  mediaSource: MediaSource;
  objectURL: string;
  removeEventListeners: () => void;
  nextSourceBufferId: number;
  sourceBuffers: SourceBufferInstanceInfo[];
}

export interface PlayerInstanceInfo {
  player: PlayerFrontEnd;
  videoElement: HTMLVideoElement;
  mediaSourceObj: MediaSourceInstanceInfo | null;
  observationsObj: {
    removeEventListeners: () => void;
    timeoutId: number | undefined;
  } | null;
  isDetroyed: boolean;
}

