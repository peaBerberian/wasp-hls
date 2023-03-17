import idGenerator from "../ts-common/idGenerator.js";
import logger from "../ts-common/logger.js";
import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer.js";
import {
  AudioTrackInfo,
  VariantInfo,
  WorkerMessageType,
} from "../ts-common/types.js";
import {
  LogLevel,
  MediaType,
  MediaSourceReadyState,
  TimerReason,
  AppendBufferErrorCode,
  AppendBufferResult,
  AttachMediaSourceResult,
  AttachMediaSourceErrorCode,
  RemoveMediaSourceResult,
  RemoveMediaSourceErrorCode,
  MediaSourceDurationUpdateResult,
  MediaSourceDurationUpdateErrorCode,
  AddSourceBufferResult,
  AddSourceBufferErrorCode,
  RemoveBufferResult,
  RemoveBufferErrorCode,
  EndOfStreamResult,
  EndOfStreamErrorCode,
  RequestErrorReason,
  OtherErrorCode,
  PlaylistType,
} from "../wasm/wasp_hls.js";
import {
  cachedCodecsSupport,
  jsMemoryResources,
  RequestId,
  requestsStore,
  ResourceId,
  SourceBufferId,
  playerInstance,
  TimerId,
  getMediaSourceObj,
} from "./globals";
import {
  getDurationFromTrun,
  getMDHDTimescale,
  getTrackFragmentDecodeTime,
} from "./isobmff-utils.js";
import postMessageToMain from "./postMessage.js";
import { getTransmuxedType, transmux } from "./transmux.js";
import { formatErrMessage, shouldTransmux } from "./utils.js";

// Some environments (such as Safari Desktop) weirdly do not support
// `performance.now` inside a WebWorker
const timerFn =
  typeof performance !== "object" ||
  performance === null ||
  typeof performance.now !== "function"
    ? Date.now.bind(Date)
    : performance.now.bind(performance);

const generateMediaSourceId = idGenerator();
const cachedTextDecoder = new TextDecoder("utf-8", {
  ignoreBOM: true,
  fatal: true,
});

export function sendSegmentRequestError(
  fatal: boolean,
  url: string,
  isInit: boolean,
  timeInfo: [number, number] | undefined,
  mediaType: MediaType,
  reason: RequestErrorReason,
  status: number | undefined
): void {
  const contentId = playerInstance.getContentInfo()?.contentId;
  if (contentId === undefined) {
    logger.error("Cannot send error, no contentId");
    return;
  }
  postMessageToMain({
    type: fatal
      ? (WorkerMessageType.Error as const)
      : (WorkerMessageType.Warning as const),
    value: {
      contentId,
      errorInfo: {
        type: "segment-request",
        value: {
          url,
          isInit,
          start: timeInfo?.[0],
          duration: timeInfo?.[1],
          mediaType,
          reason,
          status,
        },
      },
    },
  });
}

export function sendOtherError(
  fatal: boolean,
  code: OtherErrorCode,
  message: string | undefined
): void {
  const contentId = playerInstance.getContentInfo()?.contentId;
  if (contentId === undefined) {
    logger.error("Cannot send error, no contentId");
    return;
  }
  postMessageToMain({
    type: fatal
      ? (WorkerMessageType.Error as const)
      : (WorkerMessageType.Warning as const),
    value: {
      contentId,
      message,
      errorInfo: {
        type: "other-error",
        value: {
          code,
        },
      },
    },
  });
}

export function sendPlaylistParsingError(
  fatal: boolean,
  playlistType: PlaylistType,
  mediaType: MediaType | undefined,
  message: string | undefined
): void {
  const contentId = playerInstance.getContentInfo()?.contentId;
  if (contentId === undefined) {
    logger.error("Cannot send error, no contentId");
    return;
  }
  postMessageToMain({
    type: fatal
      ? (WorkerMessageType.Error as const)
      : (WorkerMessageType.Warning as const),
    value: {
      contentId,
      message,
      errorInfo: {
        type: "playlist-parse" as const,
        value: {
          type: playlistType,
          mediaType,
        },
      },
    },
  });
}

/**
 * @param {number} resourceId
 * @returns {Uint8Array|undefined}
 */
export function getResourceData(
  resourceId: ResourceId
): Uint8Array | undefined {
  return jsMemoryResources.get(resourceId);
}

