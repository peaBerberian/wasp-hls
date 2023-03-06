import logger from "../ts-common/logger";
import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import {
  AppendBufferWorkerMessage,
  AttachMediaSourceWorkerMessage,
  ClearMediaSourceWorkerMessage,
  CreateMediaSourceWorkerMessage,
  CreateSourceBufferWorkerMessage,
  EndOfStreamWorkerMessage,
  EndOfStreamErrorCode,
  MediaSourceReadyState,
  RebufferingEndedWorkerMessage,
  RebufferingStartedWorkerMessage,
  RemoveBufferWorkerMessage,
  SeekWorkerMessage,
  SetMediaSourceDurationWorkerMessage,
  SourceBufferCreationErrorCode,
  StartPlaybackObservationWorkerMessage,
  StopPlaybackObservationWorkerMessage,
  UpdatePlaybackRateWorkerMessage,
  MediaOffsetUpdateWorkerMessage,
  ContentErrorWorkerMessage,
  ContentWarningWorkerMessage,
  ContentInfoUpdateWorkerMessage,
  ContentStoppedWorkerMessage,
} from "../ts-common/types";
import observePlayback from "./observePlayback";
import postMessageToWorker from "./postMessageToWorker";
import { ContentMetadata } from "./types";
import { clearElementSrc, getErrorInformation } from "./utils";

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
  mediaElement: HTMLMediaElement
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring MediaSource attachment due to wrong `contentId`");
    return;
  }
  if (contentMetadata.stopPlaybackObservations !== null) {
    contentMetadata.stopPlaybackObservations();
    contentMetadata.stopPlaybackObservations = null;
  }

  if (msg.value.handle !== undefined) {
    mediaElement.srcObject = msg.value.handle;
  } else if (msg.value.src !== undefined) {
    mediaElement.src = msg.value.src;
  } else {
    throw new Error(
      "Unexpected \"attach-media-source\" message: missing source"
    );
  }
  contentMetadata.mediaSourceId = msg.value.mediaSourceId;
  contentMetadata.mediaSource = null;
  contentMetadata.disposeMediaSource = () => {
    if (msg.value.src !== undefined) {
      URL.revokeObjectURL(msg.value.src);
    }
  };
  contentMetadata.sourceBuffers = [];
  contentMetadata.stopPlaybackObservations = null;
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
  mediaElement: HTMLMediaElement
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring seek due to wrong `mediaSourceId`");
    return;
  }
  try {
    mediaElement.currentTime = msg.value.position;
  } catch (err) {
    logger.error("Unexpected error while seeking:", err);
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
  mediaElement: HTMLMediaElement
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring playback rate update due to wrong `mediaSourceId`");
    return;
  }
  try {
    mediaElement.playbackRate = msg.value.playbackRate;
  } catch (err) {
    logger.error("Unexpected error while changing the playback rate:", err);
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
  worker: Worker
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring MediaSource attachment due to wrong `contentId`");
  } else {
    const { mediaSourceId } = msg.value;
    try {
      contentMetadata.disposeMediaSource?.();
      contentMetadata.stopPlaybackObservations?.();

      const mediaSource = new MediaSource();

      const disposeMediaSource =
        bindMediaSource(worker, mediaSource, mediaElement, mediaSourceId);
      contentMetadata.mediaSourceId = msg.value.mediaSourceId;
      contentMetadata.mediaSource = mediaSource;
      contentMetadata.disposeMediaSource = disposeMediaSource;
      contentMetadata.sourceBuffers = [];
      contentMetadata.stopPlaybackObservations = null;
    } catch (err) {
      const { name, message } = getErrorInformation(
        err,
        "Unknown error when creating the MediaSource"
      );
      postMessageToWorker(worker, {
        type: "create-media-source-error",
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
  worker: Worker
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
      "Unknown error when updating the MediaSource's duration"
    );
    const { mediaSourceId } = msg.value;
    postMessageToWorker(worker, {
      type: "update-media-source-duration-error",
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
  mediaElement: HTMLMediaElement
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring MediaSource clearing due to wrong `mediaSourceId`"
    );
    return;
  }
  try {
    contentMetadata.disposeMediaSource?.();
    clearElementSrc(mediaElement);
  } catch (err) {
    logger.warn("API: Error when clearing current MediaSource:", err);
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
  worker: Worker
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring SourceBuffer creation due to wrong `mediaSourceId`"
    );
    return;
  }
  if (contentMetadata.mediaSource === null) {
    postMessageToWorker(worker, {
      type: "create-source-buffer-error",
      value: {
        mediaSourceId: msg.value.mediaSourceId,
        sourceBufferId: msg.value.sourceBufferId,
        code: SourceBufferCreationErrorCode.NoMediaSource,
        message: "No MediaSource created on the main thread.",
        name: undefined,
      },
    });
    return;
  }
  try {
    const sourceBuffer = contentMetadata.mediaSource
      .addSourceBuffer(msg.value.contentType);
    const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
    contentMetadata.sourceBuffers.push({
      sourceBufferId: msg.value.sourceBufferId,
      queuedSourceBuffer,
    });
  } catch (err) {
    const { name, message } = getErrorInformation(
      err,
      "Unknown error when adding the SourceBuffer to the MediaSource"
    );
    postMessageToWorker(worker, {
      type: "create-source-buffer-error",
      value: {
        mediaSourceId: msg.value.mediaSourceId,
        sourceBufferId: msg.value.sourceBufferId,
        code: SourceBufferCreationErrorCode.AddSourceBufferError,
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
  worker: Worker
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring appendBuffer operation due to wrong `mediaSourceId`"
    );
    return;
  }
  const sbObject = contentMetadata.sourceBuffers
    .find(({ sourceBufferId }) => sourceBufferId === msg.value.sourceBufferId);
  if (sbObject !== undefined) {
    const { mediaSourceId, sourceBufferId } = msg.value;
    try {
      sbObject.queuedSourceBuffer.push(msg.value.data)
        .then(() => {
          postMessageToWorker(worker, {
            type: "source-buffer-updated",
            value: { mediaSourceId, sourceBufferId },
          });
        })
        .catch(handleAppendBufferError);
    } catch (err) {
      handleAppendBufferError(err);
    }
    function handleAppendBufferError(err: unknown): void {
      const { name, message } = getErrorInformation(
        err,
        "Unknown error when appending data to the SourceBuffer"
      );
      postMessageToWorker(worker, {
        type: "source-buffer-error",
        value: { sourceBufferId, message, name },
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
  worker: Worker
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring removeBuffer operation due to wrong `mediaSourceId`"
    );
    return ;
  }
  const sbObject = contentMetadata.sourceBuffers
    .find(({ sourceBufferId }) => sourceBufferId === msg.value.sourceBufferId);
  if (sbObject !== undefined) {
    const { mediaSourceId, sourceBufferId } = msg.value;
    try {
      sbObject.queuedSourceBuffer.removeBuffer(msg.value.start, msg.value.end)
        .then(() => {
          postMessageToWorker(worker, {
            type: "source-buffer-updated",
            value: { mediaSourceId, sourceBufferId },
          });
        })
        .catch(handleRemoveBufferError);
    } catch (err) {
      handleRemoveBufferError(err);
    }
    function handleRemoveBufferError(err: unknown): void {
      const { name, message } = getErrorInformation(
        err,
        "Unknown error when removing data to the SourceBuffer"
      );
      postMessageToWorker(worker, {
        type: "source-buffer-error",
        value: { sourceBufferId, message, name },
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
  worker: Worker
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring `start-playback-observation` due to wrong `mediaSourceId`"
    );
    return;
  }
  if (contentMetadata.stopPlaybackObservations !== null) {
    contentMetadata.stopPlaybackObservations();
    contentMetadata.stopPlaybackObservations = null;
  }
  contentMetadata.stopPlaybackObservations = observePlayback(
    mediaElement,
    msg.value.mediaSourceId,
    (value) => postMessageToWorker(worker, { type: "observation", value })
  );
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
  contentMetadata: ContentMetadata | null
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring `stop-playback-observation` due to wrong `mediaSourceId`"
    );
    return;
  }
  if (contentMetadata.stopPlaybackObservations !== null) {
    contentMetadata.stopPlaybackObservations();
    contentMetadata.stopPlaybackObservations = null;
  }
}

/**
 * Handles `EndOfStreamWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @param {Worker} worker - The WebWorker concerned, messages may be sent back
 * to it.
 */
export function onEndOfStreamMessage(
  msg: EndOfStreamWorkerMessage,
  contentMetadata: ContentMetadata | null,
  worker: Worker
): void {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info(
      "API: Ignoring `end-of-stream` due to wrong `mediaSourceId`"
    );
  } else {
    const { mediaSourceId } = msg.value;
    if (contentMetadata.mediaSource === null) {
      postMessageToWorker(worker, {
        type: "end-of-stream-error",
        value: {
          mediaSourceId,
          code: EndOfStreamErrorCode.NoMediaSource,
          message: "No MediaSource created on the main thread.",
          name: undefined,
        },
      });
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
      const { name, message } = getErrorInformation(
        err,
        "Unknown error when calling MediaSource.endOfStream()"
      );
      postMessageToWorker(worker, {
        type: "end-of-stream-error",
        value: {
          mediaSourceId,
          code: EndOfStreamErrorCode.EndOfStreamError,
          message,
          name,
        },
      });
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
  contentMetadata: ContentMetadata | null
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info(
      "API: Ignoring media offset update due to wrong `contentId`"
    );
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
  mediaElement: HTMLMediaElement
): boolean {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring rebuffering start due to wrong `mediaSourceId`");
    return false;
  }
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
  mediaElement: HTMLMediaElement
): boolean {
  if (contentMetadata?.mediaSourceId !== msg.value.mediaSourceId) {
    logger.info("API: Ignoring rebuffering end due to wrong `mediaSourceId`");
    return false;
  }
  if (
    mediaElement.playbackRate === 0 &&
    contentMetadata.wantedSpeed !== 0
  ) {
    mediaElement.playbackRate = contentMetadata.wantedSpeed;
  }
  if (contentMetadata.isRebuffering) {
    contentMetadata.isRebuffering = false;
    return true;
  }
  return false;
}

/**
 * Handles `ContentErrorWorkerMessage` messages.
 * Returns either `null` if the error was ignored or the produced error if it
 * wasn't. Such error may then be used for example for the corresponding event.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {Error|null}
 */
export function onContentErrorMessage(
  msg: ContentErrorWorkerMessage,
  contentMetadata: ContentMetadata | null
): Error|null {
  // TODO
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring error due to wrong `contentId`");
    return null;
  }
  if (contentMetadata.loadingAborter !== undefined) {
    contentMetadata.loadingAborter.abort(
      new Error("Could not load content due to an error")
    );
  }

  // Make sure resources are freed
  contentMetadata.disposeMediaSource?.();
  contentMetadata.stopPlaybackObservations?.();
  const error = new Error("An error arised");
  contentMetadata.error = error;
  return error;
}

/**
 * Handles `ContentWarningWorkerMessage` messages.
 * Returns either `null` if the error was ignored or the produced error if it
 * wasn't. Such error may then be used for example for the corresponding event.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 * @returns {Warning|null}
 */
export function onContentWarningMessage(
  msg: ContentWarningWorkerMessage,
  contentMetadata: ContentMetadata | null
): Error|null {
  // TODO
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return null;
  }
  // logger.warn("Warning received", data.value.code);
  // this.trigger("warning", {
  //   code: data.value.code,
  //   // TODO
  //   message: undefined,
  // });
  return null;
}

/**
 * Handles `ContentInfoUpdateWorkerMessage` messages.
 * @param {Object} msg - The worker's message received.
 * @param {Object|null} contentMetadata - Metadata of the content currently
 * playing. `null` if no content is currently playing.
 * This object may be mutated.
 */
export function onContentInfoUpdateMessage(
  msg: ContentInfoUpdateWorkerMessage,
  contentMetadata: ContentMetadata | null
): void {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring warning due to wrong `contentId`");
    return;
  }
  contentMetadata.minimumPosition = msg.value.minimumPosition;
  contentMetadata.maximumPosition = msg.value.maximumPosition;
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
  contentMetadata: ContentMetadata | null
): boolean {
  if (contentMetadata?.contentId !== msg.value.contentId) {
    logger.info("API: Ignoring `content-stopped` due to wrong `contentId`");
    return false;
  }
  // Make sure resources are freed
  contentMetadata.disposeMediaSource?.();
  contentMetadata.stopPlaybackObservations?.();
  contentMetadata.loadingAborter?.abort(
    new Error("Content Stopped")
  );
  return true;
}

function bindMediaSource(
  worker: Worker,
  mediaSource: MediaSource,
  videoElement: HTMLMediaElement,
  mediaSourceId: string
) : () => void {
  mediaSource.addEventListener("sourceclose", onMediaSourceClose);
  mediaSource.addEventListener("sourceended", onMediaSourceEnded);
  mediaSource.addEventListener("sourceopen", onMediaSourceOpen);

  const objectURL = URL.createObjectURL(mediaSource);
  videoElement.src = objectURL;

  function onMediaSourceEnded() {
    postMessageToWorker(worker, {
      type: "media-source-state-changed",
      value: { mediaSourceId, state: MediaSourceReadyState.Ended },
    });
  }
  function onMediaSourceOpen() {
    postMessageToWorker(worker, {
      type: "media-source-state-changed",
      value: { mediaSourceId, state: MediaSourceReadyState.Open },
    });
  }
  function onMediaSourceClose() {
    postMessageToWorker(worker, {
      type: "media-source-state-changed",
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
          }
          catch (_e) {
            // TODO
          }
        }
      }
    }

    // TODO copy logic and comment of RxPlayer for proper stop
    videoElement.src = "";
    videoElement.removeAttribute("src");
  };
}
