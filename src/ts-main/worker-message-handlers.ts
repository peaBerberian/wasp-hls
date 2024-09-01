import assertNever from "../ts-common/assertNever";
import logger from "../ts-common/logger";
import QueuedSourceBuffer, {
  SourceBufferOperation,
} from "../ts-common/QueuedSourceBuffer";
import timeRangesToFloat64Array from "../ts-common/timeRangesToFloat64Array";
import type {
  AppendBufferWorkerMessage,
  AttachMediaSourceWorkerMessage,
  ClearMediaSourceWorkerMessage,
  CreateMediaSourceWorkerMessage,
  CreateSourceBufferWorkerMessage,
  EndOfStreamWorkerMessage,
  RebufferingEndedWorkerMessage,
  RebufferingStartedWorkerMessage,
  RemoveBufferWorkerMessage,
  SeekWorkerMessage,
  SetMediaSourceDurationWorkerMessage,
  StartPlaybackObservationWorkerMessage,
  StopPlaybackObservationWorkerMessage,
  UpdatePlaybackRateWorkerMessage,
  MediaOffsetUpdateWorkerMessage,
  ErrorWorkerMessage,
  ContentInfoUpdateWorkerMessage,
  ContentStoppedWorkerMessage,
  WarningWorkerMessage,
  MultivariantPlaylistParsedWorkerMessage,
  VariantUpdateWorkerMessage,
  TrackUpdateWorkerMessage,
  FlushWorkerMessage,
  AreTypesSupportedWorkerMessage,
  VariantLockStatusChangeWorkerMessage,
  SourceBufferId,
} from "../ts-common/types";
import { MainMessageType, MediaSourceReadyState } from "../ts-common/types";
import {
  AddSourceBufferErrorCode,
  MediaType,
  OtherErrorCode,
} from "../wasm/wasp_hls";
import type { WaspError } from "./errors";
import {
  WaspMediaPlaylistParsingError,
  WaspMediaPlaylistRequestError,
  WaspMultivariantPlaylistParsingError,
  WaspMultivariantPlaylistRequestError,
  WaspOtherError,
  WaspSegmentParsingError,
  WaspSegmentRequestError,
  WaspSourceBufferCreationError,
  WaspSourceBufferError,
} from "./errors";
import PlaybackObserver from "./observePlayback";
import postMessageToWorker from "./postMessageToWorker";
import type { ContentMetadata } from "./types";
import { clearElementSrc, getErrorInformation } from "./utils";

/**
 * Interval, in milliseconds, at which playback observations are sent to the
 * worker when it is rebuffering.
 */
const PLAYBACK_OBSERVATION_INTERVAL_REBUFFERING = 300;
/**
 * Interval, in milliseconds, at which playback observations are sent to the
 * worker in good playback conditions.
 */
const PLAYBACK_OBSERVATION_INTERVAL_REGULAR = 1000;

/**
 * Handles `AttachMediaSourceWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 */
export function onAttachMediaSourceMessage(
  msg: AttachMediaSourceWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info(
      "API: Ignoring MediaSource attachment due to wrong `contentId`",
    );
    return;
  }

  if (contentMetadata.playbackObserver !== null) {
    contentMetadata.playbackObserver.stop();
    contentMetadata.playbackObserver = null;
  }

  if (msg.value.handle !== undefined) {
    mediaElement.srcObject = msg.value.handle;
  } else if (msg.value.src !== undefined) {
    mediaElement.src = msg.value.src;
  } else {
    throw new Error('Unexpected "attach-media-source" message: missing source');
  }
  contentMetadata.mediaSourceId = msg.value.mediaSourceId;
  contentMetadata.mediaSource = null;
  contentMetadata.disposeMediaSource = () => {
    if (msg.value.src !== undefined) {
      URL.revokeObjectURL(msg.value.src);
    }
  };
  contentMetadata.sourceBuffers = [];
}

/**
 * Handles `SeekWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 */
export function onSeekMessage(
  msg: SeekWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring seek due to wrong `mediaSourceId`");
    return;
  }
  try {
    mediaElement.currentTime = msg.value.position;
  } catch (err) {
    const error = err instanceof Error ? err : "Unknown Error";
    logger.error("Unexpected error while seeking:", error);
  }
}

