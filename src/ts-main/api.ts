import EventEmitter from "../ts-common/EventEmitter";
import idGenerator from "../ts-common/idGenerator";
import logger, {
  LoggerLevel,
} from "../ts-common/logger";
import noop from "../ts-common/noop";
import { WorkerMessage } from "../ts-common/types";
import InitializationError from "./errors";
import postMessageToWorker from "./postMessageToWorker";
import {
  ContentMetadata,
  PlayerState,
} from "./types";
import {
  requestStopForContent,
  waitForLoad,
} from "./utils";
import {
  onAppendBufferMessage,
  onAttachMediaSourceMessage,
  onClearMediaSourceMessage,
  onContentErrorMessage,
  onContentInfoUpdateMessage,
  onContentStoppedMessage,
  onContentWarningMessage,
  onCreateMediaSourceMessage,
  onCreateSourceBufferMessage,
  onEndOfStreamMessage,
  onMediaOffsetUpdateMessage,
  onRebufferingEndedMessage,
  onRebufferingStartedMessage,
  onRemoveBufferMessage,
  onSeekMessage,
  onStartPlaybackObservationMessage,
  onStopPlaybackObservationMessage,
  onUpdateMediaSourceDurationMessage,
  onUpdatePlaybackRateMessage,
} from "./worker-message-handlers";

// Allows to ensure a never-seen-before identifier is used for each content.
const generateContentId = idGenerator();

/** List events triggered by a `WaspHlsPlayer` and corresponding payloads. */
interface WaspHlsPlayerEvents {
  /**
   * Sent when a new content is being loaded.
   *
   * The `getPlayerState` method should now return `Loading`.
   */
  loading: null;
  /**
   * Sent when that last loaded content can start to play.
   *
   * The `getPlayerState` method should now return `Loaded`.
   */
  loaded: null;
  /**
   * Sent when a content is succesfully stopped an no content is
   * currently left playing.
   *
   * The `getPlayerState` method should now return `Stopped`.
   */
  stopped: null;
  /**
   * Playback is now paused with a loaded content.
   *
   * The `isPaused` method should now return `true`.
   */
  pause: null;
  /**
   * Playback is now playing at the communicated playback rate with a loaded
   * content.
   *
   * The `isPlaying` method should now return `true`.
   */
  play: null;
  /**
   * Sent when an error provoked the impossibility to continue playing the
   * content.
   *
   * The `getPlayerState` method should now return `Error`.
   */
  error: Error;
  // TODO
  warning: Error;
  /**
   * Sent when we're starting to rebuffer to build back buffer.
   * Playback is usually not advancing while rebuffering.
   *
   * The `isRebuffering` method should now return `true` as long as rebuffering
   * is still pending.
   *
   * Rebuffering will keep being pending until any of those events happen:
   *   - a `rebufferingEnded` event has been sent.
   *   - the content was stopped
   *   - the content errored
   */
  rebufferingStarted: null;

  /**
   * Sent when we're ending a rebuffering period previously announced through
   * the `rebufferingStarted` event.
   *
   * The `isRebuffering` method should now return `false` until the next
   * `rebufferingStarted` event is sent.
   */
  rebufferingEnded: null;
}

/** Various statuses that may be set for a WaspHlsPlayer's initialization. */
const enum InitializationStatus {
  /** The WaspHlsPlayer has never been initialized. */
  Uninitialized = "Uninitialized",
  /** The WaspHlsPlayer is currently initializing. */
  Initializing = "Initializing",
  /** The WaspHlsPlayer has been initialized with success. */
  Initialized = "Initialized",
  /** The WaspHlsPlayer's initialization has failed. */
  Errored = "errorred",
  /** The WaspHlsPlayer's instance has been disposed. */
  Disposed = "disposed",
}

/** Object to provide when calling the `initialize` method. */
export interface InitializationOptions {
  /** Url to the Worker JavaScript script file. */
  workerUrl: string;
  /** Url to the WebAssembly file. */
  wasmUrl: string;
}

/**
 * `WaspHlsPlayer` class allowing to play HLS contents by relying on a WebWorker
 * and WebAssembly.
 *
 * @class {WaspHlsPlayer}
 */