/**
 * @param {number} logLevel
 * @param {string} logStr
 */
export function log(logLevel: LogLevel, logStr: string) {
  const now = timerFn().toFixed(2);
  switch (logLevel) {
    case LogLevel.Error:
      logger.error(now, logStr);
      break;
    case LogLevel.Warn:
      logger.warn(now, logStr);
      break;
    case LogLevel.Info:
      logger.info(now, logStr);
      break;
    case LogLevel.Debug:
      logger.debug(now, logStr);
      break;
  }
}

/**
 * @param {number} duration
 * @param {number} reason
 * @returns {string}
 */
export function timer(duration: number, reason: TimerReason): TimerId {
  const timerId = self.setTimeout(() => {
    const dispatcher = playerInstance.getDispatcher();
    if (dispatcher === null) {
      return;
    }
    dispatcher.on_timer_ended(timerId, reason);
  }, duration);
  return timerId;
}

/**
 * @param {number} id
 */
export function clearTimer(id: TimerId): void {
  clearTimeout(id);
}

/**
 * @param {string} url
 * @param {number|undefined} rangeStart
 * @param {number|undefined} rangeEnd
 * @param {number|undefined} timeout
 * @returns {number}
 */
export function doFetch(
  url: string,
  rangeStart: number | undefined,
  rangeEnd: number | undefined,
  timeout: number | undefined
): RequestId {
  let timeouted = false;
  const abortController = new AbortController();
  const currentRequestId = requestsStore.create({ abortController });
  const timestampBef = timerFn();

  let timeoutTimeoutId: number | undefined;
  if (timeout !== undefined) {
    timeoutTimeoutId = setTimeout(() => {
      timeouted = true;
      abortController.abort();
    }, timeout);
  }
  const headers: Array<[string, string]> = [];
  if (rangeStart !== undefined) {
    headers.push(["Range", `${rangeStart}-${rangeEnd ?? ""}`]);
  }
  fetch(url, { signal: abortController.signal, headers })
    .then(async (res) => {
      if (timeoutTimeoutId !== undefined) {
        clearTimeout(timeoutTimeoutId);
      }
      const dispatcher = playerInstance.getDispatcher();
      if (res.status >= 300) {
        dispatcher?.on_request_failed(currentRequestId, false, res.status);
        return;
      }

      const arrRes = await res.arrayBuffer();
      const elapsedMs = timerFn() - timestampBef;
      requestsStore.delete(currentRequestId);
      if (dispatcher !== null) {
        const segmentArray = new Uint8Array(arrRes);
        const currentResourceId = jsMemoryResources.create(segmentArray);
        dispatcher.on_request_finished(
          currentRequestId,
          currentResourceId,
          segmentArray.byteLength,
          res.url,
          elapsedMs
        );
      }
    })
    .catch((err) => {
      requestsStore.delete(currentRequestId);
      const dispatcher = playerInstance.getDispatcher();
      if (timeouted) {
        dispatcher?.on_request_failed(currentRequestId, true, undefined);
        return;
      }
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
      dispatcher?.on_request_failed(currentRequestId, false, undefined);
    });
  return currentRequestId;
}

/**
 * @param {number} id
 * @returns {boolean}
 */
export function abortRequest(id: RequestId): boolean {
  const requestObj = requestsStore.get(id);
  if (requestObj !== undefined) {
    requestObj.abortController.abort();

    // NOTE: we prefer deleting the id on a microtask to avoid possible RequestId
    // conflicts due to other microtask pending while this `abortRequest` call was
    // made (e.g. what if a request failure associated to that request was already
    // scheduled yet another request is made synchronously with the same RequestId?).
    /* eslint-disable-next-line @typescript-eslint/no-floating-promises */
    Promise.resolve().then(() => {
      requestsStore.delete(id);
    });
    return true;
  }
  return false;
}

// /**
//  * @param {number} warningCode
//  */
// export function warning(warningCode: WarningCode): void {
//   const contentInfo = playerInstance.getContentInfo();
//   if (contentInfo === null) {
//     return ;
//   }
//   postMessageToMain({
//     type: WorkerMessageType.Warning,
//     value: {
//       contentId: contentInfo.contentId,
//       code: warningCode,
//     },
//   });
// }

/**
 * @param {number} position
 */
export function seek(position: number): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    logger.error("Attempting to seek when no MediaSource is created");
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.Seek,
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
      position,
    },
  });
}

