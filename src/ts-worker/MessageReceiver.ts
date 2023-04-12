import assertNever from "../ts-common/assertNever";
import logger from "../ts-common/logger";
import noop from "../ts-common/noop";
import { SourceBufferOperation } from "../ts-common/QueuedSourceBuffer";
import timeRangesToFloat64Array from "../ts-common/timeRangesToFloat64Array";
import {
  MainMessage,
  InitializationErrorCode,
  WaspHlsPlayerConfig,
  MainMessageType,
  WorkerMessageType,
} from "../ts-common/types";
import initializeWasm, {
  JsTimeRanges,
  MediaObservation,
  MediaType,
  OtherErrorCode,
  PushedSegmentErrorCode,
  StartingPosition,
} from "../wasm/wasp_hls";
import { stopObservingPlayback } from "./bindings";
import {
  cachedCodecsSupport,
  ContentInfo,
  playerInstance,
  updateDispatcherConfig,
  WorkerInitializationOptions,
} from "./globals";
import postMessageToMain from "./postMessage";

let wasInitializedCalled = false;

export default function MessageReceiver() {
  let initializationProm: Promise<void> | undefined;
  onmessage = onMainMessage;
  function onMainMessage(evt: MessageEvent<MainMessage>) {
    if (evt.origin !== "") {
      logger.error("Unexpected trans-origin message");
      return;
    }
    const { data } = evt;
    if (
      typeof data !== "object" ||
      data === null ||
      typeof data.type === "undefined"
    ) {
      logger.error("unexpected main message");
      return;
    }

    if (
      data.type !== MainMessageType.Initialization &&
      data.type !== MainMessageType.DisposePlayer &&
      initializationProm !== undefined
    ) {
      initializationProm
        .then(() => {
          // TODO perhaps some intelligence could be put here to avoid
          // loading contents that have finally been stopped. No hurry though,
          // this should be rare enough and the only issue would be performance
          // and immediately aborted requests.
          onMainMessage(evt);
        })
        .catch(noop);
      return;
    }

    switch (data.type) {
      case MainMessageType.Initialization:
        if (wasInitializedCalled) {
          return handleInitializationError(
            "Worker initialization already done",
            InitializationErrorCode.AlreadyInitializedError
          );
        }
        logger.setLevel(data.value.logLevel);
        wasInitializedCalled = true;
        const { wasmUrl, hasMseInWorker, canDemuxMpeg2Ts, initialBandwidth } =
          data.value;
        initializationProm = initialize(wasmUrl, data.value.initialConfig, {
          hasMseInWorker,
          canDemuxMpeg2Ts,
          initialBandwidth,
        })
          .then(() => {
            initializationProm = undefined;
          })
          .catch((err) => {
            initializationProm = undefined;
            handleInitializationError(
              err,
              InitializationErrorCode.WasmRequestError
            );
          });
        break;

      case MainMessageType.DisposePlayer:
        dispose();
        break;

      case MainMessageType.LoadContent: {
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
        let startingPosition;
        if (data.value.startingPosition !== undefined) {
          startingPosition = new StartingPosition(
            data.value.startingPosition.startingType,
            data.value.startingPosition.position
          );
        }
        dispatcher.load_content(data.value.url, startingPosition);
        break;
      }

      case MainMessageType.StopContent: {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return postUnitializedWorkerError(data.value.contentId);
        }
        const contentInfo = playerInstance.getContentInfo();
        if (
          contentInfo === null ||
          contentInfo.contentId !== data.value.contentId
        ) {
          return;
        }
        try {
          dispatcher.stop();
        } catch (err) {
          const error = err instanceof Error ? err : "Unknown Error";
          logger.error("Error: when stopping the content:", error);
        }
        postMessageToMain({
          type: WorkerMessageType.ContentStopped,
          value: {
            contentId: data.value.contentId,
          },
        });
        break;
      }

      case MainMessageType.MediaSourceStateChanged: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.on_media_source_state_change(data.value.state);
        break;
      }

      case MainMessageType.SourceBufferOperationSuccess: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        const buffered = new JsTimeRanges(data.value.buffered);
        dispatcher.on_source_buffer_update(data.value.sourceBufferId, buffered);
        break;
      }

      case MainMessageType.MediaObservation: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }

        let audioSbBuffered: JsTimeRanges | undefined;
        let videoSbBuffered: JsTimeRanges | undefined;

        if (playerInstance.hasMseInWorker() === true) {
          for (const sourceBufferInfo of contentInfo.mediaSourceObj
            .sourceBuffers) {
            const timeRange =
              sourceBufferInfo.sourceBuffer?.getBufferedRanges();
            if (timeRange !== undefined) {
              const toFloat64 = timeRangesToFloat64Array(timeRange);
              const bufRange = new JsTimeRanges(toFloat64);
              if (sourceBufferInfo.mediaType === MediaType.Audio) {
                audioSbBuffered = bufRange;
              } else if (sourceBufferInfo.mediaType === MediaType.Video) {
                videoSbBuffered = bufRange;
              }
            }
          }
        } else if (data.value.sourceBuffersBuffered !== null) {
          const sbBuffered = data.value.sourceBuffersBuffered;
          for (const sourceBufferInfo of contentInfo.mediaSourceObj
            .sourceBuffers) {
            const element = sbBuffered[sourceBufferInfo.id];
            if (element !== undefined) {
              const bufRange = new JsTimeRanges(element);
              if (sourceBufferInfo.mediaType === MediaType.Audio) {
                audioSbBuffered = bufRange;
              } else if (sourceBufferInfo.mediaType === MediaType.Video) {
                videoSbBuffered = bufRange;
              }
            }
          }
        }
        const bufferedTimeRange = new JsTimeRanges(data.value.buffered);
        const mediaObservation = new MediaObservation(
          data.value.reason,
          data.value.currentTime,
          data.value.readyState,
          bufferedTimeRange,
          data.value.paused,
          data.value.seeking,
          data.value.ended,
          data.value.duration,
          audioSbBuffered,
          videoSbBuffered
        );
        dispatcher.on_playback_tick(mediaObservation);
        break;
      }

      case MainMessageType.CreateMediaSourceError: {
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

        // NOTE: should we go through the dispatcher here? I don't know but we
        // do in other very similar cases.
        postMessageToMain({
          type: WorkerMessageType.Error,
          value: {
            contentId: contentInfo.contentId,
            message:
              "Error while creating the `MediaSource`: " + data.value.message,
            errorInfo: {
              type: "other-error",
              value: {
                code: OtherErrorCode.MediaSourceAttachmentError,
              },
            },
          },
        });
        break;
      }

      case MainMessageType.UpdateMediaSourceDurationError: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        logger.error("Error: when setting the MediaSource's duration");
        break;
      }

      case MainMessageType.CreateSourceBufferError: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }

        dispatcher.on_source_buffer_creation_error(
          data.value.sourceBufferId,
          data.value.code,
          data.value.message
        );
        break;
      }

      case MainMessageType.UpdateWantedSpeed: {
        const dispatcher = playerInstance.getDispatcher();
        const contentInfo = playerInstance.getContentInfo();
        if (
          dispatcher === null ||
          contentInfo === null ||
          contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId
        ) {
          return;
        }
        dispatcher.set_wanted_speed(data.value.wantedSpeed);
        break;
      }

      case MainMessageType.UpdateLoggerLevel:
        logger.setLevel(data.value);
        break;

      case MainMessageType.UpdateConfig: {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return;
        }
        updateDispatcherConfig(dispatcher, data.value);
        break;
      }

      case MainMessageType.SetAudioTrack: {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return postUnitializedWorkerError(data.value.contentId);
        }
        const contentInfo = playerInstance.getContentInfo();
        if (
          contentInfo === null ||
          contentInfo.contentId !== data.value.contentId
        ) {
          return;
        }
        dispatcher.set_audio_track(data.value.trackId ?? undefined);
        break;
      }

      case MainMessageType.LockVariant: {
        const dispatcher = playerInstance.getDispatcher();
        if (dispatcher === null) {
          return postUnitializedWorkerError(data.value.contentId);
        }
        const contentInfo = playerInstance.getContentInfo();
        if (
          contentInfo === null ||
          contentInfo.contentId !== data.value.contentId
        ) {
          return;
        }
        if (data.value.variantId === null) {
          dispatcher.unlock_variant();
        } else {
          dispatcher.lock_variant(data.value.variantId);
        }
        break;
      }

      case MainMessageType.CodecsSupportUpdate:
        {
          const { mimeTypes } = data.value;
          const keys = Object.keys(mimeTypes);
          if (cachedCodecsSupport.size + keys.length > 50) {
            cachedCodecsSupport.clear();
          }
          for (const key of keys) {
            const value = mimeTypes[key];
            if (value !== undefined) {
              cachedCodecsSupport.set(key, value);
            }
          }
          const dispatcher = playerInstance.getDispatcher();
          if (dispatcher !== null) {
            dispatcher.on_codecs_support_update();
          }
        }
        break;

      case MainMessageType.SourceBufferOperationError:
        {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (
            dispatcher === null ||
            contentInfo === null ||
            contentInfo.mediaSourceObj?.mediaSourceId !==
              data.value.mediaSourceId
          ) {
            return;
          }
          if (data.value.operation === SourceBufferOperation.Remove) {
            dispatcher.on_remove_buffer_error(data.value.sourceBufferId);
          } else if (data.value.isBufferFull) {
            dispatcher.on_append_buffer_error(
              data.value.sourceBufferId,
              PushedSegmentErrorCode.BufferFull
            );
          } else {
            dispatcher.on_append_buffer_error(
              data.value.sourceBufferId,
              PushedSegmentErrorCode.UnknownError
            );
          }
        }
        break;

      default:
        assertNever(data);
    }
  }
}

function handleInitializationError(
  err: unknown,
  code: InitializationErrorCode
) {
  let message: string | undefined;
  if (typeof err === "string") {
    message = err;
  } else if (err instanceof Error) {
    message = err.message;
  }
  postMessageToMain({
    type: WorkerMessageType.InitializationError,
    value: {
      code,
      message,
    },
  });
}

function postUnitializedWorkerError(contentId: string): void {
  postMessageToMain({
    type: WorkerMessageType.Error,
    value: {
      contentId,
      message: "Error: Worker not initialized.",
      errorInfo: {
        type: "unitialized",
        value: {},
      },
    },
  });
}

function initialize(
  wasmUrl: string,
  config: WaspHlsPlayerConfig,
  opts: WorkerInitializationOptions
): Promise<void> {
  return initializeWasm(fetch(wasmUrl))
    .then((wasm) => {
      playerInstance.start(wasm, config, opts);
      postMessageToMain({ type: WorkerMessageType.Initialized, value: null });
    })
    .catch((err) => {
      throw err;
    });
}

function dispose() {
  stopObservingPlayback();
  playerInstance.dispose();
}
