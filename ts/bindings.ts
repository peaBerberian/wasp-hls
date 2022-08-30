import {
  Dispatcher,
  LogLevel,
  MediaType,
  MediaSourceReadyState,
  PlaybackTickReason,
  RemoveMediaSourceResult,
  RemoveMediaSourceErrorCode,
  AttachMediaSourceResult,
  AttachMediaSourceErrorCode,
  AddSourceBufferResult,
  AddSourceBufferErrorCode,
  AppendBufferResult,
  RemoveBufferResult,
  AppendBufferErrorCode,
  RemoveBufferErrorCode,
  MediaSourceDurationUpdateResult,
  MediaSourceDurationUpdateErrorCode,
  EndOfStreamResult,
  EndOfStreamErrorCode,
  MediaObservation,
} from "../wasm/wasp_hls.js";
import {
  jsMemoryResources,
  MediaSourceInstanceInfo,
  PlayerId,
  playersStore,
  RequestId,
  requestsStore,
  ResourceId,
  SourceBufferId,
  SourceBufferInstanceInfo,
} from "./globals.js";
import {
  getTransmuxedType,
  shouldTransmux,
  transmux,
} from "./transmux.js";

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

/**
 * @param {number} playerId
 * @returns {number}
 */
export function attachMediaSource(playerId: PlayerId): AttachMediaSourceResult {
  try {
    const playerObj = playersStore.get(playerId);
    if (playerObj === undefined) {
      return AttachMediaSourceResult.error(
        AttachMediaSourceErrorCode.PlayerInstanceNotFound
      );
    }
    const mediaSource = new MediaSource();
    mediaSource.addEventListener("sourceclose", onMediaSourceClose);
    mediaSource.addEventListener("sourceended", onMediaSourceEnded);
    mediaSource.addEventListener("sourceopen", onMediaSourceOpen);
    const removeEventListeners = () => {
      mediaSource.removeEventListener("sourceclose", onMediaSourceClose);
      mediaSource.removeEventListener("sourceended", onMediaSourceEnded);
      mediaSource.removeEventListener("sourceopen", onMediaSourceOpen);
    };

    const objectURL = URL.createObjectURL(mediaSource);
    playerObj.videoElement.src = objectURL;
    playerObj.mediaSourceObj = {
      mediaSource,
      objectURL,
      removeEventListeners,
      sourceBuffers: [],
      nextSourceBufferId: 0,
    };
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
    return AttachMediaSourceResult.success();
  } catch (e) {
    return AttachMediaSourceResult.error(
      AttachMediaSourceErrorCode.PlayerInstanceNotFound,
      getErrorMessage(e));
  }
}

/**
 * @param {number} playerId
 * @returns {number}
 */
export function removeMediaSource(playerId: PlayerId): RemoveMediaSourceResult {
  try {
    const playerObj = playersStore.get(playerId);
    if (playerObj === undefined) {
      return RemoveMediaSourceResult
        .error(RemoveMediaSourceErrorCode.PlayerInstanceNotFound);
    }
    if (playerObj.mediaSourceObj === null) {
      return RemoveMediaSourceResult
        .error(RemoveMediaSourceErrorCode.NoMediaSourceAttached);
    }

    const {
      mediaSource,
      objectURL,
      removeEventListeners,
    } = playerObj.mediaSourceObj;
    removeEventListeners();

    if (mediaSource !== null && mediaSource.readyState !== "closed") {
      const { readyState, sourceBuffers } = mediaSource;
      for (let i = sourceBuffers.length - 1; i >= 0; i--) {
        const sourceBuffer = sourceBuffers[i];
        try {
          if (readyState === "open") {
            sourceBuffer.abort();
          }
          mediaSource.removeSourceBuffer(sourceBuffer);
        }
        catch (e) {
          // TODO proper WASM communication?
          const msg = formatErrMessage(e, "Unknown error while removing SourceBuffer");
          Dispatcher.log(LogLevel.Error, "Could not remove SourceBuffer: " + msg);
        }
      }
    }

    clearElementSrc(playerObj.videoElement);
    if (objectURL !== null) {
      try {
        URL.revokeObjectURL(objectURL);
      } catch (e) {
          // TODO proper WASM communication?
        const msg = formatErrMessage(e, "Unknown error while revoking ObjectURL");
        Dispatcher.log(LogLevel.Error, "Could not revoke ObjectURL: " + msg);
      }
    }
    return RemoveMediaSourceResult.success();
  } catch (e) {
    return RemoveMediaSourceResult
      .error(RemoveMediaSourceErrorCode.UnknownError, getErrorMessage(e));
  }
}

