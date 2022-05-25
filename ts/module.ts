import init, {
  WaspHlsPlayer,
  LogLevel,
  MediaSourceReadyState,
  PlaybackTickReason,
} from "../wasm/wasp_hls.js";

let requestId = 0;

type PlayerId = number;
type RequestId = number;
type SourceBufferId = number;

const currentPlayers : Partial<Record<PlayerId, PlayerInstanceInfo>> = {};
const currentRequests : Partial<Record<RequestId, RequestObject>> = {};
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

export function abortRequest(id: RequestId) : void {
  const requestObj = currentRequests[id];
  if (requestObj !== undefined) {
    requestObj.abortController.abort();
  }
}

export function fetchU8(playerId: PlayerId, url: string): RequestId {
  const currentRequestId = requestId;
  if (requestId < Number.MAX_SAFE_INTEGER) {
    requestId++;
  } else {
    requestId = 0;
  }
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

export function attachMediaSource(playerId: PlayerId): void {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    throw new Error("Unknown player asking to create a MediaSource");
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
}

function addSourceBuffer(playerId: PlayerId, typ: string): number {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    throw new Error("Unknown player asking to create a MediaSource");
  }
  if (playerObj.mediaSourceObj === null) {
    return -1;
  }

  const {
    mediaSource,
    sourceBuffers,
    nextSourceBufferId,
  } = playerObj.mediaSourceObj;
  if (mediaSource.readyState === "closed") {
    return -2;
  }
  if (typ === "") {
    return -5;
  }
  try {
    const sourceBuffer = mediaSource.addSourceBuffer(typ);
    const sourceBufferId = nextSourceBufferId;
    sourceBuffers.push({ id: sourceBufferId, sourceBuffer });
    playerObj.mediaSourceObj.nextSourceBufferId++;
    sourceBuffer.addEventListener("updateend", function() {
      playerObj.player.on_source_buffer_update(sourceBufferId);
    });
    return sourceBufferId;
  } catch (err) {
    if (!(err instanceof Error)) {
      return -6;
    }
    if (err.name === "QuotaExceededError") {
      return -3;
    }
    if (err.name === "NotSupportedError") {
      return -4;
    }
    return -6;
  }
}

export function removeMediaSource(playerId: PlayerId): void {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    throw new Error("Unknown player asking to remove a MediaSource");
  }
  if (playerObj.mediaSourceObj === null) {
    return;
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
        const errMsg = e instanceof Error ?
          e.name + ": " + e.message :
          "Unknown error while removing SourceBuffer";
        WaspHlsPlayer.log(LogLevel.Error, "Could not remove SourceBuffer: " + errMsg);
        // TODO also real error? How could it work here? Round tripw with WASM?
      }
    }
  }

  clearElementSrc(playerObj.videoElement);
  if (objectURL !== null) {
    try {
      URL.revokeObjectURL(objectURL);
    } catch (e) {
      // TODO log?
    }
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

/**
 * @param {number} logLevel
 * @param {string} logStr
 */
export function log(logLevel: LogLevel, logStr: string): void {
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
    default:
      throw new Error("Unknown log level");
  }
}

async function run() {
  await init();
  const videoElement = document.createElement("video");
  videoElement.autoplay = true;
  videoElement.controls = true;
  document.body.appendChild(videoElement);
  const playerObj = createPlayer(videoElement);
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
    throw new Error("Unknown player id.");
  }
  const { mediaSourceObj } = playerObj;
  if (mediaSourceObj === null) {
    throw new Error("The player has no attached MediaSource");
  }
  const { sourceBuffers } = mediaSourceObj;
  for (const sourceBufferObj of sourceBuffers) {
    if (sourceBufferObj.id === sourceBufferId) {
      return sourceBufferObj.sourceBuffer;
    }
  }
  return undefined;
}

function appendBuffer(
  playerId: PlayerId,
  sourceBufferId: SourceBufferId,
  data: ArrayBuffer
): void {
  const sourceBuffer = getSourceBuffer(playerId, sourceBufferId);
  if (sourceBuffer === undefined) {
    throw new Error("No SourceBuffer found with the given SourceBufferId.");
  }
  sourceBuffer.appendBuffer(data);
}

function startObservingPlayback(playerId: PlayerId): void {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    throw new Error("Unknown player id.");
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

function stopObservingPlayback(playerId: PlayerId) {
  const playerObj = getPlayerObject(playerId);
  if (playerObj === undefined) {
    throw new Error("Unknown player id.");
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

/* eslint-disable */
const win = window as any;
win.jsLog = log;
// win.jsFetchStr = fetchStr;
win.jsFetchU8 = fetchU8;
win.jsAbortRequest = abortRequest;
win.jsAttachMediaSource = attachMediaSource;
win.jsRemoveMediaSource = removeMediaSource;
win.jsAddSourceBuffer = addSourceBuffer;
win.jsAppendBuffer = appendBuffer;
win.jsStartObservingPlayback = startObservingPlayback;
win.jsStopObservingPlayback = stopObservingPlayback;