export function flush(): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    logger.error("Attempting to flush when no MediaSource is created");
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.Flush,
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
    },
  });
}

/**
 * @param {number} position
 */
export function setPlaybackRate(position: number): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    logger.error(
      "Attempting to set playback rate when no MediaSource is created"
    );
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.UpdatePlaybackRate,
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
      playbackRate: position,
    },
  });
}

/**
 * @returns {Object}
 */
export function attachMediaSource(): AttachMediaSourceResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return AttachMediaSourceResult.error(
      AttachMediaSourceErrorCode.NoContentLoaded
    );
  }

  try {
    if (playerInstance.hasMseInWorker() !== true) {
      const mediaSourceId = generateMediaSourceId();
      contentInfo.mediaSourceObj = {
        nextSourceBufferId: 0,
        sourceBuffers: [],
        type: "main",
        mediaSourceId,
      };
      postMessageToMain({
        type: WorkerMessageType.CreateMediaSource,
        value: {
          contentId: contentInfo.contentId,
          mediaSourceId,
        },
      });
    } else {
      const mediaSource = new MediaSource();
      mediaSource.addEventListener("sourceclose", onMediaSourceClose);
      mediaSource.addEventListener("sourceended", onMediaSourceEnded);
      mediaSource.addEventListener("sourceopen", onMediaSourceOpen);
      const removeEventListeners = () => {
        mediaSource.removeEventListener("sourceclose", onMediaSourceClose);
        mediaSource.removeEventListener("sourceended", onMediaSourceEnded);
        mediaSource.removeEventListener("sourceopen", onMediaSourceOpen);
      };

      /* eslint-disable-next-line */
      const handle = (mediaSource as any).handle;
      let objectURL;
      if (handle === undefined || handle === null) {
        // Weird typing for TypeScript
        objectURL = URL.createObjectURL(mediaSource as unknown as Blob);
      }
      const mediaSourceId = generateMediaSourceId();
      contentInfo.mediaSourceObj = {
        type: "worker",
        mediaSourceId,
        mediaSource,
        removeEventListeners,
        sourceBuffers: [],
        nextSourceBufferId: 0,
      };
      postMessageToMain(
        {
          type: WorkerMessageType.AttachMediaSource,
          value: {
            contentId: contentInfo.contentId,
            /* eslint-disable-next-line */
            handle,
            src: objectURL,
            mediaSourceId,
          },
        },
        handle !== undefined ? [handle] : []
      );
    }

    function onMediaSourceEnded(): void {
      playerInstance
        .getDispatcher()
        ?.on_media_source_state_change(MediaSourceReadyState.Ended);
    }
    function onMediaSourceOpen(): void {
      playerInstance
        .getDispatcher()
        ?.on_media_source_state_change(MediaSourceReadyState.Open);
    }
    function onMediaSourceClose(): void {
      playerInstance
        .getDispatcher()
        ?.on_media_source_state_change(MediaSourceReadyState.Closed);
    }
  } catch (e) {
    return AttachMediaSourceResult.error(
      AttachMediaSourceErrorCode.UnknownError
    );
  }
  return AttachMediaSourceResult.success();
}

/**
 * @returns {Object}
 */
export function removeMediaSource(): RemoveMediaSourceResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return RemoveMediaSourceResult.error(
      RemoveMediaSourceErrorCode.NoMediaSourceAttached
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return RemoveMediaSourceResult.error(
      RemoveMediaSourceErrorCode.NoMediaSourceAttached
    );
  }

  if (contentInfo.mediaSourceObj.type === "worker") {
    const { mediaSource, removeEventListeners } = contentInfo.mediaSourceObj;
    removeEventListeners();

    if (mediaSource !== null && mediaSource.readyState !== "closed") {
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
            const msg = formatErrMessage(
              e,
              "Unknown error while removing SourceBuffer"
            );
            logger.error("Could not remove SourceBuffer: " + msg);
            return RemoveMediaSourceResult.error(
              RemoveMediaSourceErrorCode.UnknownError,
              msg
            );
          }
        }
      }
    }
  }

  postMessageToMain({
    type: WorkerMessageType.ClearMediaSource,
    value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId },
  });
  return RemoveMediaSourceResult.success();
}

/**
 * @param {number} duration
 * @returns {Object}
 */
