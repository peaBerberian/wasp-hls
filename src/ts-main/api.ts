import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import {
  WorkerMessage,
  PlaybackTickReason,
  MediaSourceReadyState,
} from "../ts-common/types";
import postMessageToWorker from "./postMessageToWorker";

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

interface CurrentPlaybackData {
  mediaSourceId: number;
  mediaSource: MediaSource | null;
  dispose: (() => void) | null;
  sourceBuffers: Array<{
    sourceBufferId: number | null;
    queuedSourceBuffer: QueuedSourceBuffer | null;
  }>;
  observationsData: null | {
    removeEventListeners: () => void;
    timeoutId: number | undefined;
    isStopped: boolean;
  };
}

export default class WaspHlsPlayer {
  public initializationStatus: InitializationStatus;
  public videoElement: HTMLVideoElement;
  private _worker : Worker | null;
  private _playbackData : CurrentPlaybackData | null;

  constructor(videoElement: HTMLVideoElement) {
    this.videoElement = videoElement;
    this.initializationStatus = InitializationStatus.Uninitialized;
    this._worker = null;
    this._playbackData = null;
  }

  public initialize(opts: InitializationOptions) : Promise<void> {
    try {
      this.initializationStatus = InitializationStatus.Initializing;
      const { wasmUrl, workerUrl } = opts;
      let resolveProm = noop;
      let rejectProm = noop;
      const ret = new Promise<void>((resolve, reject) => {
        resolveProm = resolve;
        rejectProm = reject;
      });
      this._startWorker(workerUrl, wasmUrl, resolveProm, rejectProm);
      return ret;
    }
    catch (err) {
      this.initializationStatus = InitializationStatus.Errored;
      return Promise.reject(err);
    }
  }

