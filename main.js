(() => {
  // src/ts-common/idGenerator.ts
  function idGenerator() {
    let prefix = "";
    let currId = -1;
    return function generateNewId() {
      currId++;
      if (currId >= Number.MAX_SAFE_INTEGER) {
        prefix += "0";
        currId = 0;
      }
      return prefix + String(currId);
    };
  }

  // src/ts-common/QueuedSourceBuffer.ts
  var QueuedSourceBuffer = class {
    constructor(sourceBuffer) {
      this._sourceBuffer = sourceBuffer;
      this._queue = [];
      this._pendingTask = null;
      const intervalId = setInterval(() => {
        this._flush();
      }, 2e3);
      const onError = this._onPendingTaskError.bind(this);
      const _onUpdateEnd = () => {
        this._flush();
      };
      sourceBuffer.addEventListener("error", onError);
      sourceBuffer.addEventListener("updateend", _onUpdateEnd);
      this._dispose = [() => {
        clearInterval(intervalId);
        sourceBuffer.removeEventListener("error", onError);
        sourceBuffer.removeEventListener("updateend", _onUpdateEnd);
      }];
    }
    push(data) {
      console.debug("QSB: receiving order to push data to the SourceBuffer");
      return this._addToQueue({ type: 0 /* Push */, value: data });
    }
    removeBuffer(start, end) {
      console.debug("QSB: receiving order to remove data from the SourceBuffer", start, end);
      return this._addToQueue({
        type: 1 /* Remove */,
        value: { start, end }
      });
    }
    getBufferedRanges() {
      return this._sourceBuffer.buffered;
    }
    dispose() {
      this._dispose.forEach((disposeFn) => disposeFn());
      if (this._pendingTask !== null) {
        this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled"));
        this._pendingTask = null;
      }
      while (this._queue.length > 0) {
        const nextElement = this._queue.shift();
        if (nextElement !== void 0) {
          nextElement.reject(new Error("QueuedSourceBuffer Cancelled"));
        }
      }
    }
    _onPendingTaskError(err) {
      const error = err instanceof Error ? err : new Error("An unknown error occured when doing operations on the SourceBuffer");
      if (this._pendingTask != null) {
        this._pendingTask.reject(error);
      }
    }
    _addToQueue(operation) {
      return new Promise((resolve, reject) => {
        const shouldRestartQueue = this._queue.length === 0 && this._pendingTask === null;
        const queueItem = { resolve, reject, ...operation };
        this._queue.push(queueItem);
        if (shouldRestartQueue) {
          this._flush();
        }
      });
    }
    _flush() {
      if (this._sourceBuffer.updating) {
        return;
      }
      if (this._pendingTask !== null) {
        const task = this._pendingTask;
        const { resolve } = task;
        this._pendingTask = null;
        resolve();
        return this._flush();
      } else {
        const nextItem = this._queue.shift();
        if (nextItem === void 0) {
          return;
        } else {
          this._pendingTask = nextItem;
        }
      }
      try {
        switch (this._pendingTask.type) {
          case 0 /* Push */:
            const segmentData = this._pendingTask.value;
            if (segmentData === void 0) {
              this._flush();
              return;
            }
            console.debug("QSB: pushing data");
            this._sourceBuffer.appendBuffer(segmentData);
            break;
          case 1 /* Remove */:
            const { start, end } = this._pendingTask.value;
            console.debug("QSB: removing data from SourceBuffer", start, end);
            this._sourceBuffer.remove(start, end);
            break;
          default:
            assertUnreachable(this._pendingTask);
        }
      } catch (e) {
        this._onPendingTaskError(e);
      }
    }
  };
  function assertUnreachable(_) {
    throw new Error("Unreachable path taken");
  }

  // src/wasm/wasp_hls.js
  var cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
  cachedTextDecoder.decode();
  var cachedUint8Memory0 = new Uint8Array();
  var cachedTextEncoder = new TextEncoder("utf-8");
  var encodeString = typeof cachedTextEncoder.encodeInto === "function" ? function(arg, view) {
    return cachedTextEncoder.encodeInto(arg, view);
  } : function(arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
      read: arg.length,
      written: buf.length
    };
  };
  var cachedInt32Memory0 = new Int32Array();
  var cachedFloat64Memory0 = new Float64Array();
  var MediaSourceReadyState = Object.freeze({
    Closed: 0,
    "0": "Closed",
    Ended: 1,
    "1": "Ended",
    Open: 2,
    "2": "Open"
  });
  var PlaybackTickReason = Object.freeze({ Init: 0, "0": "Init", Seeking: 1, "1": "Seeking", Seeked: 2, "2": "Seeked", RegularInterval: 3, "3": "RegularInterval", LoadedData: 4, "4": "LoadedData", LoadedMetadata: 5, "5": "LoadedMetadata", CanPlay: 6, "6": "CanPlay", CanPlayThrough: 7, "7": "CanPlayThrough", Ended: 8, "8": "Ended", Pause: 9, "9": "Pause", Play: 10, "10": "Play", RateChange: 11, "11": "RateChange", Stalled: 12, "12": "Stalled" });
  var RemoveMediaSourceErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var MediaSourceDurationUpdateErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var AttachMediaSourceErrorCode = Object.freeze({
    UnknownError: 0,
    "0": "UnknownError"
  });
  var RemoveBufferErrorCode = Object.freeze({
    SourceBufferNotFound: 0,
    "0": "SourceBufferNotFound",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var EndOfStreamErrorCode = Object.freeze({
    UnknownError: 0,
    "0": "UnknownError"
  });
  var AddSourceBufferErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    MediaSourceIsClosed: 1,
    "1": "MediaSourceIsClosed",
    QuotaExceededError: 2,
    "2": "QuotaExceededError",
    TypeNotSupportedError: 3,
    "3": "TypeNotSupportedError",
    EmptyMimeType: 4,
    "4": "EmptyMimeType",
    UnknownError: 5,
    "5": "UnknownError"
  });
  var AppendBufferErrorCode = Object.freeze({
    NoResource: 0,
    "0": "NoResource",
    NoSourceBuffer: 1,
    "1": "NoSourceBuffer",
    TransmuxerError: 2,
    "2": "TransmuxerError",
    UnknownError: 3,
    "3": "UnknownError"
  });
  var PlaybackObservationReason = Object.freeze({
    Init: 0,
    "0": "Init",
    Seeked: 1,
    "1": "Seeked",
    Seeking: 2,
    "2": "Seeking",
    Ended: 3,
    "3": "Ended",
    ReadyStateChanged: 4,
    "4": "ReadyStateChanged",
    RegularInterval: 5,
    "5": "RegularInterval",
    Error: 6,
    "6": "Error"
  });
  var TimerReason = Object.freeze({
    MediaPlaylistRefresh: 0,
    "0": "MediaPlaylistRefresh"
  });
  var LogLevel = Object.freeze({
    Error: 0,
    "0": "Error",
    Warn: 1,
    "1": "Warn",
    Info: 2,
    "2": "Info",
    Debug: 3,
    "3": "Debug"
  });
  var MediaType = Object.freeze({ Audio: 0, "0": "Audio", Video: 1, "1": "Video" });

  // src/ts-main/errors.ts
  var InitializationError = class extends Error {
    constructor(code, wasmHttpStatus, message) {
      super();
      Object.setPrototypeOf(this, InitializationError.prototype);
      this.name = "InitializationError";
      this.code = code;
      this.wasmHttpStatus = wasmHttpStatus;
      this.message = message;
    }
  };

  // src/ts-main/observePlayback.ts
  var OBSERVATION_EVENTS = [
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
    ["stalled", PlaybackTickReason.Stalled]
  ];
  function observePlayback(videoElement, mediaSourceId, onNewObservation) {
    let isStopped = false;
    let timeoutId;
    const listenerRemovers = OBSERVATION_EVENTS.map(([evtName, reason]) => {
      videoElement.addEventListener(evtName, onEvent);
      function onEvent() {
        onNextTick(reason);
      }
      return () => videoElement.removeEventListener(evtName, onEvent);
    });
    Promise.resolve().then(() => onNextTick(PlaybackTickReason.Init));
    return () => {
      if (isStopped) {
        return;
      }
      isStopped = true;
      listenerRemovers.forEach((removeCb) => removeCb());
      listenerRemovers.length = 0;
      if (timeoutId !== void 0) {
        clearTimeout(timeoutId);
        timeoutId = void 0;
      }
    };
    function onNextTick(reason) {
      if (isStopped) {
        return;
      }
      if (timeoutId !== void 0) {
        clearTimeout(timeoutId);
        timeoutId = void 0;
      }
      const buffered = new Float64Array(videoElement.buffered.length * 2);
      for (let i = 0; i < videoElement.buffered.length; i++) {
        const offset = i * 2;
        buffered[offset] = videoElement.buffered.start(i);
        buffered[offset + 1] = videoElement.buffered.end(i);
      }
      const { currentTime, readyState, paused, seeking } = videoElement;
      onNewObservation({
        mediaSourceId,
        reason,
        currentTime,
        readyState,
        buffered,
        paused,
        seeking
      });
      timeoutId = setTimeout(() => {
        if (isStopped) {
          timeoutId = void 0;
          return;
        }
        onNextTick(PlaybackTickReason.RegularInterval);
      }, 1e3);
    }
  }

  // src/ts-main/postMessageToWorker.ts
  function postMessageToWorker(worker, msg, transferables) {
    console.debug("--> sending to worker:", msg.type);
    if (transferables === void 0) {
      worker.postMessage(msg);
    } else {
      worker.postMessage(msg, transferables);
    }
  }

  // src/ts-main/api.ts
  var generateContentId = idGenerator();
  var WaspHlsPlayer = class {
    constructor(videoElement) {
      this.videoElement = videoElement;
      this.initializationStatus = InitializationStatus.Uninitialized;
      this._worker = null;
      this._currentContentMetadata = null;
    }
    initialize(opts) {
      try {
        this.initializationStatus = InitializationStatus.Initializing;
        const { wasmUrl, workerUrl } = opts;
        let resolveProm = noop;
        let rejectProm = noop;
        const ret = new Promise((resolve, reject) => {
          resolveProm = resolve;
          rejectProm = reject;
        });
        this._startWorker(workerUrl, wasmUrl, resolveProm, rejectProm);
        return ret;
      } catch (err) {
        this.initializationStatus = InitializationStatus.Errored;
        return Promise.reject(err);
      }
    }
    loadContent(url) {
      if (this._worker === null) {
        throw new Error("The Player is not initialized or disposed.");
      }
      const contentId = generateContentId();
      this._currentContentMetadata = {
        contentId,
        mediaSourceId: null,
        mediaSource: null,
        disposeMediaSource: null,
        sourceBuffers: [],
        stopPlaybackObservations: null
      };
      postMessageToWorker(this._worker, {
        type: "load",
        value: { contentId, url }
      });
    }
    seek(position) {
      this.videoElement.currentTime = position;
    }
    stop() {
      if (this._worker === null) {
        throw new Error("The Player is not initialized or disposed.");
      }
      if (this._currentContentMetadata !== null && this._currentContentMetadata.stopPlaybackObservations !== null) {
        this._currentContentMetadata.stopPlaybackObservations();
        this._currentContentMetadata.stopPlaybackObservations = null;
      }
      if (this._currentContentMetadata !== null) {
        postMessageToWorker(this._worker, {
          type: "stop",
          value: { contentId: this._currentContentMetadata.contentId }
        });
      }
    }
    dispose() {
      if (this._worker === null) {
        return;
      }
      if (this._currentContentMetadata !== null && this._currentContentMetadata.stopPlaybackObservations !== null) {
        this._currentContentMetadata.stopPlaybackObservations();
        this._currentContentMetadata.stopPlaybackObservations = null;
      }
      postMessageToWorker(this._worker, { type: "dispose", value: null });
      this._worker = null;
    }
    _startWorker(workerUrl, wasmUrl, resolveProm, rejectProm) {
      let mayStillReject = true;
      const worker = new Worker(workerUrl);
      this._worker = worker;
      postMessageToWorker(worker, {
        type: "init",
        value: {
          hasWorkerMse: typeof MediaSource === "function" && MediaSource.canConstructInDedicatedWorker === true,
          wasmUrl
        }
      });
      worker.onmessage = (evt) => {
        const { data } = evt;
        if (typeof data !== "object" || data === null || typeof data.type !== "string") {
          console.error("unexpected Worker message");
          return;
        }
        switch (data.type) {
          case "initialized":
            this.initializationStatus = InitializationStatus.Initialized;
            mayStillReject = false;
            resolveProm();
            break;
          case "initialization-error":
            if (mayStillReject) {
              const error = new InitializationError(data.value.code, data.value.wasmHttpStatus, data.value.message ?? "Error while initializing the WaspHlsPlayer");
              mayStillReject = false;
              rejectProm(error);
            }
            break;
          case "seek":
            if (this._currentContentMetadata === null || this._currentContentMetadata.mediaSourceId !== data.value.mediaSourceId) {
              console.info("API: Ignoring seek due to wrong `mediaSourceId`");
              return;
            }
            try {
              this.videoElement.currentTime = data.value.position;
            } catch (err) {
              console.error("Unexpected error while seeking:", err);
            }
            break;
          case "attach-media-source":
            if (this._currentContentMetadata === null || this._currentContentMetadata.contentId !== data.value.contentId) {
              console.info("API: Ignoring MediaSource attachment due to wrong `contentId`");
              return;
            }
            if (this._currentContentMetadata.stopPlaybackObservations !== null) {
              this._currentContentMetadata.stopPlaybackObservations();
              this._currentContentMetadata.stopPlaybackObservations = null;
            }
            if (data.value.handle !== void 0) {
              this.videoElement.srcObject = data.value.handle;
            } else if (data.value.src !== void 0) {
              this.videoElement.src = data.value.src;
            } else {
              throw new Error('Unexpected "attach-media-source" message: missing source');
            }
            this._currentContentMetadata.mediaSourceId = data.value.mediaSourceId;
            this._currentContentMetadata.mediaSource = null;
            this._currentContentMetadata.disposeMediaSource = () => {
              if (data.value.src !== void 0) {
                URL.revokeObjectURL(data.value.src);
              }
            };
            this._currentContentMetadata.sourceBuffers = [];
            this._currentContentMetadata.stopPlaybackObservations = null;
            break;
          case "create-media-source": {
            if (this._currentContentMetadata === null || this._currentContentMetadata.contentId !== data.value.contentId) {
              console.info("API: Ignoring MediaSource attachment due to wrong `contentId`");
              return;
            }
            const { mediaSourceId: mediaSourceId2 } = data.value;
            let mediaSource;
            try {
              mediaSource = new MediaSource();
            } catch (err) {
              const { name, message } = getErrorInformation(err, "Unknown error when creating the MediaSource");
              postMessageToWorker(worker, {
                type: "create-media-source-error",
                value: { mediaSourceId: mediaSourceId2, message, name }
              });
              return;
            }
            const disposeMediaSource = bindMediaSource(worker, mediaSource, this.videoElement, mediaSourceId2);
            this._currentContentMetadata.mediaSourceId = data.value.mediaSourceId;
            this._currentContentMetadata.mediaSource = mediaSource;
            this._currentContentMetadata.disposeMediaSource = disposeMediaSource;
            this._currentContentMetadata.sourceBuffers = [];
            this._currentContentMetadata.stopPlaybackObservations = null;
            break;
          }
          case "update-media-source-duration": {
            const { mediaSourceId: mediaSourceId2 } = data.value;
            if (this._currentContentMetadata?.mediaSourceId !== mediaSourceId2 || this._currentContentMetadata.mediaSource === null) {
              return;
            }
            try {
              this._currentContentMetadata.mediaSource.duration = data.value.duration;
            } catch (err) {
              const { name, message } = getErrorInformation(err, "Unknown error when updating the MediaSource's duration");
              postMessageToWorker(worker, {
                type: "update-media-source-duration-error",
                value: { mediaSourceId: mediaSourceId2, message, name }
              });
              return;
            }
            break;
          }
          case "clear-media-source": {
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            try {
              this._currentContentMetadata.disposeMediaSource?.();
              clearElementSrc(this.videoElement);
            } catch (err) {
              console.warn("API: Error when clearing current MediaSource:", err);
            }
            break;
          }
          case "create-source-buffer": {
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            if (this._currentContentMetadata.mediaSource === null) {
              postMessageToWorker(worker, {
                type: "create-source-buffer-error",
                value: {
                  mediaSourceId: data.value.mediaSourceId,
                  sourceBufferId: data.value.sourceBufferId,
                  code: 0 /* NoMediaSource */,
                  message: "No MediaSource created on the main thread.",
                  name: void 0
                }
              });
              return;
            }
            try {
              const sourceBuffer = this._currentContentMetadata.mediaSource.addSourceBuffer(data.value.contentType);
              const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
              this._currentContentMetadata.sourceBuffers.push({
                sourceBufferId: data.value.sourceBufferId,
                queuedSourceBuffer
              });
            } catch (err) {
              const { name, message } = getErrorInformation(err, "Unknown error when adding the SourceBuffer to the MediaSource");
              postMessageToWorker(worker, {
                type: "create-source-buffer-error",
                value: {
                  mediaSourceId: data.value.mediaSourceId,
                  sourceBufferId: data.value.sourceBufferId,
                  code: 1 /* AddSourceBufferError */,
                  message,
                  name
                }
              });
            }
            break;
          }
          case "append-buffer": {
            let handleAppendBufferError = function(err) {
              const { name, message } = getErrorInformation(err, "Unknown error when appending data to the SourceBuffer");
              postMessageToWorker(worker, {
                type: "source-buffer-error",
                value: { sourceBufferId, message, name }
              });
            };
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            const sbObject = this._currentContentMetadata.sourceBuffers.find(({ sourceBufferId: sourceBufferId2 }) => sourceBufferId2 === data.value.sourceBufferId);
            if (sbObject === void 0) {
              return;
            }
            const { mediaSourceId: mediaSourceId2, sourceBufferId } = data.value;
            try {
              sbObject.queuedSourceBuffer.push(data.value.data).then(() => {
                postMessageToWorker(worker, {
                  type: "source-buffer-updated",
                  value: { mediaSourceId: mediaSourceId2, sourceBufferId }
                });
              }).catch(handleAppendBufferError);
            } catch (err) {
              handleAppendBufferError(err);
            }
            break;
          }
          case "remove-buffer": {
            let handleRemoveBufferError = function(err) {
              const { name, message } = getErrorInformation(err, "Unknown error when removing data to the SourceBuffer");
              postMessageToWorker(worker, {
                type: "source-buffer-error",
                value: { sourceBufferId, message, name }
              });
            };
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            const sbObject = this._currentContentMetadata.sourceBuffers.find(({ sourceBufferId: sourceBufferId2 }) => sourceBufferId2 === data.value.sourceBufferId);
            if (sbObject === void 0) {
              return;
            }
            const { mediaSourceId: mediaSourceId2, sourceBufferId } = data.value;
            try {
              sbObject.queuedSourceBuffer.removeBuffer(data.value.start, data.value.end).then(() => {
                postMessageToWorker(worker, {
                  type: "source-buffer-updated",
                  value: { mediaSourceId: mediaSourceId2, sourceBufferId }
                });
              }).catch(handleRemoveBufferError);
            } catch (err) {
              handleRemoveBufferError(err);
            }
            break;
          }
          case "start-playback-observation": {
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            if (this._currentContentMetadata.stopPlaybackObservations !== null) {
              this._currentContentMetadata.stopPlaybackObservations();
              this._currentContentMetadata.stopPlaybackObservations = null;
            }
            this._currentContentMetadata.stopPlaybackObservations = observePlayback(this.videoElement, data.value.mediaSourceId, (value) => postMessageToWorker(worker, { type: "observation", value }));
            break;
          }
          case "stop-playback-observation": {
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            if (this._currentContentMetadata.stopPlaybackObservations !== null) {
              this._currentContentMetadata.stopPlaybackObservations();
              this._currentContentMetadata.stopPlaybackObservations = null;
            }
            break;
          }
          case "end-of-stream":
            if (this._currentContentMetadata?.mediaSourceId !== data.value.mediaSourceId) {
              return;
            }
            const { mediaSourceId } = data.value;
            if (this._currentContentMetadata.mediaSource === null) {
              postMessageToWorker(worker, {
                type: "end-of-stream-error",
                value: {
                  mediaSourceId,
                  code: 0 /* NoMediaSource */,
                  message: "No MediaSource created on the main thread.",
                  name: void 0
                }
              });
              return;
            }
            try {
              this._currentContentMetadata.mediaSource.endOfStream();
            } catch (err) {
              const { name, message } = getErrorInformation(err, "Unknown error when calling MediaSource.endOfStream()");
              postMessageToWorker(worker, {
                type: "end-of-stream-error",
                value: {
                  mediaSourceId,
                  code: 1 /* EndOfStreamError */,
                  message,
                  name
                }
              });
            }
            break;
        }
      };
      worker.onerror = (ev) => {
        console.error("API: Worker Error encountered", ev.error);
        if (mayStillReject) {
          rejectProm(ev.error);
        }
      };
    }
  };
  function bindMediaSource(worker, mediaSource, videoElement, mediaSourceId) {
    mediaSource.addEventListener("sourceclose", onMediaSourceClose);
    mediaSource.addEventListener("sourceended", onMediaSourceEnded);
    mediaSource.addEventListener("sourceopen", onMediaSourceOpen);
    const objectURL = URL.createObjectURL(mediaSource);
    videoElement.src = objectURL;
    function onMediaSourceEnded() {
      postMessageToWorker(worker, {
        type: "media-source-state-changed",
        value: { mediaSourceId, state: MediaSourceReadyState.Ended }
      });
    }
    function onMediaSourceOpen() {
      postMessageToWorker(worker, {
        type: "media-source-state-changed",
        value: { mediaSourceId, state: MediaSourceReadyState.Open }
      });
    }
    function onMediaSourceClose() {
      postMessageToWorker(worker, {
        type: "media-source-state-changed",
        value: { mediaSourceId, state: MediaSourceReadyState.Closed }
      });
    }
    return () => {
      mediaSource.removeEventListener("sourceclose", onMediaSourceClose);
      mediaSource.removeEventListener("sourceended", onMediaSourceEnded);
      mediaSource.removeEventListener("sourceopen", onMediaSourceOpen);
      URL.revokeObjectURL(objectURL);
      if (mediaSource.readyState !== "closed") {
        const { readyState, sourceBuffers } = mediaSource;
        for (let i = sourceBuffers.length - 1; i >= 0; i--) {
          const sourceBuffer = sourceBuffers[i];
          if (!sourceBuffer.updating) {
            try {
              if (readyState === "open") {
                sourceBuffer.abort();
              }
              mediaSource.removeSourceBuffer(sourceBuffer);
            } catch (_e) {
            }
          }
        }
      }
      videoElement.src = "";
      videoElement.removeAttribute("src");
    };
  }
  var InitializationStatus = /* @__PURE__ */ ((InitializationStatus2) => {
    InitializationStatus2["Uninitialized"] = "Uninitialized";
    InitializationStatus2["Initializing"] = "Initializing";
    InitializationStatus2["Initialized"] = "Initialized";
    InitializationStatus2["Errored"] = "errorred";
    InitializationStatus2["Disposed"] = "disposed";
    return InitializationStatus2;
  })(InitializationStatus || {});
  function noop() {
  }
  function clearElementSrc(element) {
    const { textTracks } = element;
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
            }
          }
        }
      }
    }
    element.src = "";
    element.removeAttribute("src");
  }
  function getErrorInformation(err, defaultMsg) {
    if (err instanceof Error) {
      return { message: err.message, name: err.name };
    } else {
      return { message: defaultMsg, name: void 0 };
    }
  }

  // src/ts-main/index.ts
  window.WaspHlsPlayer = WaspHlsPlayer;
  var ts_main_default = WaspHlsPlayer;
})();