export function setMediaSourceDuration(
  duration: number
): MediaSourceDurationUpdateResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached
    );
  }

  if (contentInfo.mediaSourceObj.type === "worker") {
    try {
      contentInfo.mediaSourceObj.mediaSource.duration = duration;
      return MediaSourceDurationUpdateResult.success();
    } catch (err) {
      return MediaSourceDurationUpdateResult.error(
        MediaSourceDurationUpdateErrorCode.UnknownError
      );
    }
  } else {
    postMessageToMain({
      type: WorkerMessageType.UpdateMediaSourceDuration,
      value: {
        mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
        duration,
      },
    });
    return MediaSourceDurationUpdateResult.success();
  }
}

/**
 * @param {*} e
 * @returns {string|undefined}
 */
export function getErrorMessage(e: unknown): string | undefined {
  return e instanceof Error ? e.message : undefined;
}

/**
 * @param {number} mediaType
 * @param {string} typ
 * @returns {Object}
 */
export function addSourceBuffer(
  mediaType: MediaType,
  typ: string
): AddSourceBufferResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return AddSourceBufferResult.error(
      AddSourceBufferErrorCode.NoMediaSourceAttached
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return AddSourceBufferResult.error(
      AddSourceBufferErrorCode.NoMediaSourceAttached
    );
  }

  if (contentInfo.mediaSourceObj.type === "main") {
    const { sourceBuffers, nextSourceBufferId } = contentInfo.mediaSourceObj;
    try {
      let mimeType = typ;
      if (shouldTransmux(typ)) {
        mimeType = getTransmuxedType(typ, mediaType);
      }
      const sourceBufferId = nextSourceBufferId;
      sourceBuffers.push({
        lastInitTimescale: undefined,
        id: sourceBufferId,
        transmuxer: mimeType === typ ? null : transmux,
        sourceBuffer: null,
      });
      contentInfo.mediaSourceObj.nextSourceBufferId++;
      postMessageToMain({
        type: WorkerMessageType.CreateSourceBuffer,
        value: {
          mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
          sourceBufferId,
          contentType: mimeType,
        },
      });
      return AddSourceBufferResult.success(sourceBufferId);
    } catch (err) {
      const msg = formatErrMessage(
        err,
        "Unknown error while creating Sourcebuffer"
      );
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.UnknownError,
        msg
      );
    }
  } else {
    const { mediaSource, sourceBuffers, nextSourceBufferId } =
      contentInfo.mediaSourceObj;
    if (mediaSource.readyState === "closed") {
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.MediaSourceIsClosed
      );
    }
    if (typ === "") {
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.EmptyMimeType
      );
    }
    try {
      let mimeType = typ;
      if (shouldTransmux(typ)) {
        mimeType = getTransmuxedType(typ, mediaType);
      }
      const sourceBuffer = mediaSource.addSourceBuffer(mimeType);
      const sourceBufferId = nextSourceBufferId;
      const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
      sourceBuffers.push({
        lastInitTimescale: undefined,
        id: sourceBufferId,
        sourceBuffer: queuedSourceBuffer,
        transmuxer: mimeType === typ ? null : transmux,
      });
      contentInfo.mediaSourceObj.nextSourceBufferId++;
      return AddSourceBufferResult.success(sourceBufferId);
    } catch (err) {
      const msg = formatErrMessage(
        err,
        "Unknown error while creating Sourcebuffer"
      );
      if (!(err instanceof Error)) {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.UnknownError,
          msg
        );
      } else if (err.name === "QuotaExceededError") {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.QuotaExceededError,
          msg
        );
      } else if (err.name === "NotSupportedError") {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.TypeNotSupportedError,
          msg
        );
      } else {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.UnknownError,
          msg
        );
      }
    }
  }
}

/**
 * @param {number} sourceBufferId
 * @param {number} resourceId
 * @param {boolean} parseTimeInformation
 * @returns {Object}
 */