/**
 * Handles `FlushWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 */
export function onFlushMessage(
  msg: FlushWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring flush due to wrong `mediaSourceId`");
    return;
  }
  try {
    mediaElement.currentTime = mediaElement.currentTime - 0.001;
  } catch (err) {
    const error = err instanceof Error ? err : "Unknown Error";
    logger.error("Unexpected error while flushing:", error);
  }
}

/**
 * Handles `UpdatePlaybackRateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 */
export function onUpdatePlaybackRateMessage(
  msg: UpdatePlaybackRateWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring playback rate update due to wrong `mediaSourceId`",
    );
    return;
  }
  try {
    mediaElement.playbackRate = msg.value.playbackRate;
  } catch (err) {
    const error = err instanceof Error ? err : "Unknown Error";
    logger.error("Unexpected error while changing the playback rate:", error);
  }
}

/**
 * Handles `CreateMediaSourceWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onCreateMediaSourceMessage(
  msg: CreateMediaSourceWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
  worker: Worker,
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info(
      "API: Ignoring MediaSource attachment due to wrong `contentId`",
    );
  } else {
    const { mediaSourceId } = msg.value;
    try {
      if (contentMetadata.disposeMediaSource !== null) {
        contentMetadata.disposeMediaSource();
        contentMetadata.disposeMediaSource = null;
      }
      if (contentMetadata.playbackObserver !== null) {
        contentMetadata.playbackObserver.stop();
        contentMetadata.playbackObserver = null;
      }

      const mediaSource = new MediaSource();

      const disposeMediaSource = bindMediaSource(
        worker,
        mediaSource,
        mediaElement,
        mediaSourceId,
      );
      contentMetadata.mediaSourceId = msg.value.mediaSourceId;
      contentMetadata.mediaSource = mediaSource;
      contentMetadata.disposeMediaSource = disposeMediaSource;
      contentMetadata.sourceBuffers = [];
    } catch (err) {
      const { name, message } = getErrorInformation(
        err,
        "Unknown error when creating the MediaSource",
      );
      postMessageToWorker(worker, {
        type: MainMessageType.CreateMediaSourceError,
        value: { mediaSourceId, message, name },
      });
    }
  }
}

/**
 * Handles `SetMediaSourceDurationWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onUpdateMediaSourceDurationMessage(
  msg: SetMediaSourceDurationWorkerMessage,
  contentMetadata: ContentMetadata | null,
  worker: Worker,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring duration update due to wrong `mediaSourceId`");
    return;
  }
  try {
    if (contentMetadata.mediaSource === null) {
      logger.info("API: Ignoring duration update due to no MediaSource");
    } else {
      contentMetadata.mediaSource.duration = msg.value.duration;
    }
  } catch (err) {
    const { name, message } = getErrorInformation(
      err,
      "Unknown error when updating the MediaSource's duration",
    );
    const { mediaSourceId } = msg.value;
    postMessageToWorker(worker, {
      type: MainMessageType.UpdateMediaSourceDurationError,
      value: { mediaSourceId, message, name },
    });
  }
}

/**
 * Handles `ClearMediaSourceWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 */
export function onClearMediaSourceMessage(
  msg: ClearMediaSourceWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring MediaSource clearing due to wrong `mediaSourceId`",
    );
    return;
  }
  try {
    if (contentMetadata.disposeMediaSource !== null) {
      contentMetadata.disposeMediaSource();
      contentMetadata.disposeMediaSource = null;
    }
    clearElementSrc(mediaElement);
  } catch (err) {
    const error = err instanceof Error ? err : "Unknown Error";
    logger.warn("API: Error when clearing current MediaSource:", error);
  }
}

