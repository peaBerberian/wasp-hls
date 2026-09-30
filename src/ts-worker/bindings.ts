import idGenerator from "../ts-common/idGenerator.ts";
import logger from "../ts-common/logger.ts";
import monotonicNow from "../ts-common/monotonicNow.ts";
import QueuedSourceBuffer, {
  SourceBufferOperationCancelledError,
} from "../ts-common/QueuedSourceBuffer.ts";
import timeRangesToFloat64Array from "../ts-common/timeRangesToFloat64Array.ts";
import type {
  AudioTrackInfo,
  SourceBufferId,
  VariantInfo,
} from "../ts-common/types.js";
import { WorkerMessageType } from "../ts-common/types.ts";
import {
  AddSourceBufferErrorCode,
  AddSourceBufferResult,
  AppendBufferResult,
  AttachMediaSourceErrorCode,
  AttachMediaSourceResult,
  EndOfStreamErrorCode,
  EndOfStreamResult,
  JsTimeRanges,
  LogLevel,
  MediaSourceDurationUpdateErrorCode,
  MediaSourceDurationUpdateResult,
  MediaSourceReadyState,
  MediaType,
  PushedSegmentErrorCode,
  RemoveBufferErrorCode,
  RemoveBufferResult,
  RemoveMediaSourceErrorCode,
  RemoveMediaSourceResult,
  SegmentParsingErrorCode,
} from "../wasm/index.js";
import type {
  ContentCompatibilityErrorCode,
  SegmentHints,
  HostBindings,
  InspectSegmentValue,
  ISafeU64,
  MediaPlaylistParsingErrorCode,
  MultivariantPlaylistParsingErrorCode,
  OtherErrorCode,
  PlaylistNature,
  PlaylistType,
  RequestErrorReason,
  SourceBufferCreationErrorCode,
  TimerReason,
} from "../wasm/index.js";
import {
  cachedCodecsSupport,
  jsMemoryResources,
  requestsStore,
  playerInstance,
  getMediaSourceObj,
} from "./globals.ts";
import type { RequestId, ResourceId, TimerId } from "./globals.ts";
import {
  getIsoBmffCodecs,
  getInitTrackInfo,
  getIsobmfTimeInfo,
} from "./isobmff-utils.js";
import postMessageToMain from "./postMessage.js";
import {
  createTransmuxer,
  getFmp4Type,
  getTransmuxedType,
} from "./transmux.js";
import { formatErrMessage, shouldTransmux } from "./utils.js";

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
  mediaType: MediaType | undefined,
  reason: RequestErrorReason,
  status: number | undefined,
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

export function sendMultivariantPlaylistRequestError(
  fatal: boolean,
  url: string,
  reason: RequestErrorReason,
  status: number | undefined,
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
        type: "multi-var-playlist-request",
        value: {
          url,
          reason,
          status,
        },
      },
    },
  });
}

export function sendMediaPlaylistRequestError(
  fatal: boolean,
  url: string,
  reason: RequestErrorReason,
  mediaType: MediaType | undefined,
  status: number | undefined,
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
        type: "media-playlist-request",
        value: {
          url,
          reason,
          mediaType,
          status,
        },
      },
    },
  });
}

export function sendPushedSegmentError(
  fatal: boolean,
  code: PushedSegmentErrorCode,
  mediaType: MediaType,
  message: string,
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
        type: "push-segment-error",
        value: {
          code,
          mediaType,
        },
      },
    },
  });
}

export function sendRemoveBufferError(
  fatal: boolean,
  mediaType: MediaType,
  message: string,
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
        type: "remove-buffer-error",
        value: {
          mediaType,
        },
      },
    },
  });
}

export function sendOtherError(
  fatal: boolean,
  code: OtherErrorCode,
  message: string,
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

export function sendContentCompatibilityError(
  fatal: boolean,
  code: ContentCompatibilityErrorCode,
  message: string,
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
        type: "content-compatibility",
        value: {
          code,
        },
      },
    },
  });
}

export function sendMultivariantPlaylistParsingError(
  fatal: boolean,
  code: MultivariantPlaylistParsingErrorCode,
  message: string,
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
        type: "multi-var-playlist-parse" as const,
        value: {
          code,
        },
      },
    },
  });
}

