import init, {
  WaspHlsPlayer,
  LogLevel,
  MediaSourceReadyState,
  PlaybackTickReason,
  RemoveMediaSourceError,
  AttachMediaSourceError,
  AddSourceBufferResult,
  AddSourceBufferError,
  AppendBufferError,
  RemoveBufferError,
} from "../wasm/wasp_hls.js";

// TODO also use enum for error code?
let lastError : null | string = null;

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
  nextRequestId = loopingIncrement(nextRequestId);
  const abortController = new AbortController();
  currentRequests[currentRequestId] = { abortController };
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      const arrRes = await res.arrayBuffer();
      delete currentRequests[currentRequestId];
      const playerObj = getPlayerObject(playerId);
      if (playerObj !== undefined) {
        playerObj.player
          .on_u8_request_finished(currentRequestId, new Uint8Array(arrRes));
        console.timeEnd("WAY 1");
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
 * TODO failure cases
 * @param {number} playerId
 * @param {string} url
 * @returns {number}
 */
function fetchU8NoCopy(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = nextRequestId;
  nextRequestId = loopingIncrement(nextRequestId);
  const abortController = new AbortController();
  currentRequests[currentRequestId] = { abortController };
  fetch(url, { signal: abortController.signal })
    .then(async res => {
      const arrRes = await res.arrayBuffer();
      delete currentRequests[currentRequestId];
      const playerObj = getPlayerObject(playerId);
      if (playerObj !== undefined) {
        const segmentArray = new Uint8Array(arrRes);
        jsMemoryResources[nextResourceId] = segmentArray;
        playerObj.player
          .on_u8_no_copy_request_finished(currentRequestId, nextResourceId);
        nextResourceId = loopingIncrement(nextResourceId);
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
    return true;
  }
  return false;
}

/**
 * @param {number} playerId
 * @returns {number}
 */
function attachMediaSource(playerId: PlayerId): AttachMediaSourceError {
  try {
    lastError = null;
    const playerObj = getPlayerObject(playerId);
    if (playerObj === undefined) {
      setLastError("Unknown player asking to attach a MediaSource");
      return AttachMediaSourceError.PlayerInstanceNotFound;
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
    return AttachMediaSourceError.None;
  } catch (e) {
    setLastError("Unknown error while trying to attach a MediaSource", e);
    return AttachMediaSourceError.PlayerInstanceNotFound;
  }
}

/**
 * @param {number} playerId
 * @returns {number}
 */
function removeMediaSource(playerId: PlayerId): RemoveMediaSourceError {
  try {
    lastError = null;
    const playerObj = getPlayerObject(playerId);
    if (playerObj === undefined) {
      setLastError("Unknown player asking to remove a MediaSource");
      return RemoveMediaSourceError.PlayerInstanceNotFound;
    }
    if (playerObj.mediaSourceObj === null) {
      setLastError("The player has no MediaSource attached");
      return RemoveMediaSourceError.NoMediaSourceAttached;
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
    return RemoveMediaSourceError.None;
  } catch (e) {
    setLastError("Unknown error while revoking ObjectURL", e);
    return RemoveMediaSourceError.UnknownError;
  }
}

/**
 * @param {number} playerId
 * @param {string} typ
 * @returns {Object}
 */
function addSourceBuffer(playerId: PlayerId, typ: string): AddSourceBufferResult {
  lastError = null;
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    return AddSourceBufferResult.error(AddSourceBufferError.PlayerInstanceNotFound);
  }
  if (playerObj.mediaSourceObj === null) {
    return AddSourceBufferResult.error(AddSourceBufferError.NoMediaSourceAttached);
  }

  const {
    mediaSource,
    sourceBuffers,
    nextSourceBufferId,
  } = playerObj.mediaSourceObj;
  if (mediaSource.readyState === "closed") {
    return AddSourceBufferResult.error(AddSourceBufferError.MediaSourceIsClosed);
  }
  if (typ === "") {
    return AddSourceBufferResult.error(AddSourceBufferError.EmptyMimeType);
  }
  try {
    const sourceBuffer = mediaSource.addSourceBuffer(typ);
    const sourceBufferId = nextSourceBufferId;
    sourceBuffers.push({ id: sourceBufferId, sourceBuffer });
    playerObj.mediaSourceObj.nextSourceBufferId++;
    sourceBuffer.addEventListener("updateend", function() {
      playerObj.player.on_source_buffer_update(sourceBufferId);
    });
    return AddSourceBufferResult.success(sourceBufferId);
  } catch (err) {
    setLastError("Unknown error when adding SourceBuffer", err);
    if (!(err instanceof Error)) {
      return AddSourceBufferResult.error(AddSourceBufferError.UnknownError);
    }
    if (err.name === "QuotaExceededError") {
      setLastError(err.message, err);
      return AddSourceBufferResult.error(AddSourceBufferError.QuotaExceededError);
    }
    if (err.name === "NotSupportedError") {
      setLastError(err.message, err);
      return AddSourceBufferResult.error(AddSourceBufferError.TypeNotSupportedError);
    }
  }
  return AddSourceBufferResult.error(AddSourceBufferError.UnknownError);
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
  data: ArrayBuffer
): AppendBufferError {
  try {
    const sourceBuffer = getSourceBuffer(playerId, sourceBufferId);
    if (sourceBuffer === undefined) {
      setLastError("The SourceBuffer associated to the given `SourceBufferId` " +
        "was not found");
      return AppendBufferError.PlayerOrSourceBufferInstanceNotFound;
    }
    sourceBuffer.appendBuffer(data);
    return AppendBufferError.None;
  } catch (err) {
    setLastError("UnknownError when calling \"appendBuffer\"", err);
    return AppendBufferError.UnknownError;
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
): AppendBufferError {
  try {
    const sourceBuffer = getSourceBuffer(playerId, sourceBufferId);
    if (sourceBuffer === undefined) {
      setLastError("The SourceBuffer associated to the given `SourceBufferId` " +
        "was not found");
      return AppendBufferError.PlayerOrSourceBufferInstanceNotFound;
    }
    const segment: Uint8Array | undefined = jsMemoryResources[resourceId];
    if (segment === undefined) {
      return AppendBufferError.GivenResourceNotFound;
    }
    sourceBuffer.appendBuffer(segment);
    return AppendBufferError.None;
  } catch (err) {
    setLastError("UnknownError when calling \"appendBuffer\"", err);
    return AppendBufferError.UnknownError;
  }
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
): RemoveBufferError {
  try {
    const sourceBuffer = getSourceBuffer(playerId, sourceBufferId);
    if (sourceBuffer === undefined) {
      setLastError("The SourceBuffer associated to the given `SourceBufferId` " +
        "was not found");
      return RemoveBufferError.PlayerOrSourceBufferInstanceNotFound;
    }
    sourceBuffer.remove(start, end);
    return RemoveBufferError.None;
  } catch (err) {
    setLastError("UnknownError when calling \"appendBuffer\"", err);
    return RemoveBufferError.UnknownError;
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

/**
 * @returns {string}
 */
function getLastError() : string {
  return lastError ?? "";
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

function setLastError(defaultMsg: string, err?: unknown) {
  if (err === undefined) {
    lastError = defaultMsg;
  } else {
    lastError = formatErrMessage(err, defaultMsg);
  }
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
  await init();
  const videoElement = document.createElement("video");
  videoElement.autoplay = true;
  videoElement.controls = true;
  document.body.appendChild(videoElement);
  const playerObj = createPlayer(videoElement);
  // playerObj.player.test_seg_back_and_forth();
  playerObj.player.load_content(
    "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8");
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

function getSourceBuffer(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId
): SourceBuffer | undefined {
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
      return sourceBufferObj.sourceBuffer;
    }
  }
  return undefined;
}

const maxU32 = Math.pow(2, 32) - 1;

function loopingIncrement(num : number) : number {
  return num >= maxU32 ? 0 : num + 1;
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
win.jsAddSourceBuffer = addSourceBuffer;
win.jsAppendBuffer = appendBuffer;
win.jsAppendBufferJsBlob = appendBufferJsBlob;
win.jsRemoveBuffer = removeBuffer;
win.jsStartObservingPlayback = startObservingPlayback;
win.jsStopObservingPlayback = stopObservingPlayback;
win.jsFreeResource = freeResource;
win.jsGetLastError = getLastError;
