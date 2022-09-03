import init,{
  Dispatcher,
} from "../wasm/wasp_hls";
import { stopObservingPlayback } from "./bindings";
import {
  PlayerId,
  PlayerInstanceInfo,
  playersStore,
} from "./globals";

enum InitializationStatus {
  Unloaded,
  Loading,
  Loaded,
  Errored,
}

let initStatus: InitializationStatus = InitializationStatus.Unloaded;

export default class JsWaspHlsPlayer {
  private _playerId: PlayerId;
  private _playerInstanceInfo: PlayerInstanceInfo;

  constructor(videoElement: HTMLVideoElement) {
    if (initStatus !== InitializationStatus.Loaded) {
      throw new Error("WebAssembly not yet loaded.");
    }
    let playerId = 0;
    while (playersStore.get(playerId) !== undefined) {
      playerId++;
    }
    this._playerId = playerId;
    const dispatcher = new Dispatcher(playerId);
    const playerObj: PlayerInstanceInfo = {
      dispatcher,
      videoElement,
      mediaSourceObj: null,
      observationsObj: null,
      isDetroyed: false,
    };
    playersStore.create(playerId, playerObj);
    this._playerInstanceInfo = playerObj;
  }

  public loadContent(url: string) {
    if (this._playerInstanceInfo.isDetroyed) {
      throw new Error("The Player is disposed.");
    }
    this._playerInstanceInfo.dispatcher.load_content(url);
  }

  public stop() {
    this._playerInstanceInfo.dispatcher.stop();
  }

  public dispose() {
    this._playerInstanceInfo.dispatcher.stop();
    this._playerInstanceInfo.dispatcher.free();
    this._playerInstanceInfo.isDetroyed = true;
    stopObservingPlayback(this._playerId);
    playersStore.dispose(this._playerId);
  }

  public static async initialize(url: string) : Promise<void> {
    if (initStatus !== InitializationStatus.Unloaded) {
      switch (initStatus) {
        case InitializationStatus.Loading:
          throw new Error("Player already loading");
        case InitializationStatus.Loaded:
          return Promise.resolve();
      }
    }
    initStatus = InitializationStatus.Loading;
    try {
      await init(fetch(url));
      initStatus = InitializationStatus.Loaded;
    } catch (err) {
      initStatus = InitializationStatus.Errored;
      throw err;
    }
  }
}
