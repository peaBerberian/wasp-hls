import initializeWasm, {
  Dispatcher,
} from "../wasm/wasp_hls";
import { stopObservingPlayback } from "./bindings";
import {
  PlayerInstanceInfo,
  playersStore,
} from "./globals";

const enum InitializationStatus {
  Uninitialized = "Uninitialized",
  Initializing = "Initializing",
  Initialized = "Initialized",
  Errored = "errorred",
  Disposed = "disposed",
}

export interface InitializationOptions {
  workerUrl: string;
  wasmUrl: string;
}

export default class JsWaspHlsPlayer {
  public initializationStatus: InitializationStatus;
  public videoElement: HTMLVideoElement;
  private _playerInstanceInfo : PlayerInstanceInfo | null;

  constructor(videoElement: HTMLVideoElement) {
    this.videoElement = videoElement;
    this.initializationStatus = InitializationStatus.Uninitialized;
    this._playerInstanceInfo = null;
  }

  public async initialize(opts: InitializationOptions) : Promise<void> {
    try {
      this.initializationStatus = InitializationStatus.Initializing;
      const { wasmUrl } = opts;
      await initializeWasm(fetch(wasmUrl));
      let playerId = 0;
      while (playersStore.get(playerId) !== undefined) {
        playerId++;
      }
      const dispatcher = new Dispatcher(playerId);
      const playerObj: PlayerInstanceInfo = {
        id: playerId,
        dispatcher,
        videoElement: this.videoElement,
        mediaSourceObj: null,
        observationsObj: null,
        isDetroyed: false,
      };
      playersStore.create(playerId, playerObj);
      this._playerInstanceInfo = playerObj;
      this.initializationStatus = InitializationStatus.Initialized;
    }
    catch (_err) {
      this.initializationStatus = InitializationStatus.Errored;
    }
  }

  public loadContent(url: string) {
    if (this._playerInstanceInfo === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    this._playerInstanceInfo.dispatcher.load_content(url);
  }

  public stop() {
    if (this._playerInstanceInfo === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    this._playerInstanceInfo.dispatcher.stop();
  }

  public dispose() {
    if (this._playerInstanceInfo !== null) {
      this._playerInstanceInfo.dispatcher.stop();
      this._playerInstanceInfo.dispatcher.free();
      this._playerInstanceInfo.isDetroyed = true;
      stopObservingPlayback(this._playerInstanceInfo.id);
      playersStore.dispose(this._playerInstanceInfo.id);
      this._playerInstanceInfo = null;
    }
  }
}
