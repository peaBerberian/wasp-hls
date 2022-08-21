import init,{
  PlayerFrontEnd,
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
    const player = new PlayerFrontEnd(playerId);
    const playerObj: PlayerInstanceInfo = {
      player,
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
      throw new Error("The player is disposed.");
    }
    this._playerInstanceInfo.player.load_content(url);
  }

  public stop() {
    this._playerInstanceInfo.player.stop();
  }

  public dispose() {
    this._playerInstanceInfo.player.stop();
    this._playerInstanceInfo.player.free();
    this._playerInstanceInfo.isDetroyed = true;
    stopObservingPlayback(this._playerId);
    playersStore.dispose(this._playerId);
  }

  public static async initialize(url: string) : Promise<void> {
    if (initStatus !== InitializationStatus.Unloaded) {
      switch (initStatus) {
        case InitializationStatus.Loading:
          throw new Error("PlayerFrontEnd already loading");
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