export default class WaspHlsPlayer extends EventEmitter<WaspHlsPlayerEvents> {
  /**
   * Current status for the `WaspHlsPlayer`'s initialization.
   *
   * The `WaspHlsPlayer` has to be initialized before any content can be loaded
   * and played.
   */
  public initializationStatus: InitializationStatus;

  /**
   * The HTMLVideoElement linked to the `WaspHlsPlayer`, on which the content
   * will play.
   *
   * A `WaspHlsPlayer` instance can only be linked to a single
   * `HTMLVideoElement`. You need to create multiple instances to be able to
   * play on several elements.
   *
   * TODO Replace by HTMLMediaElement - detecting when it is an audio one and
   * disabling the video track in that case.
   */
  public videoElement: HTMLVideoElement;

  /**
   * WebWorker associated to this `WaspHlsPlayer` instance.
   *
   * `null` when this `WaspHlsPlayer` hasn't had a succesful initialization yet
   * or if it has been disposed.
   */
  private __worker__ : Worker | null;

  /**
   * Properties associated to the content currently playing or being loaded.
   *
   * `null` only when no content is loaded right now (either none have ever been
   * loaded or the last one has been explicitely stopped).
   */
  private __contentMetadata__: ContentMetadata | null;

  /** AbortController allowing to free resources when `dispose` is called. */
  private __destroyAbortController__: AbortController;

  /** When set, keep track of a log-related callback for later clean-up. */
  private __logLevelChangeListener__: ((logLevel: LoggerLevel) => void) | null;

  /**
   * Create a new WaspHlsPlayer, associating with a video element.
   *
   * Note that you will need to call `initialize` on it befor actually loading
   * the content and perform most other operations.
   *
   * @param {HTMLVideoElement} videoElement
   */
  constructor(videoElement: HTMLVideoElement) {
    super();
    this.videoElement = videoElement;
    this.initializationStatus = InitializationStatus.Uninitialized;
    this.__worker__ = null;
    this.__contentMetadata__ = null;
    this.__logLevelChangeListener__ = null;
    this.__destroyAbortController__ = new AbortController();

    const onPause = () => {
      if (this.getPlayerState() === PlayerState.Loaded) {
        this.trigger("pause", null);
      }
    };
    const onPlay = () => {
      if (
        this.getPlayerState() === PlayerState.Loaded &&
        this.__contentMetadata__ !== null &&
        !this.__contentMetadata__.isRebuffering
      ) {
        this.trigger("play", null);
      }
    };
    this.videoElement.addEventListener("pause", onPause);
    this.videoElement.addEventListener("play", onPlay);

    this.__destroyAbortController__.signal.addEventListener("abort", () => {
      this.videoElement.removeEventListener("pause", onPause);
      this.videoElement.removeEventListener("play", onPlay);
    });
  }

  /**
   * Begin "initialization", that is, start loading the web Worker and
   * WebAssembly parts of the player so it can begin to load contents.
   *
   * The returned Promise:
   *   - Resolves once the initialization is finished with success.
   *     From that point on, you can begin to load contents.
   *
   *   - Rejects if the initialization failed, with a `InitializationError`
   *     describing the error encountered.
   * @param {Object} opts
   * @returns {Promise}
   */
  public initialize(opts: InitializationOptions) : Promise<void> {
    try {
      if (this.initializationStatus !== InitializationStatus.Uninitialized) {
        throw new Error("WaspHlsPlayer already initialized");
      }
      this.initializationStatus = InitializationStatus.Initializing;
      const { wasmUrl, workerUrl } = opts;
      let resolveProm = noop;
      let rejectProm = noop;
      const ret = new Promise<void>((resolve, reject) => {
        resolveProm = resolve;
        rejectProm = reject;
      });
      this.__startWorker__(workerUrl, wasmUrl, resolveProm, rejectProm);
      return ret;
    }
    catch (err) {
      this.initializationStatus = InitializationStatus.Errored;
      return Promise.reject(err);
    }
  }