/**
 * Handles `CreateSourceBufferWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onCreateSourceBufferMessage(
  msg: CreateSourceBufferWorkerMessage,
  contentMetadata: ContentMetadata | null,
  worker: Worker,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring SourceBuffer creation due to wrong `mediaSourceId`",
    );
    return;
  }
  if (contentMetadata.mediaSource === null) {
    postMessageToWorker(worker, {
      type: MainMessageType.CreateSourceBufferError,
      value: {
        mediaSourceId: msg.value.mediaSourceId,
        sourceBufferId: msg.value.sourceBufferId,
        code: AddSourceBufferErrorCode.NoMediaSourceAttached,
        message: "No MediaSource created on the main thread.",
        name: undefined,
      },
    });
    return;
  }
  try {
    const sourceBuffer = contentMetadata.mediaSource.addSourceBuffer(
      msg.value.contentType,
    );
    const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
    contentMetadata.sourceBuffers.push({
      sourceBufferId: msg.value.sourceBufferId,
      queuedSourceBuffer,
    });
  } catch (err) {
    const { name, message } = getErrorInformation(
      err,
      "Unknown error when adding the SourceBuffer to the MediaSource",
    );
    postMessageToWorker(worker, {
      type: MainMessageType.CreateSourceBufferError,
      value: {
        mediaSourceId: msg.value.mediaSourceId,
        sourceBufferId: msg.value.sourceBufferId,
        code: AddSourceBufferErrorCode.UnknownError,
        message,
        name,
      },
    });
  }
}

/**
 * Handles `AppendBufferWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onAppendBufferMessage(
  msg: AppendBufferWorkerMessage,
  contentMetadata: ContentMetadata | null,
  worker: Worker,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring appendBuffer operation due to wrong `mediaSourceId`",
    );
    return;
  }
  const sbObject = contentMetadata.sourceBuffers.find(
    ({ sourceBufferId }) => sourceBufferId === msg.value.sourceBufferId,
  );
  if (sbObject !== undefined) {
    const { mediaSourceId, sourceBufferId } = msg.value;
    try {
      sbObject.queuedSourceBuffer
        .push(msg.value.data)
        .then(() => {
          const buffered = sbObject.queuedSourceBuffer.getBufferedRanges();
          postMessageToWorker(worker, {
            type: MainMessageType.SourceBufferOperationSuccess,
            value: {
              mediaSourceId,
              sourceBufferId,
              buffered: timeRangesToFloat64Array(buffered),
            },
          });
        })
        .catch(handleAppendBufferError);
    } catch (err) {
      handleAppendBufferError(err);
    }
    function handleAppendBufferError(err: unknown): void {
      const { message } = getErrorInformation(
        err,
        "Unknown error when appending data to the SourceBuffer",
      );
      let buffered = new Float64Array([]);
      try {
        if (sbObject !== undefined) {
          buffered = timeRangesToFloat64Array(
            sbObject.queuedSourceBuffer.getBufferedRanges(),
          );
        }
      } catch (_) {
        /* ignore error here */
      }
      postMessageToWorker(worker, {
        type: MainMessageType.SourceBufferOperationError,
        value: {
          mediaSourceId,
          sourceBufferId,
          message,
          operation: SourceBufferOperation.Push,
          isBufferFull:
            err instanceof Error && err.name === "QuotaExceededError",
          buffered,
        },
      });
    }
  }
}

/**
 * Handles `RemoveBufferWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onRemoveBufferMessage(
  msg: RemoveBufferWorkerMessage,
  contentMetadata: ContentMetadata | null,
  worker: Worker,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring removeBuffer operation due to wrong `mediaSourceId`",
    );
    return;
  }
  const sbObject = contentMetadata.sourceBuffers.find(
    ({ sourceBufferId }) => sourceBufferId === msg.value.sourceBufferId,
  );
  if (sbObject !== undefined) {
    const { mediaSourceId, sourceBufferId } = msg.value;
    try {
      sbObject.queuedSourceBuffer
        .removeBuffer(msg.value.start, msg.value.end)
        .then(() => {
          const buffered = sbObject.queuedSourceBuffer.getBufferedRanges();
          postMessageToWorker(worker, {
            type: MainMessageType.SourceBufferOperationSuccess,
            value: {
              mediaSourceId,
              sourceBufferId,
              buffered: timeRangesToFloat64Array(buffered),
            },
          });
        })
        .catch(handleRemoveBufferError);
    } catch (err) {
      handleRemoveBufferError(err);
    }
    function handleRemoveBufferError(err: unknown): void {
      const { message } = getErrorInformation(
        err,
        "Unknown error when removing data to the SourceBuffer",
      );
      let buffered = new Float64Array([]);
      try {
        if (sbObject !== undefined) {
          buffered = timeRangesToFloat64Array(
            sbObject.queuedSourceBuffer.getBufferedRanges(),
          );
        }
      } catch (_) {
        /* ignore error here */
      }
      postMessageToWorker(worker, {
        type: MainMessageType.SourceBufferOperationError,
        value: {
          mediaSourceId,
          sourceBufferId,
          message,
          operation: SourceBufferOperation.Remove,
          isBufferFull: false,
          buffered,
        },
      });
    }
  }
}

