import {
  MainMessage,
  WorkerErrorCode,
} from "../ts-common/types";
import initializeWasm, {
  Dispatcher,
  MediaObservation,
} from "../wasm/wasp_hls";
import { stopObservingPlayback } from "./bindings";
import {
  PlayerInstanceInfo,
  playersStore,
} from "./globals";
import postMessageToMain from "./postMessage";

let wasInitializedCalled = false;
let playerInstanceInfo : PlayerInstanceInfo | null = null;

export default function MessageReceiver() {
  onmessage = function(evt: MessageEvent<MainMessage>) {
    if (evt.origin !== "") {
      console.error("Unexpected trans-origin message");
      return;
    }
    const { data } = evt;
    if (typeof data !== "object" || data === null || typeof data.type !== "string") {
      console.error("unexpected main message");
      return;
    }

    switch (data.type) {

      case "init":
        if (wasInitializedCalled) {
          return handleError(
            "Worker initialization already done",
            WorkerErrorCode.AlreadyInitializedError
          );
        }
        wasInitializedCalled = true;
        const { wasmUrl, hasWorkerMse } = data.value;
        initialize(wasmUrl, hasWorkerMse)
          .catch((err) => handleError(err, WorkerErrorCode.WasmInitializationError));
        break;

      case "dispose":
        dispose();
        break;

      case "load":
        if (playerInstanceInfo === null) {
          handleError(
            "Loading content in an Uninitialized player",
            WorkerErrorCode.UnitializedLoadError
          );
          return;
        }
        playerInstanceInfo.dispatcher.load_content(data.value.url);
        break;

      case "stop":
        if (playerInstanceInfo === null) {
          handleError(
            "Stopping content in an Uninitialized player",
            WorkerErrorCode.UnitializedStopError
          );
          return;
        }
        playerInstanceInfo.dispatcher.stop();
        break;

      case "media-source-state-changed":
        if (
          playerInstanceInfo === null ||
          playerInstanceInfo.mediaSourceObj === undefined ||
          playerInstanceInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        playerInstanceInfo.dispatcher
          .on_media_source_state_change(data.value.state);
        break;

      case "source-buffer-updated":
        if (
          playerInstanceInfo === null ||
          playerInstanceInfo.mediaSourceObj === undefined ||
          playerInstanceInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        playerInstanceInfo.dispatcher.on_source_buffer_update(data.value.sourceBufferId);
        break;


      case "observation":
        if (
          playerInstanceInfo === null ||
          playerInstanceInfo.mediaSourceObj === undefined ||
          playerInstanceInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        const mediaObservation = new MediaObservation(
          data.value.reason,
          data.value.currentTime,
          data.value.readyState,
          data.value.buffered,
          data.value.paused,
          data.value.seeking
        );
        playerInstanceInfo.dispatcher.on_playback_tick(mediaObservation);
        break;
    }
  };
}

function handleError(err: unknown, code: WorkerErrorCode) {
  let message : string | undefined;
  if (typeof err === "string") {
    message = err;
  } else if (err instanceof Error) {
    message = err.message;
  }
  postMessageToMain({
    type: "error",
    value: {
      code,
      message,
    },
  });
}

async function initialize(wasmUrl: string, hasWorkerMse: boolean) {
  await initializeWasm(fetch(wasmUrl));
  let playerId = 0;
  while (playersStore.get(playerId) !== undefined) {
    playerId++;
  }
  const dispatcher = new Dispatcher(playerId);
  const playerObj: PlayerInstanceInfo = {
    id: playerId,
    hasWorkerMse,
    dispatcher,
    mediaSourceObj: null,
    observationsObj: null,
    isDetroyed: false,
  };
  playersStore.create(playerId, playerObj);
  playerInstanceInfo = playerObj;
  postMessageToMain({
    type: "initialized",
    value: null,
  });
}

function dispose() {
  if (playerInstanceInfo !== null) {
    playerInstanceInfo.dispatcher.stop();
    playerInstanceInfo.dispatcher.free();
    playerInstanceInfo.isDetroyed = true;
    stopObservingPlayback(playerInstanceInfo.id);
    playersStore.dispose(playerInstanceInfo.id);
    playerInstanceInfo = null;
  }
}