  public loadContent(url: string): void {
    if (this._worker === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    postMessageToWorker(this._worker, {
      type: "load",
      value: { url },
    });
  }

  public seek(position: number): void {
    this.videoElement.currentTime = position;
  }

  public stop(): void {
    if (this._worker === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    this._stopObservingPlayback();
    postMessageToWorker(this._worker, { type: "stop", value: null });
  }

  public dispose() {
    if (this._worker === null) {
      return;
    }
    this._stopObservingPlayback();
    // TODO needed? What about GC once it is set to `null`?
    postMessageToWorker(this._worker, { type: "dispose", value: null });
    this._worker = null;
  }

  private _startWorker(
    workerUrl: string,
    wasmUrl: string,
    resolveProm: () => void,
    rejectProm: (err: unknown) => void
  ) {
    let mayStillReject = true;
    const worker = new Worker(workerUrl);
    this._worker = worker;
    postMessageToWorker(worker, {
      type: "init",
      value: {
        hasWorkerMse: typeof MediaSource === "function" &&
          /* eslint-disable-next-line */
          (MediaSource as any).canConstructInDedicatedWorker === true,
        wasmUrl,
      },
    });

    worker.onmessage = (evt: MessageEvent<WorkerMessage>) => {
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

        case "error":
          // TODO use code
          const error = new Error(data.value.message ?? "Unknown error");
          if (mayStillReject) {
            mayStillReject = false;
            rejectProm(error);
          }
          break;

        case "seek":
          this.videoElement.currentTime = data.value;
          break;

        case "attach-media-source":
          if (data.value.handle !== undefined) {
            this.videoElement.srcObject = data.value.handle;
          } else if (data.value.src !== undefined) {
            this.videoElement.src = data.value.src;
          } else {
            // TODO MediaSourceError?
          }
          this._playbackData = {
            mediaSourceId: data.value.mediaSourceId,
            mediaSource: null,
            dispose: () => {
              if (data.value.src !== undefined) {
                URL.revokeObjectURL(data.value.src);
              }
            },
            sourceBuffers: [],
            observationsData: null,
          };
          break;

        case "create-media-source": {
          const { mediaSourceId } = data.value;

          // TODO handle error
          const mediaSource = new MediaSource();

          // TODO do something with dispose code
          const dispose =
            bindMediaSource(worker, mediaSource, this.videoElement, mediaSourceId);
          this._playbackData = {
            mediaSourceId: data.value.mediaSourceId,
            mediaSource,
            dispose,
            sourceBuffers: [],
            observationsData: null,
          };
          break;
        }

        case "update-media-source-duration": {
          const { mediaSourceId } = data.value;
          if (
            this._playbackData?.mediaSourceId !== mediaSourceId ||
            this._playbackData.mediaSource === null
          ) {
            return;
          }
          try {
            this._playbackData.mediaSource.duration = data.value.duration;
          } catch (err) {
            // TODO handle errors
          }
          break;
        }

        case "clear-media-source": {
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          this._playbackData.dispose?.();
          clearElementSrc(this.videoElement);
          break;
        }

        case "create-source-buffer": {
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          if (this._playbackData.mediaSource === null) {
            // TODO error
            return;
          }
          try {
            const sourceBuffer = this._playbackData.mediaSource
              .addSourceBuffer(data.value.contentType);
            const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
            this._playbackData.sourceBuffers.push({
              sourceBufferId: data.value.sourceBufferId,
              queuedSourceBuffer,
            });
          } catch (err) {
            // TODO
          }
          // TODO
          break;
        }

        case "append-buffer": {
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          const sbObject = this._playbackData.sourceBuffers
            .find(({ sourceBufferId }) => sourceBufferId === data.value.sourceBufferId);
          if (sbObject === undefined || sbObject.queuedSourceBuffer === null) {
            // TODO error
            return;
          }
          sbObject.queuedSourceBuffer.push(data.value.data)
            .then(() => {
              postMessageToWorker(worker, {
                type: "source-buffer-updated",
                value: {
                  mediaSourceId: data.value.mediaSourceId,
                  sourceBufferId: data.value.sourceBufferId,
                },
              });
            })
            .catch((_err: unknown) => {
              // TODO report err
            });
          break;
        }

        case "remove-buffer": {
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          const sbObject = this._playbackData.sourceBuffers
            .find(({ sourceBufferId }) => sourceBufferId === data.value.sourceBufferId);
          if (sbObject === undefined || sbObject.queuedSourceBuffer === null) {
            // TODO error
            return;
          }
          sbObject.queuedSourceBuffer.removeBuffer(data.value.start, data.value.end)
            .catch((_err) => {
              // TODO report err
            });
          break;
        }

        case "start-playback-observation": {
          this._startPlaybackObservation(data.value.mediaSourceId);
          break;
        }

        case "stop-playback-observation": {
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          this._stopObservingPlayback();
          break;
        }

        case "end-of-stream":
          if (this._playbackData?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          if (this._playbackData.mediaSource === null) {
            // TODO error
            return;
          }
          try {
            this._playbackData.mediaSource.endOfStream();
          } catch (err) {
            // TODO report error?
          }
          break;
      }
    };

    worker.onerror = (ev: ErrorEvent) => {
      rejectProm(ev.error);
    };
  }

  private _startPlaybackObservation(mediaSourceId: number): void {
    if (
      this._playbackData?.mediaSourceId !== mediaSourceId ||
      this._worker === null ||
      this._playbackData.observationsData !== null
    ) {
      return;
    }
    const worker = this._worker;
    const videoElement = this.videoElement;

    const listenerRemovers = OBSERVATION_EVENTS.map(([evtName, reason]) => {
      videoElement.addEventListener(evtName, onEvent);
      function onEvent() {
        onNextTick(reason);
      }
      return () => videoElement.removeEventListener(evtName, onEvent);
    });

    this._playbackData.observationsData = {
      removeEventListeners() {
        listenerRemovers.forEach(removeCb => removeCb());
      },
      timeoutId: undefined,
      isStopped: false,
    };
    const observationsData = this._playbackData.observationsData;
    /* eslint-disable @typescript-eslint/no-floating-promises */
    Promise.resolve().then(() => onNextTick(PlaybackTickReason.Init));
    function onNextTick(reason: PlaybackTickReason) {
      if (observationsData.isStopped) {
        return;
      }
      if (observationsData.timeoutId !== undefined) {
        clearTimeout(observationsData.timeoutId);
        observationsData.timeoutId = undefined;
      }

      const buffered = new Float64Array(videoElement.buffered.length * 2);
      for (let i = 0; i < videoElement.buffered.length; i++) {
        const offset = i * 2;
        buffered[offset] = videoElement.buffered.start(i);
        buffered[offset + 1] = videoElement.buffered.end(i);
      }

      const { currentTime, readyState, paused, seeking } = videoElement;
      postMessageToWorker(worker, {
        type: "observation",
        value: {
          mediaSourceId,
          reason,
          currentTime,
          readyState,
          buffered,
          paused,
          seeking,
        },
      });

      observationsData.timeoutId = setTimeout(() => {
        if (observationsData.isStopped) {
          observationsData.timeoutId = undefined;
          return;
        }
        onNextTick(PlaybackTickReason.RegularInterval);
      }, 1000);
    }
  }

  private _stopObservingPlayback(): void {
    if (this._playbackData === null || this._playbackData.observationsData === null) {
      return;
    }
    this._playbackData.observationsData.isStopped = true;
    this._playbackData.observationsData.removeEventListeners();
    if (this._playbackData.observationsData.timeoutId !== undefined) {
      clearTimeout(this._playbackData.observationsData.timeoutId);
    }
    this._playbackData.observationsData = null;
  }
}

function bindMediaSource(
  worker: Worker,
  mediaSource: MediaSource,
  videoElement: HTMLVideoElement,
  mediaSourceId: number
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

const enum InitializationStatus {
  Uninitialized = "Uninitialized",
  Initializing = "Initializing",
  Initialized = "Initialized",
  Errored = "errorred",
  Disposed = "disposed",
}

export interface InitializationOptions {
  workerUrl: string;
  wasmUrl: string;
}

function noop() {
  /* do nothing! */
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