/**
 * Handles `StartPlaybackObservationWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onStartPlaybackObservationMessage(
  msg: StartPlaybackObservationWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
  worker: Worker,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring `start-playback-observation` due to wrong `mediaSourceId`",
    );
    return;
  }
  if (contentMetadata.playbackObserver !== null) {
    contentMetadata.playbackObserver.stop();
    contentMetadata.playbackObserver = null;
  }
  contentMetadata.playbackObserver = new PlaybackObserver(
    mediaElement,
    PLAYBACK_OBSERVATION_INTERVAL_REBUFFERING,
  );
  contentMetadata.playbackObserver.addEventListener(
    "newObservation",
    (value) => {
      const sourceBuffersBuffered: Partial<
        Record<SourceBufferId, Float64Array>
      > = {};

      for (const sourceBuffer of contentMetadata.sourceBuffers) {
        const ranges = sourceBuffer.queuedSourceBuffer.getBufferedRanges();
        const toFloat64 = timeRangesToFloat64Array(ranges);
        sourceBuffersBuffered[sourceBuffer.sourceBufferId] = toFloat64;
      }
      postMessageToWorker(worker, {
        type: MainMessageType.MediaObservation,
        value: Object.assign(value, {
          sourceBuffersBuffered,
          mediaSourceId: msg.value.mediaSourceId,
        }),
      });
    },
  );
  contentMetadata.playbackObserver.start();
}

/**
 * Handles `StopPlaybackObservationWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 */
export function onStopPlaybackObservationMessage(
  msg: StopPlaybackObservationWorkerMessage,
  contentMetadata: ContentMetadata | null,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring `stop-playback-observation` due to wrong `mediaSourceId`",
    );
    return;
  }
  if (contentMetadata.playbackObserver !== null) {
    contentMetadata.playbackObserver.stop();
    contentMetadata.playbackObserver = null;
  }
}

/**
 * Handles `EndOfStreamWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 */
export function onEndOfStreamMessage(
  msg: EndOfStreamWorkerMessage,
  contentMetadata: ContentMetadata | null,
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring `end-of-stream` due to wrong `mediaSourceId`");
  } else {
    if (contentMetadata.mediaSource === null) {
      return;
    }
    if (contentMetadata.mediaSource.readyState === "ended") {
      logger.info("Ignoring redundant end-of-stream order");
      return;
    }
    try {
      // TODO Maybe the best here would be a more complex logic to
      // call `endOfStream` at the right time.
      contentMetadata.mediaSource.endOfStream();
    } catch (err) {
      logger.error(
        "Error when calling MediaSource.endOfStream():",
        err instanceof Error ? err.toString() : "Unknown Error",
      );
    }
  }
}

/**
 * Handles `MediaOffsetUpdateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 */
export function onMediaOffsetUpdateMessage(
  msg: MediaOffsetUpdateWorkerMessage,
  contentMetadata: ContentMetadata | null,
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring media offset update due to wrong `contentId`");
    return;
  }
  contentMetadata.mediaOffset = msg.value.offset;
}

/**
 * Handles `RebufferingStartedWorkerMessage` messages.
 * Returns `true` if a new rebuffering period has stared and `false ` either
 * if no rebuffering period has started or if we were already in a rebuffering
 * period.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 * @returns {boolean} - `true` if a new rebuffering period has been entered.
 */
