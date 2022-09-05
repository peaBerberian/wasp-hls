import {
  Dispatcher,
  LogLevel,
  MediaType,
  MediaSourceReadyState,
  // PlaybackTickReason,
  // RemoveMediaSourceResult,
  // RemoveMediaSourceErrorCode,
  // AddSourceBufferResult,
  // AddSourceBufferErrorCode,
  // AppendBufferResult,
  // RemoveBufferResult,
  // AppendBufferErrorCode,
  // RemoveBufferErrorCode,
  // MediaSourceDurationUpdateResult,
  // MediaSourceDurationUpdateErrorCode,
  // EndOfStreamResult,
  // EndOfStreamErrorCode,
  // MediaObservation,
} from "../wasm/wasp_hls.js";
import {
  WorkerMediaSourceInstanceInfo,
  MainMediaSourceInstanceInfo,
  jsMemoryResources,
  PlayerId,
  playersStore,
  RequestId,
  requestsStore,
  ResourceId,
  SourceBufferId,
} from "./globals";
import postMessageToMain from "./postMessage.js";
import {
  getTransmuxedType,
  shouldTransmux,
  transmux,
} from "./transmux.js";

function IdGenerator() {
  let id = 0;
  return function generateNewId() {
    // TODO more intelligent id with notion of which is still in use, or
    // infinite string-based?
    if (id >= Number.MAX_SAFE_INTEGER) {
      id = 0;
    }
    return id++;
  };
}

const generateMediaSourceId = IdGenerator();

const MAX_U32 = Math.pow(2, 32) - 1;

let nextRequestId = 0;
let nextResourceId = 0;

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
 * TODO failure cases
 * @param {number} playerId
 * @param {string} url
 * @returns {number}
 */
export function fetchU8(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = nextRequestId;
  incrementRequestId();
  const abortController = new AbortController();
  requestsStore.create(currentRequestId, { playerId, abortController });
  const timestampBef = performance.now();
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      if (abortController.signal.aborted) {
        return; // Should not be possible. Still, exit if that's the case.
      }
      const arrRes = await res.arrayBuffer();
      const elapsedMs = performance.now() - timestampBef;
      requestsStore.delete(currentRequestId);
      const playerObj = playersStore.get(playerId);
      if (playerObj !== undefined) {
        playerObj.dispatcher
          .on_u8_request_finished(currentRequestId,
                                  new Uint8Array(arrRes),
                                  res.url,
                                  elapsedMs);
      }
    })
    .catch(err => {
      if (abortController.signal.aborted) {
        return;
      }
      requestsStore.delete(currentRequestId);
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
      // Here call the right WASM callback
    });
  return currentRequestId;
}

/**
 * TODO failure cases
 * @param {number} playerId
 * @param {string} url
 * @returns {number}
 */