export function setMediaSourceDuration(
  playerId: number,
  duration: number
) : MediaSourceDurationUpdateResult {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return MediaSourceDurationUpdateResult
      .error(MediaSourceDurationUpdateErrorCode.PlayerInstanceNotFound);
  }
  if (playerObj.mediaSourceObj === null) {
    return MediaSourceDurationUpdateResult
      .error(MediaSourceDurationUpdateErrorCode.NoMediaSourceAttached);
  }

  try {
    const { mediaSource } = playerObj.mediaSourceObj;
    mediaSource.duration = duration;
    return MediaSourceDurationUpdateResult.success();
  } catch (err) {
    return MediaSourceDurationUpdateResult.error(
      MediaSourceDurationUpdateErrorCode.UnknownError,
      getErrorMessage(err)
    );
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
): AddSourceBufferResult {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return AddSourceBufferResult.error(AddSourceBufferErrorCode.PlayerInstanceNotFound);
  }
  if (playerObj.mediaSourceObj === null) {
    return AddSourceBufferResult.error(AddSourceBufferErrorCode.NoMediaSourceAttached);
  }

  const {
    mediaSource,
    sourceBuffers,
    nextSourceBufferId,
  } = playerObj.mediaSourceObj;
  if (mediaSource.readyState === "closed") {
    return AddSourceBufferResult.error(AddSourceBufferErrorCode.MediaSourceIsClosed);
  }
  if (typ === "") {
    return AddSourceBufferResult.error(AddSourceBufferErrorCode.EmptyMimeType);
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
    return AddSourceBufferResult.success(sourceBufferId);
  } catch (err) {
    if (!(err instanceof Error)) {
      return AddSourceBufferResult.error(AddSourceBufferErrorCode.UnknownError);
    } else if (err.name === "QuotaExceededError") {
      return AddSourceBufferResult.error(AddSourceBufferErrorCode.QuotaExceededError,
                                         err.message);
    } else if (err.name === "NotSupportedError") {
      return AddSourceBufferResult.error(AddSourceBufferErrorCode.TypeNotSupportedError,
                                         err.message);
    } else {
      return AddSourceBufferResult.error(AddSourceBufferErrorCode.UnknownError,
                                         err.message);
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
): AppendBufferResult {
  try {
    const sourceBufferObj = getSourceBufferObj(playerId, sourceBufferId);
    if (sourceBufferObj === undefined) {
      return AppendBufferResult
        .error(AppendBufferErrorCode.PlayerOrSourceBufferInstanceNotFound);
    }
    let pushedData = data;
    if (sourceBufferObj.transmuxer !== null) {
      const transmuxedData = sourceBufferObj.transmuxer(data);

      // TODO specific error for transmuxing error
      if (transmuxedData !== null) {
        pushedData = transmuxedData;
      }
    }
    sourceBufferObj.sourceBuffer.appendBuffer(pushedData);
    return AppendBufferResult.success();
  } catch (err) {
    return AppendBufferResult
      .error(AppendBufferErrorCode.UnknownError, getErrorMessage(err));
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
): AppendBufferResult {
  const segment: Uint8Array | undefined = jsMemoryResources.get(resourceId);
  if (segment === undefined) {
    return AppendBufferResult.error(AppendBufferErrorCode.GivenResourceNotFound);
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
): RemoveBufferResult {
  try {
    const sourceBuffer = getSourceBufferObj(playerId, sourceBufferId);
    if (sourceBuffer === undefined) {
      return RemoveBufferResult
        .error(RemoveBufferErrorCode.PlayerOrSourceBufferInstanceNotFound);
    }
    sourceBuffer.sourceBuffer.remove(start, end);
    return RemoveBufferResult.success();
  } catch (err) {
    return RemoveBufferResult
      .error(RemoveBufferErrorCode.UnknownError, getErrorMessage(err));
  }
}

/**
 * @param {number} playerId
 * @returns {number}
 */
export function endOfStream(
  playerId: PlayerId
): EndOfStreamResult {
  try {
    const mediaSourceObj = getMediaSourceObj(playerId);
    if (mediaSourceObj === undefined) {
      return EndOfStreamResult
        .error(EndOfStreamErrorCode.PlayerInstanceNotFound);
    }
    mediaSourceObj.mediaSource.endOfStream();
    return EndOfStreamResult.success();
  } catch (err) {
    return EndOfStreamResult
      .error(EndOfStreamErrorCode.UnknownError, getErrorMessage(err));
  }
}

const OBSERVATION_EVENTS = [
  ["seeking", PlaybackTickReason.Seeking],
  ["seeked", PlaybackTickReason.Seeked],
  ["loadedmetadata", PlaybackTickReason.LoadedMetadata],
  ["loadeddata", PlaybackTickReason.LoadedData],
  ["canplay", PlaybackTickReason.CanPlay],
  ["canplaythrough", PlaybackTickReason.CanPlayThrough],
  ["ended", PlaybackTickReason.Ended],
  ["pause", PlaybackTickReason.Pause],
  ["play", PlaybackTickReason.Play],
  ["ratechange", PlaybackTickReason.RateChange],
  ["stalled", PlaybackTickReason.Stalled],
  // "durationchange",
] as const;

/**
 * @param {number} playerId
 */
export function startObservingPlayback(playerId: PlayerId): void {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return;
  }
  if (playerObj.observationsObj !== null) {
    return;
  }
  const { videoElement } = playerObj;

  const listenerRemovers = OBSERVATION_EVENTS.map(([evtName, reason]) => {
    videoElement.addEventListener(evtName, onEvent);
    function onEvent() {
      onNextTick(reason);
    }
    return () => videoElement.removeEventListener(evtName, onEvent);
  });

  playerObj.observationsObj = {
    removeEventListeners() {
      listenerRemovers.forEach(removeCb => removeCb());
    },
    timeoutId: undefined,
  };
  /* eslint-disable @typescript-eslint/no-floating-promises */
  Promise.resolve().then(() => onNextTick(PlaybackTickReason.Init));
  function onNextTick(reason: PlaybackTickReason) {
    const innerPlayerObj = playersStore.get(playerId);
    if (innerPlayerObj === undefined || innerPlayerObj.observationsObj === null) {
      return;
    }
    if (innerPlayerObj === undefined) {
      stopObservingPlayback(playerId);
      return;
    }
    if (innerPlayerObj.observationsObj.timeoutId !== undefined) {
      clearTimeout(innerPlayerObj.observationsObj.timeoutId);
      innerPlayerObj.observationsObj.timeoutId = undefined;
    }

    const buffered = new Float64Array(videoElement.buffered.length * 2);
    for (let i = 0; i < videoElement.buffered.length; i++) {
      const offset = i * 2;
      buffered[offset] = videoElement.buffered.start(i);
      buffered[offset + 1] = videoElement.buffered.end(i);
    }

    const observation = new MediaObservation(
      reason,
      videoElement.currentTime,
      videoElement.readyState,
      buffered,
      videoElement.paused,
      videoElement.seeking
    );
    innerPlayerObj.dispatcher.on_playback_tick(observation);

    innerPlayerObj.observationsObj.timeoutId = setTimeout(() => {
      if (innerPlayerObj.observationsObj !== null) {
        innerPlayerObj.observationsObj.timeoutId = undefined;
      }
      onNextTick(PlaybackTickReason.RegularInterval);
    }, 1000);
  }
  /* eslint-enable @typescript-eslint/no-floating-promises */
}

/**
 * @param {number} playerId
 */
export function stopObservingPlayback(playerId: PlayerId) {
  const playerObj = playersStore.get(playerId);
  if (playerObj === undefined) {
    return;
  }
  if (playerObj.observationsObj === null) {
    return;
  }
  playerObj.observationsObj.removeEventListeners();
  if (playerObj.observationsObj.timeoutId !== undefined) {
    clearTimeout(playerObj.observationsObj.timeoutId);
  }
  playerObj.observationsObj = null;
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

/**
 * Clear element's src attribute.
 * @param {HTMLMediaElement} element
 */
function clearElementSrc(element: HTMLMediaElement): void {
  // On some browsers, we first have to make sure the textTracks elements are
  // both disabled and removed from the DOM.
  // If we do not do that, we may be left with displayed text tracks on the
  // screen, even if the track elements are properly removed, due to browser
  // issues.
  // Bug seen on Firefox (I forgot which version) and Chrome 96.
  const { textTracks } = element;
  if (textTracks != null) {
    for (let i = 0; i < textTracks.length; i++) {
      textTracks[i].mode = "disabled";
    }
    if (element.hasChildNodes()) {
      const { childNodes } = element;
      for (let j = childNodes.length - 1; j >= 0; j--) {
        if (childNodes[j].nodeName === "track") {
          try {
            element.removeChild(childNodes[j]);
          } catch (err) {
            // TODO
          }
        }
      }
    }
  }
  element.src = "";

  // On IE11, element.src = "" is not sufficient as it
  // does not clear properly the current MediaKey Session.
  // Microsoft recommended to use element.removeAttr("src").
  element.removeAttribute("src");
}

function getMediaSourceObj(
  playerId: PlayerId
) : MediaSourceInstanceInfo | undefined {
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

function getSourceBufferObj(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId
): SourceBufferInstanceInfo | undefined {
  const mediaSourceObj = getMediaSourceObj(playerId);
  if (mediaSourceObj === undefined) {
    return undefined;
  }
  const { sourceBuffers } = mediaSourceObj;
  for (const sourceBufferObj of sourceBuffers) {
    if (sourceBufferObj.id === sourceBufferId) {
      return sourceBufferObj;
    }
  }
  return undefined;
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
const win = window as any;
win.jsLog = log;
win.jsFetchU8 = fetchU8;
win.jsFetchU8NoCopy = fetchU8NoCopy;
win.jsAbortRequest = abortRequest;
win.jsAttachMediaSource = attachMediaSource;
win.jsRemoveMediaSource = removeMediaSource;
win.jsSetMediaSourceDuration = setMediaSourceDuration;
win.jsAddSourceBuffer = addSourceBuffer;
win.jsAppendBuffer = appendBuffer;
win.jsAppendBufferJsBlob = appendBufferJsBlob;
win.jsRemoveBuffer = removeBuffer;
win.jsEndOfStream = endOfStream;
win.jsStartObservingPlayback = startObservingPlayback;
win.jsStopObservingPlayback = stopObservingPlayback;
win.jsFreeResource = freeResource;