export function appendBuffer(
  sourceBufferId: SourceBufferId,
  resourceId: ResourceId,
  parseTimeInformation?: boolean
): AppendBufferResult {
  let segment = jsMemoryResources.get(resourceId);
  const mediaSourceObj = getMediaSourceObj();
  if (segment === undefined) {
    return AppendBufferResult.error(
      AppendBufferErrorCode.NoResource,
      "Segment preparation error: No resource with the given `resourceId`"
    );
  }
  if (mediaSourceObj === undefined) {
    return AppendBufferResult.error(
      AppendBufferErrorCode.NoSourceBuffer,
      "Segment preparation error: No MediaSource attached"
    );
  }

  // Weirdly enough TypeScript is only able to type-check when findIndex is
  // used then used as an index. Not when `find` is used directly.
  const sourceBufferObjIdx = mediaSourceObj.sourceBuffers.findIndex(
    ({ id }) => id === sourceBufferId
  );
  if (sourceBufferObjIdx < -1) {
    return AppendBufferResult.error(
      AppendBufferErrorCode.NoSourceBuffer,
      "Segment preparation error: No SourceBuffer with the given `SourceBufferId`"
    );
  }

  const sourceBufferObj = mediaSourceObj.sourceBuffers[sourceBufferObjIdx];
  if (sourceBufferObj.transmuxer !== null) {
    try {
      const transmuxedData = sourceBufferObj.transmuxer(segment);
      if (transmuxedData !== null) {
        segment = transmuxedData;
      } else {
        return AppendBufferResult.error(
          AppendBufferErrorCode.TransmuxerError,
          "Segment preparation error: the transmuxer couldn't process the segment"
        );
      }
    } catch (err) {
      const msg = formatErrMessage(
        err,
        "Unknown error while transmuxing segment"
      );
      return AppendBufferResult.error(
        AppendBufferErrorCode.TransmuxerError,
        msg
      );
    }
  }

  // TODO Check if mp4 first and if init segment?
  let timescale = getMDHDTimescale(segment);
  if (timescale !== undefined) {
    sourceBufferObj.lastInitTimescale = timescale;
  } else {
    timescale = sourceBufferObj.lastInitTimescale;
  }

  let timeInfo;
  if (parseTimeInformation === true && timescale !== undefined) {
    // TODO Check if mp4 first
    timeInfo = getTimeInformationFromMp4(segment, timescale);
  }
  try {
    if (sourceBufferObj.sourceBuffer !== null) {
      sourceBufferObj.sourceBuffer
        .push(segment)
        .then(() => {
          try {
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_update`", error);
          }
        })
        .catch(() => {
          try {
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_error`", error);
          }
        });
    } else {
      const buffer = segment.buffer;
      postMessageToMain(
        {
          type: WorkerMessageType.AppendBuffer,
          value: {
            mediaSourceId: mediaSourceObj.mediaSourceId,
            sourceBufferId,
            data: buffer,
          },
        },
        [buffer]
      );
    }
  } catch (err) {
    return AppendBufferResult.error(AppendBufferErrorCode.UnknownError);
  }
  return AppendBufferResult.success(timeInfo?.time, timeInfo?.duration);
}

/**
 * @param {number} sourceBufferId
 * @param {number} start
 * @param {number} end
 * @returns {Object}
 */
export function removeBuffer(
  sourceBufferId: SourceBufferId,
  start: number,
  end: number
): RemoveBufferResult {
  try {
    const mediaSourceObj = getMediaSourceObj();
    if (mediaSourceObj === undefined) {
      return RemoveBufferResult.error(
        RemoveBufferErrorCode.SourceBufferNotFound,
        "No MediaSource created."
      );
    }

    if (mediaSourceObj.type === "worker") {
      const sourceBuffer = mediaSourceObj.sourceBuffers.find(
        ({ id }) => id === sourceBufferId
      );
      if (sourceBuffer === undefined) {
        return RemoveBufferResult.error(
          RemoveBufferErrorCode.SourceBufferNotFound,
          "SourceBuffer linked to the given id not found."
        );
      }
      sourceBuffer.sourceBuffer
        .removeBuffer(start, end)
        .then(() => {
          try {
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_update`", error);
          }
        })
        .catch(() => {
          try {
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_error`", error);
          }
        });
    } else {
      postMessageToMain({
        type: WorkerMessageType.RemoveBuffer,
        value: {
          mediaSourceId: mediaSourceObj.mediaSourceId,
          sourceBufferId,
          start,
          end,
        },
      });
    }
  } catch (err) {
    const msg = formatErrMessage(err, "Unknown error while removing buffer");
    return RemoveBufferResult.error(RemoveBufferErrorCode.UnknownError, msg);
  }
  return RemoveBufferResult.success();
}