export function fetchU8NoCopy(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = nextRequestId;
  incrementRequestId();
  const abortController = new AbortController();
  requestsStore.create(currentRequestId, { playerId, abortController });
  const timestampBef = performance.now();
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      const arrRes = await res.arrayBuffer();
      const elapsedMs = performance.now() - timestampBef;
      requestsStore.delete(currentRequestId);
      const playerObj = playersStore.get(playerId);
      if (playerObj !== undefined) {
        const currentResourceId = nextResourceId;
        incrementResourceId();
        const segmentArray = new Uint8Array(arrRes);
        jsMemoryResources.create(currentResourceId, playerId, segmentArray);
        playerObj.dispatcher
          .on_u8_no_copy_request_finished(currentRequestId,
                                          currentResourceId,
                                          segmentArray.byteLength,
                                          res.url,
                                          elapsedMs);
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

export function seek(playerId: PlayerId, position: number) {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    // XXX TODO
    return ;
  }
  postMessageToMain({ type: "seek", value: position });
}

/**
 * @param {number} playerId
 * @returns {number}
 */
export function attachMediaSource(playerId: PlayerId): void {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    // TODO
    return;
  }

  try {
    if (!playerObj.hasWorkerMse) {
      const mediaSourceId = generateMediaSourceId();
      playerObj.mediaSourceObj = {
        nextSourceBufferId: 0,
        sourceBuffers: [],
        type: "main",
        mediaSourceId,
      };
      postMessageToMain({
        type: "create-media-source",
        value: {
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
      playerObj.mediaSourceObj = {
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
          /* eslint-disable-next-line */
          handle,
          src: objectURL,
          mediaSourceId,
        },
      }, handle !== undefined ? [handle] : []);
    }

    function onMediaSourceEnded() {
      playerObj?.dispatcher
        .on_media_source_state_change(MediaSourceReadyState.Ended);
    }
    function onMediaSourceOpen() {
      playerObj?.dispatcher
        .on_media_source_state_change(MediaSourceReadyState.Open);
    }
    function onMediaSourceClose() {
      playerObj?.dispatcher
        .on_media_source_state_change(MediaSourceReadyState.Closed);
    }
  } catch (e) {
    // TODO
    scheduleMicrotask(() => {
      // playerObj?.dispatcher.on_media_source_creation_error(
      //   getErrorMessage(e)
      // );
    });
  }
}

function scheduleMicrotask(fn: () => unknown) : void {
  if (typeof queueMicrotask === "function") {
    queueMicrotask(fn);
  } else {
    Promise.resolve().then(fn).catch(() => {
      /* noop*/
    });
  }
}

/**
 * @param {number} playerId
 * @returns {number}
 */
export function removeMediaSource(playerId: PlayerId): void {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    // TODO
    return;
    // return RemoveMediaSourceResult
    //   .error(RemoveMediaSourceErrorCode.PlayerInstanceNotFound);
  }
  if (playerObj.mediaSourceObj === null) {
    // TODO
    return;
    // return RemoveMediaSourceResult
    //   .error(RemoveMediaSourceErrorCode.NoMediaSourceAttached);
  }

  if (playerObj.mediaSourceObj.type === "worker") {
    const {
      mediaSource,
      removeEventListeners,
    } = playerObj.mediaSourceObj;
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
            // TODO?
            const msg =
              formatErrMessage(e, "Unknown error while removing SourceBuffer");
            Dispatcher.log(LogLevel.Error, "Could not remove SourceBuffer: " + msg);
          }
        }
      }
    }
    // clearElementSrc(playerObj.videoElement);
    // // if (objectURL !== null) {
    // //   try {
    // //     URL.revokeObjectURL(objectURL);
    // //   } catch (e) {
    // //       // TODO proper WASM communication?
    // //     const msg = formatErrMessage(e, "Unknown error while revoking ObjectURL");
    // //     Dispatcher.log(LogLevel.Error, "Could not revoke ObjectURL: " + msg);
    // //   }
    // // }
  }

  postMessageToMain({
    type: "clear-media-source",
    value: { mediaSourceId: playerObj.mediaSourceObj.mediaSourceId },
  });
}

export function setMediaSourceDuration(
  playerId: number,
  duration: number
) : void {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    // TODO
    return ;
  }
  if (playerObj.mediaSourceObj === null) {
    // TODO
    return;
  }

  if (playerObj.mediaSourceObj.type === "worker") {
    try {
      playerObj.mediaSourceObj.mediaSource.duration = duration;
    } catch (err) {
      // TODO
    }
  } else {
    postMessageToMain({
      type: "update-media-source-duration",
      value: {
        mediaSourceId: playerObj.mediaSourceObj.mediaSourceId,
        duration,
      },
    });
  }
}

export function getErrorMessage(e: unknown) : string | undefined {
  return e instanceof Error ?
    e.message :
    undefined;
}

/**
 * @param {number} playerId
 * @param {number} mediaType
 * @param {string} typ
 * @returns {Object}
 */