  /**
   * Loads a new HLS content whose MultiVariantPlaylist's URL is given in
   * argument.
   *
   * This method can only be called if the `WaspHlsPlayer` is initialized.
   * If that's the case, you should receive synchronously a `"loading"` event,
   * indicating that this content is being loaded.
   *
   * You can know when that content is succesfully loaded through the `"loaded"`
   * event or if it failed through the `"error"` event.
   * Note that a loading operation can also just be cancelled because another
   * content has been loaded in the meantime. You can also listen for new
   * `"loading"` events to also catch this scenario.
   *
   * TODO return promise?
   * @param {string} url
   */
  public loadContent(url: string): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (this.__contentMetadata__ !== null) {
      requestStopForContent(this.__contentMetadata__, this.__worker__);
    }
    const contentId = generateContentId();
    const loadingAborter = new AbortController();
    this.__contentMetadata__ = {
      contentId,
      mediaSourceId: null,
      mediaSource: null,
      disposeMediaSource: null,
      sourceBuffers: [],
      stopPlaybackObservations: null,
      isRebuffering: false,
      mediaOffset: undefined,
      wantedSpeed: 1,
      minimumPosition: undefined,
      maximumPosition: undefined,
      loadingAborter,
      error: null,
    };
    this.trigger("loading", null);
    postMessageToWorker(this.__worker__, {
      type: "load",
      value: { contentId, url },
    });
  }

  /**
   * Returns the "state" the `WaspHlsPlayer` is currently in.
   * @see PlayerState
   * @returns {string}
   */
  public getPlayerState() : PlayerState {
    if (this.__contentMetadata__ === null) {
      return PlayerState.Stopped;
    }
    if (this.__contentMetadata__.error !== null) {
      return PlayerState.Error;
    }
    if (this.__contentMetadata__.loadingAborter !== undefined) {
      return PlayerState.Loading;
    }
    return PlayerState.Loaded;
  }

  /**
   * Returns the current position in the content.
   *
   * Returns `0` if no content is loaded.
   *
   * This value should only be considered if a content is currently loaded.
   * That is, the current `PlayerState` is equal to `Loaded`.
   */
  public getPosition(): number {
    if (this.__contentMetadata__ === null) {
      return 0;
    }
    const currentTime = this.videoElement.currentTime;
    return currentTime - (this.__contentMetadata__.mediaOffset ?? 0);
  }

  /**
   * Move the position of playback to the given position.
   *
   * This method can only be called if a content is loaded (`Loaded`
   * player state).
   *
   * @param {number} position
   */
  public seek(position: number): void {
    if (this.getPlayerState() !== PlayerState.Loaded) {
      throw new Error("Cannot seek: no content loaded.");
    }
    this.videoElement.currentTime = position +
      (this.__contentMetadata__?.mediaOffset ?? 0);
  }

  public getMediaOffset(): number | undefined {
    return this.__contentMetadata__?.mediaOffset ?? undefined;
  }

  /**
   * Updates the audio volume.
   *
   * `0` indicates the minimum volume, `1` the maximum volume. Values in between
   * are not necessarily linear and may depend on the platform.
   *
   * TODO Remove? Might be simpler to just say to user to update it directly on
   * the media element.
   * @param {number} volume
   */
  public setVolume(volume: number): void {
    this.videoElement.volume = volume;
  }

  /**
   * Returns `true when there's both a content loaded and playback is not
   * paused.
   * @returns {boolean}
   */
  public isPlaying(): boolean {
    return this.getPlayerState() === PlayerState.Loaded &&
            !this.videoElement.paused;
  }

  /**
   * Returns `true` when playback has been paused.
   *
   * Note that this may also be because no content is currently loaded.
   * @returns {boolean}
   */
  public isPaused(): boolean {
    return this.videoElement.paused;
  }

  /**
   * Returns `true` when playback is currently not advancing because of
   * rebuffering.
   * @returns {boolean}
   */
  public isRebuffering(): boolean {
    return this.__contentMetadata__?.isRebuffering ?? false;
  }

  /**
   * Pause playback of a loaded content.
   *
   * You should receive a `"pause"` event when and if the operation
   * succeeds.
   *
   * This method can only be called if a content is loaded (`Loaded`
   * player state).
   */
  public pause(): void {
    if (this.getPlayerState() !== PlayerState.Loaded) {
      throw new Error("Cannot pause: no content loaded.");
    }
    this.videoElement.pause();
  }

  /**
   * Play/resume playback of a loaded content.
   *
   * You should receive a `"resume"` event when and if the operation
   * succeeds.
   *
   * This method can only be called if a content is loaded (`Loaded`
   * player state).
   */
  public resume(): void {
    if (this.getPlayerState() !== PlayerState.Loaded) {
      throw new Error("Cannot resume: no content loaded.");
    }
    this.videoElement.play()
      .catch(() => { /* noop */});
  }

  /**
   * Stops a currently-playing content.
   *
   * You should receive a `"stopped"` event when the operation succeeds unless
   * another content has been loaded in the meantime (which you can know through
   * the `"loading"` event.
   *
   * This method can only be called if the `WaspHlsPlayer` has been initialized
   * with success and not disposed.
   */
  public stop(): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (this.__contentMetadata__ !== null) {
      requestStopForContent(this.__contentMetadata__, this.__worker__);
    }
  }

  public setSpeed(speed: number): void {
    // There's two ways in which we could do this:
    //
    //   - We could mutate directly the HTMLMediaElement's `playbackRate`
    //     attribute in the main thread, unless the content is currently
    //     rebuffering (in which case it is set when we exit rebuffering).
    //     The core logic then knows about it through the usual
    //     `MediaObservation`.
    //
    //     This has the advantage of being simpler, allowing direct feedback,
    //     being persisted between contents and being able to update it even
    //     when no content is playing. We also don't need to introduce a
    //     supplementary option to update the speed at loading time.
    //
    //   - We could send the order to update the playback rate to the worker
    //     which then sends back the order to update the playback rate.
    //     The advantages here is that the worker knows about the playback rate
    //     change before it actually happens and the core logic totally controls
    //     the playback rate, which may lead to more predictable behavior in the
    //     core logic, which is inherently more complex.
    //
    // For now I went with the second solution even though I think the first is
    // the better one, because why making it simple when you can make it
    // complex?
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (
      this.__contentMetadata__ === null ||
      this.__contentMetadata__.mediaSourceId === null
    ) {
      throw new Error("No content is loaded");
    }

    this.__contentMetadata__.wantedSpeed = speed;
    postMessageToWorker(this.__worker__, {
      type: "update-wanted-speed",
      value: {
        mediaSourceId: this.__contentMetadata__.mediaSourceId,
        wantedSpeed: speed,
      },
    });
  }

  public getSpeed(): number {
    return this.__contentMetadata__?.wantedSpeed ?? 1;
  }

  public getMinimumPosition() : number | undefined {
    return this.__contentMetadata__?.minimumPosition;
  }

  public getMaximumPosition() : number | undefined {
    return this.__contentMetadata__?.maximumPosition;
  }

  public getError() : Error | null {
    return this.__contentMetadata__?.error ?? null;
  }

  public dispose() {
    this.__destroyAbortController__.abort();
    if (this.__worker__ === null) {
      return;
    }
    if (this.__contentMetadata__ !== null) {
      requestStopForContent(this.__contentMetadata__, this.__worker__);
    }
    // TODO needed? What about GC once it is set to `null`?
    postMessageToWorker(this.__worker__, { type: "dispose", value: null });
    this.__worker__ = null;
    if (this.__logLevelChangeListener__ !== null) {
      logger.removeEventListener("onLogLevelChange", this.__logLevelChangeListener__);
    }
  }

  private __startWorker__(
    workerUrl: string,
    wasmUrl: string,
    resolveProm: () => void,
    rejectProm: (err: unknown) => void
  ) {
    let mayStillReject = true;
    const worker = new Worker(workerUrl);
    this.__worker__ = worker;
    postMessageToWorker(worker, {
      type: "init",
      value: {
        hasWorkerMse: typeof MediaSource === "function" &&
          /* eslint-disable-next-line */
          (MediaSource as any).canConstructInDedicatedWorker === true,
        wasmUrl,
        logLevel: logger.getLevel(),
      },
    });

    if (this.__logLevelChangeListener__ !== null) {
      logger.removeEventListener("onLogLevelChange", this.__logLevelChangeListener__);
    }
    logger.addEventListener("onLogLevelChange", onLogLevelChange);
    this.__logLevelChangeListener__ = onLogLevelChange;

    worker.onmessage = (evt: MessageEvent<WorkerMessage>) => {
      const { data } = evt;
      if (typeof data !== "object" || data === null || typeof data.type !== "string") {
        logger.error("unexpected Worker message");
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
            const error = new InitializationError(
              data.value.code,
              data.value.wasmHttpStatus,
              data.value.message ?? "Error while initializing the WaspHlsPlayer"
            );
            mayStillReject = false;
            rejectProm(error);
          }
          break;
        case "seek":
          onSeekMessage(data, this.__contentMetadata__, this.videoElement);
          break;
        case "update-playback-rate":
          onUpdatePlaybackRateMessage(data, this.__contentMetadata__, this.videoElement);
          break;
        case "attach-media-source":
          onAttachMediaSourceMessage(data, this.__contentMetadata__, this.videoElement);
          this.__startListeningToLoadedEvent__();
          break;
        case "create-media-source":
          onCreateMediaSourceMessage(
            data,
            this.__contentMetadata__,
            this.videoElement,
            worker
          );
          this.__startListeningToLoadedEvent__();
          break;
        case "update-media-source-duration":
          onUpdateMediaSourceDurationMessage(data, this.__contentMetadata__, worker);
          break;
        case "clear-media-source":
          onClearMediaSourceMessage(data, this.__contentMetadata__, this.videoElement);
          break;
        case "create-source-buffer":
          onCreateSourceBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case "append-buffer":
          onAppendBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case "remove-buffer":
          onRemoveBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case "start-playback-observation":
          onStartPlaybackObservationMessage(
            data,
            this.__contentMetadata__,
            this.videoElement,
            worker
          );
          break;
        case "stop-playback-observation":
          onStopPlaybackObservationMessage(data, this.__contentMetadata__);
          break;
        case "end-of-stream":
          onEndOfStreamMessage(data, this.__contentMetadata__, worker);
          break;
        case "media-offset-update":
          onMediaOffsetUpdateMessage(data, this.__contentMetadata__);
          break;
        case "content-error": {
          const error = onContentErrorMessage(data, this.__contentMetadata__);
          if (error !== null) {
            this.trigger("error", error);
          }
          break;
        }
        case "content-warning": {
          const error = onContentWarningMessage(data, this.__contentMetadata__);
          if (error !== null) {
            this.trigger("warning", error);
          }
          break;
        }
        case "content-info-update":
          onContentInfoUpdateMessage(data, this.__contentMetadata__);
          break;
        case "content-stopped":
          if (onContentStoppedMessage(data, this.__contentMetadata__)) {
            this.__contentMetadata__ = null;
            this.trigger("stopped", null);
          }
          break;
        case "rebuffering-started":
          if (onRebufferingStartedMessage(
            data,
            this.__contentMetadata__,
            this.videoElement)
          ) {
            this.trigger("rebufferingStarted", null);
          }
          break;
        case "rebuffering-ended":
          if (onRebufferingEndedMessage(
            data,
            this.__contentMetadata__,
            this.videoElement)
          ) {
            this.trigger("rebufferingEnded", null);
          }
          break;
      }

    };

    // TODO check on which case this is triggered
    worker.onerror = (ev: ErrorEvent) => {
      logger.error("API: Worker Error encountered", ev.error);
      if (mayStillReject) {
        rejectProm(ev.error);
      }
    };
    function onLogLevelChange(level: LoggerLevel): void {
      postMessageToWorker(worker, {
        type: "update-logger-level",
        value: level,
      });
    }
  }

  private __startListeningToLoadedEvent__() {
    const contentMetadata = this.__contentMetadata__;
    if (contentMetadata === null) {
      return;
    } else if (contentMetadata.loadingAborter === undefined) {
      return;
    }
    waitForLoad(this.videoElement, contentMetadata.loadingAborter.signal).then(
      () => {
        if (this.__contentMetadata__ !== null) {
          this.__contentMetadata__.loadingAborter = undefined;
        }
        this.trigger("loaded", null);
      },
      (reason) => {
        if (this.__contentMetadata__ !== null) {
          this.__contentMetadata__.loadingAborter = undefined;
        }
        logger.info("Could not load content:", reason);
      }
    );
  }
}