/**
 * @returns {Object}
 */
export function endOfStream(): EndOfStreamResult {
  try {
    const mediaSourceObj = getMediaSourceObj();
    if (mediaSourceObj === undefined) {
      return EndOfStreamResult.error(
        EndOfStreamErrorCode.NoMediaSourceAttached,
        "There's no MediaSource attached currently."
      );
    }
    if (mediaSourceObj.type === "worker") {
      mediaSourceObj.mediaSource.endOfStream();
    } else {
      postMessageToMain({
        type: WorkerMessageType.EndOfStream,
        value: { mediaSourceId: mediaSourceObj.mediaSourceId },
      });
    }
  } catch (err) {
    const msg = formatErrMessage(
      err,
      "Unknown error while calling endOfStream"
    );
    return EndOfStreamResult.error(EndOfStreamErrorCode.UnknownError, msg);
  }
  return EndOfStreamResult.success();
}

/**
 */
export function startObservingPlayback(): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  if (contentInfo.mediaSourceObj === null) {
    logger.error("Cannot start observing playback: No MediaSource Attached");
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.StartPlaybackObservation,
    value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId },
  });
}

/**
 */
export function stopObservingPlayback() {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  if (contentInfo.mediaSourceObj === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.StopPlaybackObservation,
    value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId },
  });
}

/**
 * @param {number} mediaOffset
 */
export function setMediaOffset(mediaOffset: number) {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.MediaOffsetUpdate,
    value: {
      contentId: contentInfo.contentId,
      offset: mediaOffset,
    },
  });
}

/**
 * @param {number} resourceId
 * @returns {boolean}
 */
export function freeResource(resourceId: ResourceId): boolean {
  if (jsMemoryResources.get(resourceId) === undefined) {
    return false;
  }
  jsMemoryResources.delete(resourceId);
  return true;
}

/**
 * @param {Uint8Array} segment
 * @param {number} initTimescale
 * @returns {Object|null}
 */
function getTimeInformationFromMp4(
  segment: Uint8Array,
  initTimescale: number
): { time: number; duration: number | undefined } | null {
  const baseDecodeTime = getTrackFragmentDecodeTime(segment);
  if (baseDecodeTime === undefined) {
    return null;
  }
  const trunDuration = getDurationFromTrun(segment);
  return {
    time: baseDecodeTime / initTimescale,
    duration:
      trunDuration === undefined ? undefined : trunDuration / initTimescale,
  };
}

/**
 * @param {number|undefined} minimumPosition
 * @param {number|undefined} maximumPosition
 */
export function updateContentInfo(
  minimumPosition: number | undefined,
  maximumPosition: number | undefined
): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.ContentTimeBoundsUpdate,
    value: {
      contentId: contentInfo.contentId,
      minimumPosition,
      maximumPosition,
    },
  });
}

export function announceFetchedContent(
  variantInfo: Uint32Array,
  audioTracksInfo: Uint32Array
): void {
  const contentInfo = playerInstance.getContentInfo();
  const memory = playerInstance.getCurrentWasmMemory();
  if (contentInfo === null || memory === null) {
    return;
  }
  const variantInfoObj: VariantInfo[] = [];
  {
    let i = 0;
    i++; // Skip number of variants
    while (i < variantInfo.length) {
      const idLen = variantInfo[i];
      i++;

      const idU8 = new Uint8Array(memory.buffer, variantInfo[i], idLen);
      i++;
      const id = cachedTextDecoder.decode(idU8);

      const height = variantInfo[i];
      i++;

      const width = variantInfo[i];
      i++;

      const frameRate = variantInfo[i];
      i++;

      const bandwidth = variantInfo[i];
      i++;

      variantInfoObj.push({
        id,
        height,
        width,
        frameRate,
        bandwidth,
      });
    }
  }
  const audioTracksObj: AudioTrackInfo[] = [];
  {
    let i = 0;
    i++; // Skip number of audio tracks
    while (i < audioTracksInfo.length) {
      const idLen = audioTracksInfo[i];
      i++;
      const idU8 = new Uint8Array(memory.buffer, audioTracksInfo[i], idLen);
      i++;
      const id = cachedTextDecoder.decode(idU8);

      const languageLen = audioTracksInfo[i];
      i++;
      const languageU8 = new Uint8Array(
        memory.buffer,
        audioTracksInfo[i],
        languageLen
      );
      i++;
      const language = cachedTextDecoder.decode(languageU8);

      const assocLanguageLen = audioTracksInfo[i];
      i++;
      const assocLanguageU8 = new Uint8Array(
        memory.buffer,
        audioTracksInfo[i],
        assocLanguageLen
      );
      i++;
      const assocLanguage = cachedTextDecoder.decode(assocLanguageU8);

      const nameLen = audioTracksInfo[i];
      i++;
      const nameU8 = new Uint8Array(memory.buffer, audioTracksInfo[i], nameLen);
      i++;
      const name = cachedTextDecoder.decode(nameU8);

      const channels = audioTracksInfo[i];
      i++;

      audioTracksObj.push({
        id,
        language: language === "" ? undefined : language,
        assocLanguage: assocLanguage === "" ? undefined : assocLanguage,
        name,
        channels,
      });
    }
  }
  postMessageToMain({
    type: WorkerMessageType.MultiVariantPlaylistParsed,
    value: {
      contentId: contentInfo.contentId,
      variants: variantInfoObj,
      audioTracks: audioTracksObj,
    },
  });
}

