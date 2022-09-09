import {
  MainMessage,
  InitializationErrorCode,
  ContentErrorCode,
} from "../ts-common/types";
import initializeWasm, {
  MediaObservation,
} from "../wasm/wasp_hls";
import { stopObservingPlayback } from "./bindings";
import {
  ContentInfo,
  playerInstance,
} from "./globals";
import postMessageToMain from "./postMessage";

let wasInitializedCalled = false;

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
          return handleInitializationError(
            "Worker initialization already done",
            InitializationErrorCode.AlreadyInitializedError
          );
        }
        wasInitializedCalled = true;
        const { wasmUrl, hasWorkerMse } = data.value;
        initialize(wasmUrl, hasWorkerMse);
        break;

      case "dispose":
        dispose();
        break;

      case "load": {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return postUnitializedWorkerError(data.value.contentId);
        }
        const contentInfo: ContentInfo = {
          contentId: data.value.contentId,
          mediaSourceObj: null,
          observationsObj: null,
        };
        playerInstance.changeContent(contentInfo);
        dispatcher.load_content(data.value.url);
        break;
      }

      case "stop": {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return postUnitializedWorkerError(data.value.contentId);
        }
        dispatcher.stop();
        break;
      }

      case "media-source-state-changed": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null || contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.on_media_source_state_change(data.value.state);
        break;
      }

      case "source-buffer-updated": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null || contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.on_source_buffer_update(data.value.sourceBufferId);
        break;
      }


      case "observation": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null || contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
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
        dispatcher.on_playback_tick(mediaObservation);
        break;
      }

      case "create-media-source-error": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null || contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.stop();

        // TODO re-dispatch ContentError
        break;
      }

      case "update-media-source-duration-error": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        // TODO this should proably not be sent
        console.error("Error: when setting the MediaSource's duration");
        break;
      }

      case "create-source-buffer-error": {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.stop();

        // TODO re-dispatch ContentError
        break;
      }
    }
  };
}

function handleInitializationError(err: unknown, code: InitializationErrorCode) {
  let message : string | undefined;
  if (typeof err === "string") {
    message = err;
  } else if (err instanceof Error) {
    message = err.message;
  }
  postMessageToMain({
    type: "initialization-error",
    value: {
      code,
      message,
    },
  });
}

function postUnitializedWorkerError(contentId: string): void {
  postMessageToMain({
    type: "content-error",
    value: {
      contentId,
      message: "Error: Worker not initialized.",
      code: ContentErrorCode.UnitializedError,
    },
  });
}

function initialize(
  wasmUrl: string,
  hasWorkerMse: boolean
) {
  initializeWasm(fetch(wasmUrl)).then(() => {
    playerInstance.start(hasWorkerMse);
    postMessageToMain({ type: "initialized", value: null });
  }).catch(err => {
    handleInitializationError(err, InitializationErrorCode.WasmRequestError);
  });
}

function dispose() {
  stopObservingPlayback();
  playerInstance.dispose();
}