export function onRebufferingStartedMessage(
  msg: RebufferingStartedWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): boolean {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring rebuffering start due to wrong `mediaSourceId`");
    return false;
  }
  contentMetadata.playbackObserver?.updateMinimumObservationInterval(
    PLAYBACK_OBSERVATION_INTERVAL_REBUFFERING,
  );
  if (msg.value.updatePlaybackRate) {
    mediaElement.playbackRate = 0;
  }
  if (!contentMetadata.isRebuffering) {
    contentMetadata.isRebuffering = true;
    return true;
  }
  return false;
}

/**
 * Handles `RebufferingEndedWorkerMessage` messages.
 * Returns `true` if a new rebuffering period has ended and `false ` either
 * if no rebuffering period has ended or if we were not in a rebuffering
 * period before.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {HTMLMediaElement} mediaElement - HTMLMediaElement on which the
 * content plays.
 * @returns {boolean} - `true` if a rebuffering period has been exited.
 */
export function onRebufferingEndedMessage(
  msg: RebufferingEndedWorkerMessage,
  contentMetadata: ContentMetadata | null,
  mediaElement: HTMLMediaElement,
): boolean {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring rebuffering end due to wrong `mediaSourceId`");
    return false;
  }
  contentMetadata.playbackObserver?.updateMinimumObservationInterval(
    PLAYBACK_OBSERVATION_INTERVAL_REGULAR,
  );
  if (mediaElement.playbackRate === 0 && contentMetadata.wantedSpeed !== 0) {
    mediaElement.playbackRate = contentMetadata.wantedSpeed;
  }
  if (contentMetadata.isRebuffering) {
    contentMetadata.isRebuffering = false;
    return true;
  }
  return false;
}

/**
 * Handles `ErrorWorkerMessage` messages.
 * Returns either `null` if the error was ignored or the produced error if it
 * wasn't. Such error may then be used for example for the corresponding event.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {Error|null}
 */
export function onErrorMessage(
  msg: ErrorWorkerMessage,
  contentMetadata: ContentMetadata | null,
): WaspError | null {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring error due to wrong `contentId`");
    return null;
  }
  if (contentMetadata.loadingAborter !== undefined) {
    contentMetadata.loadingAborter.abort(
      new Error("Could not load content due to an error"),
    );
  }

  // Make sure resources are freed
  if (contentMetadata.disposeMediaSource !== null) {
    contentMetadata.disposeMediaSource();
    contentMetadata.disposeMediaSource = null;
  }
  if (contentMetadata.playbackObserver !== null) {
    contentMetadata.playbackObserver.stop();
    contentMetadata.playbackObserver = null;
  }
  contentMetadata.playbackObserver = null;

  const error = formatError(msg);
  contentMetadata.error = error;
  return error;
}

function formatError(
  msg: ErrorWorkerMessage | WarningWorkerMessage,
): WaspError {
  switch (msg.value.errorInfo.type) {
    case "segment-request":
      return new WaspSegmentRequestError(
        msg.value.errorInfo.value,
        msg.value.message,
      );
    case "multi-var-playlist-request":
      return new WaspMultivariantPlaylistRequestError(
        msg.value.errorInfo.value.url,
        msg.value.errorInfo.value.reason,
        msg.value.errorInfo.value.status,
        msg.value.message,
      );
    case "media-playlist-request":
      return new WaspMediaPlaylistRequestError(
        msg.value.errorInfo.value.url,
        msg.value.errorInfo.value.reason,
        msg.value.errorInfo.value.status,
        msg.value.errorInfo.value.mediaType,
        msg.value.message,
      );
    case "other-error":
      return new WaspOtherError(
        msg.value.errorInfo.value.code,
        msg.value.message,
      );
    case "sb-creation":
      return new WaspSourceBufferCreationError(
        msg.value.errorInfo.value.code,
        msg.value.errorInfo.value.mediaType,
        msg.value.message,
      );
    case "multi-var-playlist-parse":
      return new WaspMultivariantPlaylistParsingError(
        msg.value.errorInfo.value.code,
        msg.value.message,
      );
    case "media-playlist-parse":
      return new WaspMediaPlaylistParsingError(
        msg.value.errorInfo.value.mediaType,
        msg.value.errorInfo.value.code,
        msg.value.message,
      );
    case "segment-parse":
      return new WaspSegmentParsingError(
        msg.value.errorInfo.value.code,
        msg.value.errorInfo.value.mediaType,
        msg.value.message,
      );
    case "push-segment-error":
      return new WaspSourceBufferError(
        SourceBufferOperation.Push,
        msg.value.errorInfo.value.code,
        msg.value.errorInfo.value.mediaType,
        msg.value.message,
      );
    case "remove-buffer-error":
      return new WaspSourceBufferError(
        SourceBufferOperation.Remove,
        null,
        msg.value.errorInfo.value.mediaType,
        msg.value.message,
      );
    case "unitialized":
      return new WaspOtherError(OtherErrorCode.Unknown, msg.value.message);
    default:
      assertNever(msg.value.errorInfo);
  }
}

