import init, {
  WaspHlsPlayer,
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
} from "../wasm/wasp_hls.js";
import {
  getTransmuxedType,
  shouldTransmux,
  transmux,
} from "./transmux.js";

const MAX_U32 = Math.pow(2, 32) - 1;

let nextRequestId = 0;
let nextResourceId = 0;

const jsMemoryResources : Partial<Record<ResourceId, Uint8Array>> = {};
const currentPlayers : Partial<Record<PlayerId, PlayerInstanceInfo>> = {};
const currentRequests : Partial<Record<RequestId, RequestObject>> = {};

/**
 * @param {number} logLevel
 * @param {string} logStr
 */
function log(logLevel: LogLevel, logStr: string) {
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
function fetchU8(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = nextRequestId;
  incrementRequestId();
  const abortController = new AbortController();
  currentRequests[currentRequestId] = { abortController };
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      if (abortController.signal.aborted) {
        return; // Should not be possible. Still, exit if that's the case.
      }
      const arrRes = await res.arrayBuffer();
      delete currentRequests[currentRequestId];
      const playerObj = getPlayerObject(playerId);
      if (playerObj !== undefined) {
        playerObj.player
          .on_u8_request_finished(currentRequestId, new Uint8Array(arrRes), res.url);
        console.timeEnd("WAY 1");
      }
    })
    .catch(err => {
      if (abortController.signal.aborted) {
        return;
      }
      delete currentRequests[currentRequestId];
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
function fetchU8NoCopy(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = nextRequestId;
  incrementRequestId();
  const abortController = new AbortController();
  currentRequests[currentRequestId] = { abortController };
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      const arrRes = await res.arrayBuffer();
      delete currentRequests[currentRequestId];
      const playerObj = getPlayerObject(playerId);
      if (playerObj !== undefined) {
        const currentResourceId = nextResourceId;
        incrementResourceId();
        const segmentArray = new Uint8Array(arrRes);
        jsMemoryResources[currentResourceId] = segmentArray;
        playerObj.player
          .on_u8_no_copy_request_finished(currentRequestId, currentResourceId, res.url);
      }
    })
    .catch(err => {
      delete currentRequests[currentRequestId];
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
function abortRequest(id: RequestId) : boolean {
  const requestObj = currentRequests[id];
  if (requestObj !== undefined) {
    requestObj.abortController.abort();

    // NOTE: we prefer deleting the id on a microtask to avoid possible RequestId
    // conflicts due to other microtask pending while this `abortRequest` call was
    // made (e.g. what if a request failure associated to that request was already
    // scheduled yet another request is made synchronously with the same RequestId?).
    /* eslint-disable-next-line @typescript-eslint/no-floating-promises */
    Promise.resolve().then(() => { delete currentRequests[id]; });
    return true;
  }
  return false;
}

/**
 * @param {number} playerId
 * @returns {number}
 */
function attachMediaSource(playerId: PlayerId): AttachMediaSourceResult {
  try {
    const playerObj = getPlayerObject(playerId);
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
      playerObj?.player
        .on_media_source_state_change(MediaSourceReadyState.Ended);
    }
    function onMediaSourceOpen() {
      playerObj?.player
        .on_media_source_state_change(MediaSourceReadyState.Open);
    }
    function onMediaSourceClose() {
      playerObj?.player
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
function removeMediaSource(playerId: PlayerId): RemoveMediaSourceResult {
  try {
    const playerObj = getPlayerObject(playerId);
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
          WaspHlsPlayer.log(LogLevel.Error, "Could not remove SourceBuffer: " + msg);
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
        WaspHlsPlayer.log(LogLevel.Error, "Could not revoke ObjectURL: " + msg);
      }
    }
    return RemoveMediaSourceResult.success();
  } catch (e) {
    return RemoveMediaSourceResult
      .error(RemoveMediaSourceErrorCode.UnknownError, getErrorMessage(e));
  }
}

function setMediaSourceDuration(
  playerId: number,
  duration: number
) : MediaSourceDurationUpdateResult {
  const playerObj = getPlayerObject(playerId);
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

function getErrorMessage(e: unknown) : string | undefined {
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
function addSourceBuffer(
  playerId: PlayerId,
  mediaType: MediaType,
  typ: string
): AddSourceBufferResult {
  const playerObj = getPlayerObject(playerId);
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
      playerObj.player.on_source_buffer_update(sourceBufferId);
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
function appendBuffer(
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
function appendBufferJsBlob(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId,
  resourceId: ResourceId
): AppendBufferResult {
  const segment: Uint8Array | undefined = jsMemoryResources[resourceId];
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
function removeBuffer(
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
 */
function startObservingPlayback(playerId: PlayerId): void {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    return;
  }
  if (playerObj.observationsObj !== null) {
    return;
  }

  playerObj.videoElement.addEventListener("seeking", onSeeking);
  playerObj.videoElement.addEventListener("seeked", onSeeked);
  const removeEventListeners = () => {
    playerObj.videoElement.removeEventListener("seeking", onSeeking);
    playerObj.videoElement.removeEventListener("seeked", onSeeked);
  };

  function onSeeked() {
    onNextTick(PlaybackTickReason.Seeked);
  }
  function onSeeking() {
    onNextTick(PlaybackTickReason.Seeking);
  }

  playerObj.observationsObj = {
    removeEventListeners,
    timeoutId: undefined,
  };
  /* eslint-disable @typescript-eslint/no-floating-promises */
  Promise.resolve().then(() => onNextTick(PlaybackTickReason.Init));
  function onNextTick(reason: PlaybackTickReason) {
    const innerPlayerObj = getPlayerObject(playerId);
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
    innerPlayerObj.player.on_playback_tick(
      reason,
      innerPlayerObj.videoElement.currentTime);

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
function stopObservingPlayback(playerId: PlayerId) {
  const playerObj = getPlayerObject(playerId);
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

function freeResource(resourceId: number) : boolean {
  if (jsMemoryResources[resourceId] === undefined) {
    return false;
  }
  delete jsMemoryResources[resourceId];
  return true;
}

type PlayerId = number;
type RequestId = number;
type SourceBufferId = number;
type ResourceId = number;
interface RequestObject {
  abortController: AbortController;
}

interface SourceBufferInstanceInfo {
  id: SourceBufferId;
  sourceBuffer: SourceBuffer;
  transmuxer: null | ((input: Uint8Array) => Uint8Array | null);
}

interface MediaSourceInstanceInfo {
  mediaSource: MediaSource;
  objectURL: string;
  removeEventListeners: () => void;
  nextSourceBufferId: number;
  sourceBuffers: SourceBufferInstanceInfo[];
}

interface PlayerInstanceInfo {
  player: WaspHlsPlayer;
  videoElement: HTMLVideoElement;
  mediaSourceObj: MediaSourceInstanceInfo | null;
  observationsObj: {
    removeEventListeners: () => void;
    timeoutId: number | undefined;
  } | null;
}

function formatErrMessage(err: unknown, defaultMsg: string) {
  return err instanceof Error ?
    err.name + ": " + err.message :
    defaultMsg;
}

function createPlayer(videoElement: HTMLVideoElement): PlayerInstanceInfo {
  let playerId = 0;
  while (currentPlayers[playerId] !== undefined) {
    playerId++;
  }
  const player = new WaspHlsPlayer(playerId);
  const playerObj = {
    player,
    videoElement,
    mediaSourceObj: null,
    observationsObj: null,
  };
  currentPlayers[playerId] = playerObj;
  return playerObj;
}

async function run() {
  await init(fetch("./wasp_hls_bg.wasm"));
  const videoElement = document.createElement("video");
  videoElement.autoplay = true;
  videoElement.controls = true;
  document.body.appendChild(videoElement);
  const playerObj = createPlayer(videoElement);
  // playerObj.player.test_seg_back_and_forth();
  playerObj.player.load_content(
    "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8");
  // playerObj.player.load_content(
  //   "https://cdn.jwplayer.com/manifests/pZxWPRg4.m3u8");
  // playerObj.player.load_content(
  //   "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8");
}

setTimeout(() => {
  run().catch(err => {
    console.error("WebAssembly package initialization failed:", err);
  });
}, 100);


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

function getPlayerObject(playerId: PlayerId): PlayerInstanceInfo | undefined {
  return currentPlayers[playerId];
}

function getSourceBufferObj(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId
): SourceBufferInstanceInfo | undefined {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    return undefined;
  }
  const { mediaSourceObj } = playerObj;
  if (mediaSourceObj === null) {
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
    jsMemoryResources[nextResourceId] !== undefined ||
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
    currentRequests[nextRequestId] !== undefined ||
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
win.jsStartObservingPlayback = startObservingPlayback;
win.jsStopObservingPlayback = stopObservingPlayback;
win.jsFreeResource = freeResource;