export function sendMediaPlaylistParsingError(
  fatal: boolean,
  code: MediaPlaylistParsingErrorCode,
  mediaType: MediaType | undefined,
  message: string,
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
        type: "media-playlist-parse" as const,
        value: {
          code,
          mediaType,
        },
      },
    },
  });
}

export function sendSourceBufferCreationError(
  fatal: boolean,
  code: SourceBufferCreationErrorCode,
  mediaType: MediaType,
  message: string,
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
        type: "sb-creation" as const,
        value: {
          code,
          mediaType,
        },
      },
    },
  });
}

export function sendSegmentParsingError(
  fatal: boolean,
  code: SegmentParsingErrorCode,
  mediaType: MediaType | undefined,
  message: string,
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
        type: "segment-parse" as const,
        value: {
          code,
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
  resourceId: ResourceId,
): Uint8Array | undefined {
  return jsMemoryResources.get(resourceId);
}

/**
 * @param {number} logLevel
 * @param {string} logStr
 */
export function log(logLevel: LogLevel, logStr: string) {
  const now = monotonicNow().toFixed(2);
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
  timeout: number,
): RequestId {
  let timeouted = false;
  const abortController = new AbortController();
  const currentRequestId = requestsStore.create({ abortController });
  const timestampBef = monotonicNow();

  let timeoutTimeoutId: number | undefined;
  if (timeout >= 0) {
    timeoutTimeoutId = setTimeout(() => {
      timeouted = true;
      abortController.abort();
    }, timeout);
  }
  const headers: Array<[string, string]> = [];
  if (rangeStart !== undefined) {
    headers.push(["Range", `bytes=${rangeStart}-${rangeEnd ?? ""}`]);
  }
  fetch(url, { signal: abortController.signal, headers })
    .then(async (res) => {
      if (timeoutTimeoutId !== undefined) {
        clearTimeout(timeoutTimeoutId);
      }
      const dispatcher = playerInstance.getDispatcher();
      if (res.status >= 300) {
        logger.warn(
          `Worker: fetch failed id=${currentRequestId} status=${res.status} elapsed=${(monotonicNow() - timestampBef).toFixed(1)}ms url=${res.url || url}`,
        );
        requestsStore.delete(currentRequestId);
        dispatcher?.on_request_failed(currentRequestId, false, res.status);
        return;
      }

      const arrRes = await res.arrayBuffer();
      const elapsedMs = monotonicNow() - timestampBef;
      requestsStore.delete(currentRequestId);
      if (dispatcher !== null) {
        const segmentArray = new Uint8Array(arrRes);
        const currentResourceId = jsMemoryResources.create(segmentArray);
        dispatcher.on_request_finished(
          currentRequestId,
          currentResourceId,
          segmentArray.byteLength,
          res.url,
          elapsedMs,
        );
      }
    })
    .catch((err) => {
      requestsStore.delete(currentRequestId);
      const dispatcher = playerInstance.getDispatcher();
      if (timeouted) {
        logger.warn(
          `Worker: fetch timeout id=${currentRequestId} timeout=${timeout}ms url=${url}`,
        );
        dispatcher?.on_request_failed(currentRequestId, true, undefined);
        return;
      }
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
      logger.warn(
        `Worker: fetch error id=${currentRequestId} elapsed=${(monotonicNow() - timestampBef).toFixed(1)}ms url=${url} err=${formatErrMessage(err, "Unknown fetch error")}`,
      );
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
      "Attempting to set playback rate when no MediaSource is created",
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
      AttachMediaSourceErrorCode.NoContentLoaded,
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
        handle !== undefined ? [handle] : [],
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
  } catch (_e) {
    return AttachMediaSourceResult.error(
      AttachMediaSourceErrorCode.UnknownError,
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
      RemoveMediaSourceErrorCode.NoMediaSourceAttached,
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return RemoveMediaSourceResult.error(
      RemoveMediaSourceErrorCode.NoMediaSourceAttached,
    );
  }

  if (contentInfo.mediaSourceObj.type === "worker") {
    const {
      mediaSource,
      removeEventListeners,
      sourceBuffers: sourceBufferInfos,
    } = contentInfo.mediaSourceObj;
    removeEventListeners();
    for (const sourceBuffer of sourceBufferInfos) {
      sourceBuffer.sourceBuffer.dispose();
    }
    contentInfo.mediaSourceObj.sourceBuffers = [];

    if (mediaSource !== null && mediaSource.readyState !== "closed") {
      const { readyState } = mediaSource;
      const { sourceBuffers } = mediaSource as MediaSource & {
        sourceBuffers: ArrayLike<SourceBuffer>;
      };
      for (let i = sourceBuffers.length - 1; i >= 0; i--) {
        const sourceBuffer = sourceBuffers[i];
        try {
          if (readyState === "open") {
            sourceBuffer.abort();
          }
          mediaSource.removeSourceBuffer(sourceBuffer);
        } catch (e) {
          const msg = formatErrMessage(
            e,
            "Unknown error while removing SourceBuffer",
          );
          logger.error("Could not remove SourceBuffer: " + msg);
          return RemoveMediaSourceResult.error(
            RemoveMediaSourceErrorCode.UnknownError,
            msg,
          );
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
  duration: number,
): MediaSourceDurationUpdateResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached,
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached,
    );
  }

  if (contentInfo.mediaSourceObj.type === "worker") {
    try {
      contentInfo.mediaSourceObj.mediaSource.duration = duration;
      return MediaSourceDurationUpdateResult.success();
    } catch (_err) {
      return MediaSourceDurationUpdateResult.error(
        MediaSourceDurationUpdateErrorCode.UnknownError,
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
 * @param {number} mediaType
 * @param {string} typ
 * @returns {Object}
 */
export function addSourceBuffer(
  mediaType: MediaType,
  typ: string,
): AddSourceBufferResult {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return AddSourceBufferResult.error(
      AddSourceBufferErrorCode.NoMediaSourceAttached,
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return AddSourceBufferResult.error(
      AddSourceBufferErrorCode.NoMediaSourceAttached,
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
        lastInitTrackInfoByTrackId: undefined,
        id: sourceBufferId,
        transmuxer: mimeType === typ ? null : createTransmuxer(),
        sourceBuffer: null,
        mediaType,
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
        "Unknown error while creating Sourcebuffer",
      );
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.UnknownError,
        msg,
      );
    }
  } else {
    const { mediaSource, sourceBuffers, nextSourceBufferId } =
      contentInfo.mediaSourceObj;
    if (mediaSource.readyState === "closed") {
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.MediaSourceIsClosed,
      );
    }
    if (typ === "") {
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.EmptyMimeType,
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
        lastInitTrackInfoByTrackId: undefined,
        id: sourceBufferId,
        sourceBuffer: queuedSourceBuffer,
        transmuxer: mimeType === typ ? null : createTransmuxer(),
        mediaType,
      });
      contentInfo.mediaSourceObj.nextSourceBufferId++;
      return AddSourceBufferResult.success(sourceBufferId);
    } catch (err) {
      const msg = formatErrMessage(
        err,
        "Unknown error while creating Sourcebuffer",
      );
      if (!(err instanceof Error)) {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.UnknownError,
          msg,
        );
      } else if (err.name === "QuotaExceededError") {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.QuotaExceededError,
          msg,
        );
      } else if (err.name === "NotSupportedError") {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.TypeNotSupportedError,
          msg,
        );
      } else {
        return AddSourceBufferResult.error(
          AddSourceBufferErrorCode.UnknownError,
          msg,
        );
      }
    }
  }
}