/**
 * Handles `WarningWorkerMessage` messages.
 * Returns either `null` if the error was ignored or the produced error if it
 * wasn't. Such error may then be used for example for the corresponding event.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {Warning|null}
 */
export function onWarningMessage(
  msg: WarningWorkerMessage,
  contentMetadata: ContentMetadata | null,
): WaspError | null {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return null;
  }
  const error = formatError(msg);
  return error;
}

/**
 * Handles `ContentInfoUpdateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - Returns `true` if at least one of the content's info
 * attributes has changed.
 */
export function onContentInfoUpdateMessage(
  msg: ContentInfoUpdateWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return false;
  }
  let hasChanged = false;
  if (msg.value.minimumPosition !== contentMetadata.minimumPosition) {
    contentMetadata.minimumPosition = msg.value.minimumPosition;
    hasChanged = true;
  }
  if (msg.value.maximumPosition !== contentMetadata.maximumPosition) {
    contentMetadata.maximumPosition = msg.value.maximumPosition;
    hasChanged = true;
  }
  if (msg.value.playlistType !== contentMetadata.playlistType) {
    contentMetadata.playlistType = msg.value.playlistType;
    hasChanged = true;
  }
  return hasChanged;
}

/**
 * Handles `MultivariantPlaylistParsedWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - `true` if the message concerned the current content.
 */
export function onMultivariantPlaylistParsedMessage(
  msg: MultivariantPlaylistParsedWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return false;
  }
  contentMetadata.variants = msg.value.variants;
  contentMetadata.audioTracks = msg.value.audioTracks;
  return true;
}

/**
 * Handles `TrackUpdateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - `true` if the message concerned the current content.
 */
export function onTrackUpdateMessage(
  msg: TrackUpdateWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return false;
  }
  if (msg.value.mediaType !== MediaType.Audio) {
    logger.warn("API: track update for a type not handled for now");
    return false;
  }
  contentMetadata.currentAudioTrack = msg.value.audioTrack
    ? {
        id: msg.value.audioTrack.current,
        isSelected: msg.value.audioTrack.isSelected,
      }
    : undefined;
  return true;
}

/**
 * Handles `VariantUpdateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - `true` if the current variant has been updated.
 */
export function onVariantUpdateMessage(
  msg: VariantUpdateWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return false;
  }
  const variant = contentMetadata.variants.find(
    (v) => v.id === msg.value.variantId,
  );
  if (variant === undefined) {
    logger.warn("API: VariantUpdate for an unfound variant");
  }
  if (variant !== contentMetadata.currVariant) {
    contentMetadata.currVariant = variant;
    return true;
  }
  return false;
}

/**
 * Handles `VariantLockStatusChangeWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - `true` if the lock status has changed
 */
export function onVariantLockStatusChangeMessage(
  msg: VariantLockStatusChangeWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return false;
  }
  if (msg.value.lockedVariant === null) {
    if (contentMetadata.lockedVariant !== null) {
      contentMetadata.lockedVariant = null;
      return true;
    }
    return false;
  }

  const variant = contentMetadata.variants.find(
    (v) => v.id === msg.value.lockedVariant,
  );
  if (variant === undefined) {
    logger.warn("API: VariantLockStatusChange for an unfound variant");
    if (contentMetadata.lockedVariant !== null) {
      contentMetadata.lockedVariant = null;
      return true;
    }
    return false;
  }
  if (variant !== contentMetadata.lockedVariant) {
    contentMetadata.lockedVariant = variant;
    return true;
  }
  return false;
}