export function addSourceBuffer(
  playerId: PlayerId,
  mediaType: MediaType,
  typ: string
): number {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    // TODO
    throw new Error("Error 1");
  }
  if (playerObj.mediaSourceObj === null) {
    // TODO
    throw new Error("Error 2");
  }

  if (playerObj.mediaSourceObj.type === "main") {
    const {
      sourceBuffers,
      nextSourceBufferId,
    } = playerObj.mediaSourceObj;
    try {
      let mimeType = typ;
      if (shouldTransmux(typ)) {
        mimeType = getTransmuxedType(typ, mediaType);
      }
      const sourceBufferId = nextSourceBufferId;
      sourceBuffers.push({
        id: sourceBufferId,
        transmuxer: mimeType === typ ? null : transmux,
        sourceBuffer: null,
      });
      playerObj.mediaSourceObj.nextSourceBufferId++;
      postMessageToMain({
        type: "create-source-buffer",
        value: {
          mediaSourceId: playerObj.mediaSourceObj.mediaSourceId,
          sourceBufferId,
          contentType: mimeType,
        },
      });
      return sourceBufferId;
    } catch (err) {
      // TODO
      throw err;
    }
  } else {
    const {
      mediaSource,
      sourceBuffers,
      nextSourceBufferId,
    } = playerObj.mediaSourceObj;
    if (mediaSource.readyState === "closed") {
      // TODO
      throw new Error("A");
    }
    if (typ === "") {
      // TODO
      throw new Error("B");
    }
    try {
      let mimeType = typ;
      if (shouldTransmux(typ)) {
        mimeType = getTransmuxedType(typ, mediaType);
      }
      const sourceBuffer = mediaSource.addSourceBuffer(mimeType);
      const sourceBufferId = nextSourceBufferId;
      sourceBuffers.push({
        id: sourceBufferId,
        sourceBuffer,
        transmuxer: mimeType === typ ? null : transmux,
      });
      playerObj.mediaSourceObj.nextSourceBufferId++;
      sourceBuffer.addEventListener("updateend", function() {
        playerObj.dispatcher.on_source_buffer_update(sourceBufferId);
      });
      // TODO
      return sourceBufferId;
    } catch (err) {
      throw new Error("C");
      // if (!(err instanceof Error)) {
      //   // TODO
      //   return;
      // } else if (err.name === "QuotaExceededError") {
      //   // TODO
      //   return;
      // } else if (err.name === "NotSupportedError") {
      //   // TODO
      //   return;
      // } else {
      //   // TODO
      //   return;
      // }
    }
  }
}

/**
 * @param {number} playerId
 * @param {number} sourceBufferId
 * @param {ArrayBuffer} data
 * @returns {number}
 */