/**
 * @param sourceBufferId - The identifier for the SourceBuffer on which the
 * segment should be pushed.
 * @param resourceId - The identifier of the segment to push.
 * @param segmentHints - Potential supplementary context on the segment to
 * push, such as its base decode time.
 * @returns {Object}
 */
export function appendBuffer(
  sourceBufferId: SourceBufferId,
  resourceId: ResourceId,
  segmentHints: SegmentHints,
): AppendBufferResult {
  let segment = jsMemoryResources.get(resourceId);
  const mediaSourceObj = getMediaSourceObj();
  if (segment === undefined) {
    return AppendBufferResult.error(
      SegmentParsingErrorCode.NoResource,
      "Segment preparation error: No resource with the given `resourceId`",
    );
  }
  if (mediaSourceObj === undefined) {
    return AppendBufferResult.error(
      SegmentParsingErrorCode.NoSourceBuffer,
      "Segment preparation error: No MediaSource attached",
    );
  }

  // Weirdly enough TypeScript is only able to type-check when findIndex is
  // used then used as an index. Not when `find` is used directly.
  const sourceBufferObjIdx = mediaSourceObj.sourceBuffers.findIndex(
    ({ id }) => id === sourceBufferId,
  );
  if (sourceBufferObjIdx < 0) {
    return AppendBufferResult.error(
      SegmentParsingErrorCode.NoSourceBuffer,
      "Segment preparation error: No SourceBuffer with the given `SourceBufferId`",
    );
  }

  let segmentPreciseTiming:
    | {
        start: ISafeU64;
        end: ISafeU64 | undefined;
        timescale: number;
      }
    | undefined;

  const sourceBufferObj = mediaSourceObj.sourceBuffers[sourceBufferObjIdx];
  if (sourceBufferObj.transmuxer !== null) {
    try {
      const dtsHint = combineSafeTimeValue(
        segmentHints.baseDecodeTimeStartHi,
        segmentHints.baseDecodeTimeStartLo,
      );
      const transmuxedData = sourceBufferObj.transmuxer.transmuxSegment(
        segment,
        {
          reset: segmentHints.resetTransmuxerState,
          baseMediaDecodeTimeSeed: {
            value: dtsHint,
            timescale: segmentHints.baseDecodeTimeStartTimescale,
          },
        },
      );
      if (transmuxedData !== null) {
        segment = transmuxedData.data;
        if (transmuxedData.timingInfo !== undefined) {
          if (logger.hasLevel(LogLevel.Debug)) {
            const startString = (
              transmuxedData.timingInfo.start /
              transmuxedData.timingInfo.timescale
            ).toFixed(3);
            const endString = (
              transmuxedData.timingInfo.end /
              transmuxedData.timingInfo.timescale
            ).toFixed(3);
            const hint = (
              dtsHint / segmentHints.baseDecodeTimeStartTimescale
            ).toFixed(3);
            logger.debug(
              `Worker: transmuxed segment with start=${startString} end=${endString} hinted=${hint}`,
            );
          }
          segmentPreciseTiming = {
            start: splitTimeValue(transmuxedData.timingInfo.start),
            end: splitTimeValue(transmuxedData.timingInfo.end),
            timescale: transmuxedData.timingInfo.timescale,
          };
        } else {
          logger.warn("Worker: transmuxed segment with no timing info");
        }
      } else {
        return AppendBufferResult.error(
          SegmentParsingErrorCode.TransmuxerError,
          "Segment preparation error: the transmuxer couldn't process the segment",
        );
      }
    } catch (err) {
      const msg = formatErrMessage(
        err,
        "Unknown error while transmuxing segment",
      );
      return AppendBufferResult.error(
        SegmentParsingErrorCode.TransmuxerError,
        msg,
      );
    }
  }

  const initTrackInfoByTrackId = getInitTrackInfo(segment);
  if (initTrackInfoByTrackId !== undefined) {
    // TODO: In transmuxing step when possible?
    sourceBufferObj.lastInitTrackInfoByTrackId = initTrackInfoByTrackId;
  }

  if (
    segmentPreciseTiming === undefined &&
    sourceBufferObj.lastInitTrackInfoByTrackId
  ) {
    const timeInfo = getTimeInformationFromMp4(
      segment,
      sourceBufferObj.lastInitTrackInfoByTrackId,
    );

    if (timeInfo) {
      segmentPreciseTiming = {
        start: splitTimeValue(timeInfo.time),
        timescale: timeInfo.timescale,
        end:
          timeInfo.duration !== undefined
            ? splitTimeValue(timeInfo.time + timeInfo.duration)
            : undefined,
      };
    }
  }
  const transferableSegment = new Uint8Array(segment);
  try {
    if (sourceBufferObj.sourceBuffer !== null) {
      sourceBufferObj.sourceBuffer
        .push(transferableSegment)
        .then(() => {
          try {
            const timeRange = sourceBufferObj.sourceBuffer.getBufferedRanges();
            const buffered = new JsTimeRanges(
              timeRangesToFloat64Array(timeRange),
            );
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_update(sourceBufferId, buffered);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_update`", error);
          }
        })
        .catch((err) => {
          if (err instanceof SourceBufferOperationCancelledError) {
            logger.info("Worker: Ignoring cancelled appendBuffer operation");
            return;
          }
          logger.warn(
            `Worker: appendBuffer failed sb=${sourceBufferId} mediaType=${sourceBufferObj.mediaType} resource=${resourceId} bytes=${transferableSegment.byteLength} err=${formatErrMessage(err, "Unknown appendBuffer error")}`,
          );
          try {
            let buffered;
            try {
              const timeRange =
                sourceBufferObj.sourceBuffer.getBufferedRanges();
              buffered = new JsTimeRanges(timeRangesToFloat64Array(timeRange));
            } catch (_) {
              buffered = new JsTimeRanges(new Float64Array([]));
            }
            if (err instanceof Error && err.name === "QuotaExceededError") {
              playerInstance
                .getDispatcher()
                ?.on_append_buffer_error(
                  sourceBufferId,
                  PushedSegmentErrorCode.BufferFull,
                  buffered,
                );
            } else {
              playerInstance
                .getDispatcher()
                ?.on_append_buffer_error(
                  sourceBufferId,
                  PushedSegmentErrorCode.UnknownError,
                  buffered,
                );
            }
          } catch (err2) {
            const error = err2 instanceof Error ? err2 : "Unknown Error";
            logger.error("Error when calling `on_append_buffer_error`", error);
          }
        });
    } else {
      const buffer = transferableSegment.buffer;
      postMessageToMain(
        {
          type: WorkerMessageType.AppendBuffer,
          value: {
            mediaSourceId: mediaSourceObj.mediaSourceId,
            sourceBufferId,
            data: buffer,
          },
        },
        [buffer],
      );
    }
  } catch (_err) {
    return AppendBufferResult.error(SegmentParsingErrorCode.UnknownError);
  }
  return AppendBufferResult.success(
    segmentPreciseTiming?.start,
    segmentPreciseTiming?.end,
    segmentPreciseTiming?.timescale,
  );
}

/**
 * Recuperate more information on a segment, identified by its `ResourceId`.
 *
 * This can generally be relied on to e.g. obtain the `codec` and mime-type
 * directly from segment metadata.
 *
 * @param resourceId - `ResourceId` of the segment you want more metadata from
 * @returns - Object describing the result of the operation.
 */
export function inspectSegment(resourceId: ResourceId): {
  value: InspectSegmentValue | undefined;
  errorCode: SegmentParsingErrorCode | undefined;
  description: string | undefined;
} {
  const segment = jsMemoryResources.get(resourceId);
  if (segment === undefined) {
    return {
      value: undefined,
      errorCode: SegmentParsingErrorCode.NoResource,
      description:
        "Segment inspection error: No resource with the given `resourceId`",
    };
  }

  const inspection = inspectProbeSegment(segment);
  if (inspection === undefined) {
    return {
      value: undefined,
      errorCode: SegmentParsingErrorCode.UnknownError,
      description:
        "Segment inspection error: no codec metadata was found in the probe segment",
    };
  }
  return {
    value: inspection,
    errorCode: undefined,
    description: undefined,
  };
}

function inspectProbeSegment(
  segment: Uint8Array,
): InspectSegmentValue | undefined {
  const codecs = getIsoBmffCodecs(segment);
  if (codecs.length > 0) {
    const mediaType = inferMediaTypeFromCodecs(codecs);
    return {
      mediaType,
      mimeType: mediaType === MediaType.Audio ? "audio/mp4" : "video/mp4",
      codec: codecs.join(","),
    };
  }

  const transmuxedSegment = createTransmuxer().transmuxSegment(segment);
  if (transmuxedSegment === null) {
    return undefined;
  }
  const transmuxedCodecs = getIsoBmffCodecs(transmuxedSegment.data);
  if (transmuxedCodecs.length === 0) {
    logger.error("Worker: transmuxed segment is null");
    return undefined;
  }
  const mediaType = inferMediaTypeFromCodecs(transmuxedCodecs);
  if (isLikelyAacProbeSegment(segment)) {
    return {
      mediaType,
      mimeType: "audio/aac",
      codec: transmuxedCodecs.join(","),
    };
  }
  if (isLikelyMpeg2TsProbeSegment(segment)) {
    return {
      mediaType,
      mimeType: mediaType === MediaType.Audio ? "audio/mp2t" : "video/mp2t",
      codec: transmuxedCodecs.join(","),
    };
  }
  return undefined;
}

function inferMediaTypeFromCodecs(codecs: string[]): MediaType {
  return codecs.some((codec) => !isAudioCodec(codec))
    ? MediaType.Video
    : MediaType.Audio;
}

function isAudioCodec(codec: string): boolean {
  return /^(mp4a|ac-3|ec-3|ac-4|opus|flac|alac)\b/i.test(codec);
}

function isLikelyAacProbeSegment(data: Uint8Array): boolean {
  const offset = getId3Offset(data, 0);
  return (
    data.length >= offset + 2 &&
    (data[offset] & 0xff) === 0xff &&
    (data[offset + 1] & 0xf0) === 0xf0 &&
    (data[offset + 1] & 0x16) === 0x10
  );
}

function getId3Offset(data: Uint8Array, initialOffset: number): number {
  if (
    data.length - initialOffset < 10 ||
    data[initialOffset] !== 0x49 ||
    data[initialOffset + 1] !== 0x44 ||
    data[initialOffset + 2] !== 0x33
  ) {
    return initialOffset;
  }

  const flags = data[initialOffset + 5];
  const footerPresent = (flags & 16) >> 4;
  const size =
    (data[initialOffset + 6] << 21) |
    (data[initialOffset + 7] << 14) |
    (data[initialOffset + 8] << 7) |
    data[initialOffset + 9];
  const tagSize = Math.max(0, size) + (footerPresent === 1 ? 20 : 10);
  return getId3Offset(data, initialOffset + tagSize);
}

function isLikelyMpeg2TsProbeSegment(data: Uint8Array): boolean {
  if (data.length < 188 || data[0] !== 0x47) {
    return false;
  }
  return (
    data.length === 188 ||
    data[188] === 0x47 ||
    (data.length >= 377 && data[376] === 0x47)
  );
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
  end: number,
): RemoveBufferResult {
  try {
    const mediaSourceObj = getMediaSourceObj();
    if (mediaSourceObj === undefined) {
      return RemoveBufferResult.error(
        RemoveBufferErrorCode.SourceBufferNotFound,
        "No MediaSource created.",
      );
    }

    if (mediaSourceObj.type === "worker") {
      const sourceBufferObj = mediaSourceObj.sourceBuffers.find(
        ({ id }) => id === sourceBufferId,
      );
      if (sourceBufferObj === undefined) {
        return RemoveBufferResult.error(
          RemoveBufferErrorCode.SourceBufferNotFound,
          "SourceBuffer linked to the given id not found.",
        );
      }
      sourceBufferObj.sourceBuffer
        .removeBuffer(start, end)
        .then(() => {
          try {
            const timeRange = sourceBufferObj.sourceBuffer.getBufferedRanges();
            const buffered = new JsTimeRanges(
              timeRangesToFloat64Array(timeRange),
            );
            playerInstance
              .getDispatcher()
              ?.on_source_buffer_update(sourceBufferId, buffered);
          } catch (err) {
            const error = err instanceof Error ? err : "Unknown Error";
            logger.error("Error when calling `on_source_buffer_update`", error);
          }
        })
        .catch((err) => {
          if (err instanceof SourceBufferOperationCancelledError) {
            logger.info("Worker: Ignoring cancelled removeBuffer operation");
            return;
          }
          let buffered;
          try {
            const timeRange = sourceBufferObj.sourceBuffer.getBufferedRanges();
            buffered = new JsTimeRanges(timeRangesToFloat64Array(timeRange));
          } catch (_) {
            buffered = new JsTimeRanges(new Float64Array([]));
          }
          try {
            playerInstance
              .getDispatcher()
              ?.on_remove_buffer_error(sourceBufferId, buffered);
          } catch (err2) {
            const error = err2 instanceof Error ? err2 : "Unknown Error";
            logger.error("Error when calling `on_remove_buffer_error`", error);
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
        "There's no MediaSource attached currently.",
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
      "Unknown error while calling endOfStream",
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
 * @param {Map<number, Object>} initTrackInfoByTrackId
 * @returns {Object|null}
 */
function getTimeInformationFromMp4(
  segment: Uint8Array,
  initTrackInfoByTrackId: Map<
    number,
    {
      timescale: number;
      type: "audio" | "video" | "other";
      defaultSampleDuration: number | undefined;
    }
  >,
): { time: number; duration: number | undefined; timescale: number } | null {
  return getIsobmfTimeInfo(segment, initTrackInfoByTrackId);
}

function splitTimeValue(value: number): ISafeU64 {
  return {
    hi: Math.floor(value / 0x100000000),
    lo: value >>> 0,
  };
}

function combineSafeTimeValue(hi: number, lo: number): number {
  return hi * 0x100000000 + lo;
}

/**
 * @param {number|undefined} minimumPosition
 * @param {number|undefined} maximumPosition
 * @param {number} playlistType
 */
export function updateContentInfo(
  minimumPosition: number | undefined,
  maximumPosition: number | undefined,
  playlistType: PlaylistNature,
  isFinalized: boolean,
  usesProgramDateTime: boolean,
): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.ContentInfoUpdate,
    value: {
      contentId: contentInfo.contentId,
      minimumPosition,
      maximumPosition,
      playlistType,
      isFinalized,
      usesProgramDateTime,
    },
  });
}

export function announceFetchedContent(
  playlistType: PlaylistType,
  variantInfo: Uint32Array,
  audioTracksInfo: Uint32Array,
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
      const id = variantInfo[i];
      i++;

      const height = variantInfo[i];
      i++;

      const width = variantInfo[i];
      i++;

      const frameRate = variantInfo[i];
      i++;

      const bandwidth = variantInfo[i];
      i++;

      const videoRangeLen = variantInfo[i];
      i++;
      const videoRangeU8 = new Uint8Array(
        memory.buffer,
        variantInfo[i],
        videoRangeLen,
      );
      i++;
      const videoRange = cachedTextDecoder.decode(videoRangeU8);

      variantInfoObj.push({
        id,
        height: height === 0 ? undefined : height,
        width: width === 0 ? undefined : width,
        frameRate: frameRate === 0 ? undefined : frameRate,
        bandwidth: bandwidth === 0 ? undefined : bandwidth,
        videoRange: videoRange === "" ? undefined : videoRange,
      });
    }
  }
  const audioTracksObj: AudioTrackInfo[] = [];
  {
    let i = 0;
    i++; // Skip number of audio tracks
    while (i < audioTracksInfo.length) {
      const id = audioTracksInfo[i];
      i++;

      const languageLen = audioTracksInfo[i];
      i++;
      const languageU8 = new Uint8Array(
        memory.buffer,
        audioTracksInfo[i],
        languageLen,
      );
      i++;
      const language = cachedTextDecoder.decode(languageU8);

      const assocLanguageLen = audioTracksInfo[i];
      i++;
      const assocLanguageU8 = new Uint8Array(
        memory.buffer,
        audioTracksInfo[i],
        assocLanguageLen,
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

      const characteristicsCount = audioTracksInfo[i];
      i++;
      const characteristics: string[] = [];
      for (let j = 0; j < characteristicsCount; j++) {
        const characteristicLen = audioTracksInfo[i];
        i++;
        const characteristicU8 = new Uint8Array(
          memory.buffer,
          audioTracksInfo[i],
          characteristicLen,
        );
        i++;
        characteristics.push(cachedTextDecoder.decode(characteristicU8));
      }

      const bitDepth = audioTracksInfo[i];
      i++;

      const sampleRate = audioTracksInfo[i];
      i++;

      const bitDepthCount = audioTracksInfo[i];
      i++;
      const bitDepths: number[] = [];
      for (let j = 0; j < bitDepthCount; j++) {
        bitDepths.push(audioTracksInfo[i]);
        i++;
      }

      const sampleRateCount = audioTracksInfo[i];
      i++;
      const sampleRates: number[] = [];
      for (let j = 0; j < sampleRateCount; j++) {
        sampleRates.push(audioTracksInfo[i]);
        i++;
      }

      audioTracksObj.push({
        id,
        language: language === "" ? undefined : language,
        assocLanguage: assocLanguage === "" ? undefined : assocLanguage,
        name,
        channels: channels === 0 ? undefined : channels,
        characteristics:
          characteristics.length === 0 ? undefined : characteristics,
        bitDepth: bitDepth === 0 ? undefined : bitDepth,
        sampleRate: sampleRate === 0 ? undefined : sampleRate,
        bitDepths: bitDepths.length === 0 ? undefined : bitDepths,
        sampleRates: sampleRates.length === 0 ? undefined : sampleRates,
      });
    }
  }
  postMessageToMain({
    type: WorkerMessageType.TopLevelPlaylistParsed,
    value: {
      contentId: contentInfo.contentId,
      playlistType,
      variants: variantInfoObj,
      audioTracks: audioTracksObj,
    },
  });
}

export function announceTrackUpdate(
  mediaType: MediaType,
  currentAudioTrack: number | undefined,
  isAudioTrackSelected: boolean,
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
      audioTrack:
        currentAudioTrack !== undefined
          ? {
              current: currentAudioTrack,
              isSelected: isAudioTrackSelected,
            }
          : undefined,
    },
  });
}

export function announceVariantUpdate(variantId: number | undefined): void {
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
      "Attempting to start rebuffering when no MediaSource is created",
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
      "Attempting to stop rebuffering when no MediaSource is created",
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
  codec: string,
): boolean | undefined {
  const mimeType = getFmp4Type(mediaType, codec);

  // TODO keep somewhere which one is supported to be able to know if
  // transmuxing is necessary or not
  if (playerInstance.hasMseInWorker() === true) {
    return MediaSource.isTypeSupported(mimeType);
  }
  const cached = cachedCodecsSupport.get(mimeType);
  if (cached !== undefined) {
    return cached;
  }

  codecsToAskForSupport.add(mimeType);
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

export function announceVariantLockStatusChange(
  variantId: number | undefined,
): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return;
  }
  postMessageToMain({
    type: WorkerMessageType.VariantLockStatusChange,
    value: {
      contentId: contentInfo.contentId,
      lockedVariant: variantId ?? null,
    },
  });
}

export function getWaspHostCapabilities(): HostBindings {
  return {
    log,
    timer,
    clearTimer,
    getResourceData,
    fetch: doFetch,
    abortRequest,
    attachMediaSource,
    removeMediaSource,
    setMediaSourceDuration,
    addSourceBuffer,
    isTypeSupported,
    inspectSegment,
    appendBuffer,
    removeBuffer,
    endOfStream,
    startObservingPlayback,
    stopObservingPlayback,
    freeResource,
    setPlaybackRate,
    seek,
    flush,
    setMediaOffset,
    updateContentInfo,
    announceFetchedContent,
    announceVariantUpdate,
    announceTrackUpdate,
    announceVariantLockStatusChange,
    startRebuffering,
    stopRebuffering,
    getRandom,
    sendSegmentRequestError,
    sendMultivariantPlaylistRequestError,
    sendMediaPlaylistRequestError,
    sendSourceBufferCreationError,
    sendMultivariantPlaylistParsingError,
    sendMediaPlaylistParsingError,
    sendSegmentParsingError,
    sendPushedSegmentError,
    sendRemoveBufferError,
    sendOtherError,
    sendContentCompatibilityError,
  };
}