/**
 * Handles `ContentStoppedWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {boolean} - `true` if the content was effectively stopped.
 */
export function onContentStoppedMessage(
  msg: ContentStoppedWorkerMessage,
  contentMetadata: ContentMetadata | null,
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring `content-stopped` due to wrong `contentId`");
    return false;
  }
  // Make sure resources are freed
  if (contentMetadata.disposeMediaSource !== null) {
    contentMetadata.disposeMediaSource();
    contentMetadata.disposeMediaSource = null;
  }
  if (contentMetadata.playbackObserver !== null) {
    contentMetadata.playbackObserver.stop();
    contentMetadata.playbackObserver = null;
  }
  contentMetadata.playbackObserver = null;
  contentMetadata.loadingAborter?.abort(new Error("Content Stopped"));
  return true;
}

function bindMediaSource(
  worker: Worker,
  mediaSource: MediaSource,
  videoElement: HTMLMediaElement,
  mediaSourceId: string,
): () => void {
  mediaSource.addEventListener("sourceclose", onMediaSourceClose);
  mediaSource.addEventListener("sourceended", onMediaSourceEnded);
  mediaSource.addEventListener("sourceopen", onMediaSourceOpen);

  const objectURL = URL.createObjectURL(mediaSource);
  videoElement.src = objectURL;

  function onMediaSourceEnded() {
    postMessageToWorker(worker, {
      type: MainMessageType.MediaSourceStateChanged,
      value: { mediaSourceId, state: MediaSourceReadyState.Ended },
    });
  }
  function onMediaSourceOpen() {
    postMessageToWorker(worker, {
      type: MainMessageType.MediaSourceStateChanged,
      value: { mediaSourceId, state: MediaSourceReadyState.Open },
    });
  }
  function onMediaSourceClose() {
    postMessageToWorker(worker, {
      type: MainMessageType.MediaSourceStateChanged,
      value: { mediaSourceId, state: MediaSourceReadyState.Closed },
    });
  }

  return () => {
    mediaSource.removeEventListener("sourceclose", onMediaSourceClose);
    mediaSource.removeEventListener("sourceended", onMediaSourceEnded);
    mediaSource.removeEventListener("sourceopen", onMediaSourceOpen);
    URL.revokeObjectURL(objectURL);

    if (mediaSource.readyState !== "closed") {
      // TODO should probably wait until updates finish and whatnot
      const { readyState, sourceBuffers } = mediaSource;
      for (let i = sourceBuffers.length - 1; i >= 0; i--) {
        const sourceBuffer = sourceBuffers[i];

        // TODO what if not? Is the current code useful at all?
        if (!sourceBuffer.updating) {
          try {
            if (readyState === "open") {
              sourceBuffer.abort();
            }
            mediaSource.removeSourceBuffer(sourceBuffer);
          } catch (e) {
            const error = e instanceof Error ? e : "Unknown Error";
            logger.warn("Could not remove SourceBuffer", error);
          }
        }
      }
    }

    // TODO copy logic and comment of RxPlayer for proper stop
    videoElement.src = "";
    videoElement.removeAttribute("src");
  };
}

/**
 * Handles `AreTypesSupportedWorkerMessage` messages.
 * Returns `true` if a new rebuffering period has ended and `false ` either
 * if no rebuffering period has ended or if we were not in a rebuffering
 * period before.
 * @param {Object} msg - The worker's message received.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onAreTypesSupportedMessage(
  msg: AreTypesSupportedWorkerMessage,
  worker: Worker,
): void {
  const res: Partial<Record<string, boolean>> = {};
  for (const mimeType of msg.value.mimeTypes) {
    res[mimeType] = MediaSource.isTypeSupported(mimeType);
  }
  postMessageToWorker(worker, {
    type: MainMessageType.CodecsSupportUpdate,
    value: { mimeTypes: res },
  });
}
