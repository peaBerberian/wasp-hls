import idGenerator from "../ts-common/idGenerator.js";
import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer.js";
import {
  Dispatcher,
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
} from "../wasm/wasp_hls.js";
import {
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
import {
  getTransmuxedType,
  shouldTransmux,
  transmux,
} from "./transmux.js";
import { formatErrMessage } from "./utils.js";

const generateMediaSourceId = idGenerator();

/**
 * @param {number} resourceId
 * @returns {Uint8Array|undefined}
 */
export function getResourceData(
  resourceId: ResourceId
) : Uint8Array | undefined {
  return jsMemoryResources.get(resourceId);
}

/**
 * @param {number} logLevel
 * @param {string} logStr
 */
export function log(logLevel: LogLevel, logStr: string) {
  const now = performance.now().toFixed(2);
  switch (logLevel) {
    case LogLevel.Error:
      console.error(now, logStr);
      break;
    case LogLevel.Warn:
      console.warn(now, logStr);
      break;
    case LogLevel.Info:
      console.info(now, logStr);
      break;
    case LogLevel.Debug:
      console.debug(now, logStr);
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
 * TODO failure cases
 * @param {string} url
 * @returns {number}
 */
export function doFetch(url: string): RequestId {
  const abortController = new AbortController();
  const currentRequestId = requestsStore.create({ abortController });
  const timestampBef = performance.now();
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      const arrRes = await res.arrayBuffer();
      const elapsedMs = performance.now() - timestampBef;
      requestsStore.delete(currentRequestId);
      const dispatcher = playerInstance.getDispatcher();
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
    .catch(err => {
      requestsStore.delete(currentRequestId);
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
      // Here call the right WASM callback
    });
  return currentRequestId;
}

/**
 * @param {number} id
 * @returns {boolean}
 */
export function abortRequest(id: RequestId) : boolean {
  const requestObj = requestsStore.get(id);
  if (requestObj !== undefined) {
    requestObj.abortController.abort();

    // NOTE: we prefer deleting the id on a microtask to avoid possible RequestId
    // conflicts due to other microtask pending while this `abortRequest` call was
    // made (e.g. what if a request failure associated to that request was already
    // scheduled yet another request is made synchronously with the same RequestId?).
    /* eslint-disable-next-line @typescript-eslint/no-floating-promises */
    Promise.resolve().then(() => { requestsStore.delete(id); });
    return true;
  }
  return false;
}

/**
 * @param {number} warningCode
 */
export function warning(warningCode: WarningCode): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return ;
  }
  postMessageToMain({
    type: "content-warning",
    value: {
      contentId: contentInfo.contentId,
      code: warningCode,
    },
  });
}

/**
 * @param {number} position
 */
export function seek(position: number): void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null || contentInfo.mediaSourceObj === null) {
    console.error("Attempting to seek when no MediaSource is created");
    return ;
  }
  postMessageToMain({
    type: "seek",
    value: {
      mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
      position,
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
    if (playerInstance.hasWorkerMse !== true) {
      const mediaSourceId = generateMediaSourceId();
      contentInfo.mediaSourceObj = {
        nextSourceBufferId: 0,
        sourceBuffers: [],
        type: "main",
        mediaSourceId,
      };
      postMessageToMain({
        type: "create-media-source",
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
        objectURL = URL.createObjectURL(mediaSource);
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
      postMessageToMain({
        type: "attach-media-source",
        value: {
          contentId: contentInfo.contentId,
          /* eslint-disable-next-line */
          handle,
          src: objectURL,
          mediaSourceId,
        },
      }, handle !== undefined ? [handle] : []);
    }

    function onMediaSourceEnded(): void {
      playerInstance.getDispatcher()?.on_media_source_state_change(
        MediaSourceReadyState.Ended
      );
    }
    function onMediaSourceOpen(): void {
      playerInstance.getDispatcher()?.on_media_source_state_change(
        MediaSourceReadyState.Open
      );
    }
    function onMediaSourceClose(): void {
      playerInstance.getDispatcher()?.on_media_source_state_change(
        MediaSourceReadyState.Closed
      );
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
    const {
      mediaSource,
      removeEventListeners,
    } = contentInfo.mediaSourceObj;
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
          }
          catch (e) {
            const msg =
              formatErrMessage(e, "Unknown error while removing SourceBuffer");
            Dispatcher.log(LogLevel.Error, "Could not remove SourceBuffer: " + msg);
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
    type: "clear-media-source",
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
    return  MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached
    );
  }
  if (contentInfo.mediaSourceObj === null) {
    return  MediaSourceDurationUpdateResult.error(
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
      type: "update-media-source-duration",
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
export function getErrorMessage(e: unknown) : string | undefined {
  return e instanceof Error ?
    e.message :
    undefined;
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
    const {
      sourceBuffers,
      nextSourceBufferId,
    } = contentInfo.mediaSourceObj;
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
        type: "create-source-buffer",
        value: {
          mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
          sourceBufferId,
          contentType: mimeType,
        },
      });
      return AddSourceBufferResult.success(sourceBufferId);
    } catch (err) {
      const msg =
        formatErrMessage(err, "Unknown error while creating Sourcebuffer");
      return AddSourceBufferResult.error(
        AddSourceBufferErrorCode.UnknownError,
        msg
      );
    }
  } else {
    const {
      mediaSource,
      sourceBuffers,
      nextSourceBufferId,
    } = contentInfo.mediaSourceObj;
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
      const msg =
        formatErrMessage(err, "Unknown error while creating Sourcebuffer");
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
  const sourceBufferObjIdx = mediaSourceObj.sourceBuffers
    .findIndex(({ id }) => id === sourceBufferId);
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
      const msg =
        formatErrMessage(err, "Unknown error while transmuxing segment");
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
      sourceBufferObj.sourceBuffer.push(segment)
        .then(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_update`", err);
          }
        })
        .catch(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_error`", err);
          }
        });
    } else {
      const buffer = segment.buffer;
      postMessageToMain({
        type: "append-buffer",
        value: {
          mediaSourceId: mediaSourceObj.mediaSourceId,
          sourceBufferId,
          data: buffer,
        },
      }, [buffer]);
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
      const sourceBuffer = mediaSourceObj.sourceBuffers
        .find(({ id }) => id === sourceBufferId);
      if (sourceBuffer === undefined) {
        return RemoveBufferResult.error(
          RemoveBufferErrorCode.SourceBufferNotFound,
          "SourceBuffer linked to the given id not found."
        );
      }
      sourceBuffer.sourceBuffer.removeBuffer(start, end)
        .then(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_update`", err);
          }
        })
        .catch(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_error`", err);
          }
        });
    } else {
      postMessageToMain({
        type: "remove-buffer",
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
    return RemoveBufferResult.error(
      RemoveBufferErrorCode.UnknownError,
      msg
    );
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
        type: "end-of-stream",
        value: { mediaSourceId: mediaSourceObj.mediaSourceId },
      });
    }
  } catch (err) {
    const msg = formatErrMessage(err, "Unknown error while calling endOfStream");
    return EndOfStreamResult.error(
      EndOfStreamErrorCode.UnknownError,
      msg
    );
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
    Dispatcher.log(
      LogLevel.Error,
      "Cannot start observing playback: No MediaSource Attached"
    );
    return;
  }
  postMessageToMain({
    type: "start-playback-observation",
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
    type: "stop-playback-observation",
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
    type: "media-offset-update",
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
export function freeResource(resourceId: ResourceId) : boolean {
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
) : ({ time: number; duration: number | undefined }) | null {
  const baseDecodeTime = getTrackFragmentDecodeTime(segment);
  if (baseDecodeTime === undefined) {
    return null;
  }
  const trunDuration = getDurationFromTrun(segment);
  return {
    time: baseDecodeTime / initTimescale,
    duration: trunDuration === undefined ?
      undefined :
      trunDuration / initTimescale,
  };
}

/**
 * @param {number|undefined} minimumPosition
 * @param {number|undefined} maximumPosition
 */
export function updateContentInfo(
  minimumPosition: number | undefined,
  maximumPosition: number | undefined
) : void {
  const contentInfo = playerInstance.getContentInfo();
  if (contentInfo === null) {
    return ;
  }
  postMessageToMain({
    type: "content-info-update",
    value: {
      contentId: contentInfo.contentId,
      minimumPosition,
      maximumPosition,
    },
  });
}

// TODO real way of binding
/* eslint-disable */
const global = self as any;
global.jsAbortRequest = abortRequest;
global.jsAddSourceBuffer = addSourceBuffer;
global.jsAppendBuffer = appendBuffer;
global.jsAttachMediaSource = attachMediaSource;
global.jsClearTimer = clearTimer;
global.jsEndOfStream = endOfStream;
global.jsFetch = doFetch;
global.jsFreeResource = freeResource;
global.jsGetResourceData = getResourceData;
global.jsLog = log;
global.jsRemoveBuffer = removeBuffer;
global.jsRemoveMediaSource = removeMediaSource;
global.jsSeek = seek;
global.jsSetMediaOffset = setMediaOffset;
global.jsSetMediaSourceDuration = setMediaSourceDuration;
global.jsStartObservingPlayback = startObservingPlayback;
global.jsStopObservingPlayback = stopObservingPlayback;
global.jsTimer = timer;
global.jsUpdateContentInfo = updateContentInfo;
global.jsWarning = warning;