export function announceTrackUpdate(
  mediaType: MediaType,
  currentAudioTrack: string,
  isAudioTrackSelected: boolean
): void {
  const contentInfo = playerInstance.getContentInfo();
  const memory = playerInstance.getCurrentWasmMemory();
  if (contentInfo === null || memory === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.TrackUpdate,
    value: {
      mediaType,
      contentId: contentInfo.contentId,
      audioTrack: currentAudioTrack
        ? {
            current: currentAudioTrack,
            isSelected: isAudioTrackSelected,
          }
        : undefined,
    },
  });
}

export function announceVariantUpdate(variantId: string | undefined): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.VariantUpdate,
    value: {
      contentId: contentInfo.contentId,
      variantId,
    },
  });
}

export function startRebuffering(): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    logger.error(
      "Attempting to start rebuffering when no MediaSource is created"
    );
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.RebufferingStarted,
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
      updatePlaybackRate: true,
    },
  });
}

export function stopRebuffering(): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    logger.error(
      "Attempting to stop rebuffering when no MediaSource is created"
    );
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.RebufferingEnded,
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
    },
  });
}

export function getRandom(): number {
  return Math.random();
}

const codecsToAskForSupport = new Set<string>();
let isCurrentlyWaitingToAskSupport = false;

export function isTypeSupported(
  mediaType: MediaType,
  codec: string
): boolean | undefined {
  const mimeTypes = [];
  let mimeTypePrefix: string;
  switch (mediaType) {
    case MediaType.Audio:
      mimeTypePrefix = "audio/";
      break;
    case MediaType.Video:
      mimeTypePrefix = "video/";
      break;
    default:
      logger.error("Unknown MediaType");
      return false;
  }
  if (playerInstance.canDemuxMpeg2Ts() === true) {
    mimeTypes.push(`${mimeTypePrefix}mp2t;codecs=\"${codec}\"`);
  }
  mimeTypes.push(`${mimeTypePrefix}mp4;codecs=\"${codec}\"`);

  // TODO keep somewhere which one is supported to be able to know if
  // transmuxing is necessary or not
  if (playerInstance.hasMseInWorker() === true) {
    return mimeTypes.some(mimeType =>MediaSource.isTypeSupported(mimeType));
  }
  const cached = mimeTypes.map(mimeType => cachedCodecsSupport.get(mimeType))
    .filter(isSupported => isSupported !== undefined);
  if (cached.length > 0) {
    return cached.some(isSupported => isSupported);
  }

  mimeTypes.forEach(mimeType =>
    codecsToAskForSupport.add(mimeType)
  );
  if (isCurrentlyWaitingToAskSupport) {
    return undefined;
  }
  isCurrentlyWaitingToAskSupport = true;

  // We here schedule a micro-task to pool multiple synchronous calls to
  // `isTypeSupported` together when asking it to the main thread.
  Promise.resolve()
    .then(() => {
      isCurrentlyWaitingToAskSupport = false;
      postMessageToMain({
        type: WorkerMessageType.AreTypesSupported,
        value: {
          mimeTypes: Array.from(codecsToAskForSupport.keys()),
        },
      });
      codecsToAskForSupport.clear();
    })
    .catch(() => {
      /* noop */
    });
}