export function appendBuffer(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId,
  data: Uint8Array
): void {
  try {
    const mediaSourceObj = getMediaSourceObj(playerId);
    if (mediaSourceObj === undefined) {
      // TODO
      return;
    }

    // Weirdly enough TypeScript is only able to type-check when findIndex is
    // used then used as an index. Not when `find` is used directly.
    const sourceBufferObjIdx = mediaSourceObj.sourceBuffers
      .findIndex(({ id }) => id === sourceBufferId);
    if (sourceBufferObjIdx < 0) {
      // TODO
      return;
    }
    const sourceBufferObj = mediaSourceObj.sourceBuffers[sourceBufferObjIdx];
    let pushedData = data;
    if (sourceBufferObj.transmuxer !== null) {
      const transmuxedData = sourceBufferObj.transmuxer(data);

      // TODO specific error for transmuxing error
      if (transmuxedData !== null) {
        pushedData = transmuxedData;
      }
    }

    if (sourceBufferObj.sourceBuffer !== null) {
      sourceBufferObj.sourceBuffer.appendBuffer(pushedData);
    } else {
      const buffer = pushedData.buffer;
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
    // TODO
    return;
  }
}

/**
 * @param {number} playerId
 * @param {number} sourceBufferId
 * @param {number} resourceId
 * @returns {number}
 */
export function appendBufferJsBlob(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId,
  resourceId: ResourceId
): void {
  const segment: Uint8Array | undefined = jsMemoryResources.get(resourceId);
  if (segment === undefined) {
    // TODO
    return;
  }
  return appendBuffer(playerId, sourceBufferId, segment);
}

/**
 * @param {number} playerId
 * @param {number} sourceBufferId
 * @param {number} start
 * @param {number} end
 * @returns {number}
 */
export function removeBuffer(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId,
  start: number,
  end: number
): void {
  try {
    const mediaSourceObj = getMediaSourceObj(playerId);
    if (mediaSourceObj === undefined) {
      // TODO
      return;
    }

    if (mediaSourceObj.type === "worker") {
      const sourceBuffer = mediaSourceObj.sourceBuffers
        .find(({ id }) => id === sourceBufferId);
      if (sourceBuffer === undefined) {
        // TODO
        return;
      }
      sourceBuffer.sourceBuffer.remove(start, end);
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
    // TODO
    return ;
  }
}

/**
 * @param {number} playerId
 * @returns {number}
 */
export function endOfStream(
  playerId: PlayerId
): void {
  try {
    const mediaSourceObj = getMediaSourceObj(playerId);
    if (mediaSourceObj === undefined) {
      // TODO
      return;
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
    // TODO
    return;
  }
}

/**
 * @param {number} playerId
 */
export function startObservingPlayback(playerId: PlayerId): void {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return;
  }
  if (playerObj.mediaSourceObj === null) {
    // TODO error/log?
    return;
  }
  postMessageToMain({
    type: "start-playback-observation",
    value: { mediaSourceId: playerObj.mediaSourceObj.mediaSourceId },
  });
}

/**
 * @param {number} playerId
 */
export function stopObservingPlayback(playerId: PlayerId) {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return;
  }
  if (playerObj.mediaSourceObj === null) {
    // TODO error/log?
    return;
  }
  postMessageToMain({
    type: "stop-playback-observation",
    value: { mediaSourceId: playerObj.mediaSourceObj.mediaSourceId },
  });
}

export function freeResource(resourceId: number) : boolean {
  if (jsMemoryResources.get(resourceId) === undefined) {
    return false;
  }
  jsMemoryResources.delete(resourceId);
  return true;
}

function formatErrMessage(err: unknown, defaultMsg: string) {
  return err instanceof Error ?
    err.name + ": " + err.message :
    defaultMsg;
}

function getMediaSourceObj(
  playerId: PlayerId
) : MainMediaSourceInstanceInfo | WorkerMediaSourceInstanceInfo | undefined {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return undefined;
  }
  const { mediaSourceObj } = playerObj;
  if (mediaSourceObj === null) {
    return undefined;
  }
  return mediaSourceObj;
}

const MAX_LOOP_ITERATIONS = 1e6;

function incrementResourceId() : void {
  let iteration = 0;
  do {
    nextResourceId = nextResourceId >= MAX_U32 ? 0 : nextResourceId + 1;
    iteration++;
  } while (
    // TODO in store?
    jsMemoryResources.get(nextResourceId) !== undefined ||
    iteration >= MAX_LOOP_ITERATIONS
  );
  if (iteration >= MAX_LOOP_ITERATIONS) {
    throw new Error("Too many resources reserved. Is it normal?");
  }
}

function incrementRequestId() : void {
  let iteration = 0;
  do {
    nextRequestId = nextRequestId >= MAX_U32 ? 0 : nextRequestId + 1;
    iteration++;
  } while (
    // TODO in store?
    requestsStore.get(nextRequestId) !== undefined ||
    iteration >= MAX_LOOP_ITERATIONS
  );
  if (iteration >= MAX_LOOP_ITERATIONS) {
    throw new Error("Too many pending requests. Is it normal?");
  }
}

// TODO real way of binding
/* eslint-disable */
const global = self as any;
global.jsLog = log;
global.jsFetchU8 = fetchU8;
global.jsFetchU8NoCopy = fetchU8NoCopy;
global.jsAbortRequest = abortRequest;
global.jsAttachMediaSource = attachMediaSource;
global.jsRemoveMediaSource = removeMediaSource;
global.jsSetMediaSourceDuration = setMediaSourceDuration;
global.jsAddSourceBuffer = addSourceBuffer;
global.jsAppendBuffer = appendBuffer;
global.jsAppendBufferJsBlob = appendBufferJsBlob;
global.jsRemoveBuffer = removeBuffer;
global.jsEndOfStream = endOfStream;
global.jsStartObservingPlayback = startObservingPlayback;
global.jsStopObservingPlayback = stopObservingPlayback;
global.jsFreeResource = freeResource;
global.jsSeek = seek;
