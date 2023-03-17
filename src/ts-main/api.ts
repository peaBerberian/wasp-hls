import assertNever from "../ts-common/assertNever";
import EventEmitter from "../ts-common/EventEmitter";
import idGenerator from "../ts-common/idGenerator";
import logger, { LoggerLevel } from "../ts-common/logger";
import noop from "../ts-common/noop";
import {
  AudioTrackInfo,
  MainMessageType,
  VariantInfo,
  WaspHlsPlayerConfig,
  WorkerMessage,
  WorkerMessageType,
} from "../ts-common/types";
import { MediaType } from "../wasm/wasp_hls";
import { WaspInitializationError } from "./errors";
import postMessageToWorker from "./postMessageToWorker";
import { ContentMetadata, PlayerState } from "./types";
import { requestStopForContent, waitForLoad } from "./utils";
import {
  onAppendBufferMessage,
  onAttachMediaSourceMessage,
  onClearMediaSourceMessage,
  onErrorMessage,
  onContentTimeBoundsUpdateMessage,
  onContentStoppedMessage,
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
  onWarningMessage,
  onMultiVariantPlaylistParsedMessage,
  onVariantUpdateMessage,
  onTrackUpdateMessage,
  onFlushMessage,
  onAreTypesSupportedMessage,
} from "./worker-message-handlers";

const DEFAULT_MPEG2_TS_TYPE =
  "video/mp2t;codecs=\"avc1.4D401F\"";

// Allows to ensure a never-seen-before identifier is used for each content.
const generateContentId = idGenerator();

/** List events triggered by a `WaspHlsPlayer` and corresponding payloads. */
interface WaspHlsPlayerEvents {
  /**
   * Sent when WaspHlsPlayerEvents transition from one PlayerState to another,
   * with the new state as a payload.
   *
   * The `getPlayerState` method should now return that new state.
   */
  playerStateChange: PlayerState;
  /**
   * Playback is now paused with a loaded content.
   *
   * The `isPaused` method should now return `true`.
   */
  paused: null;
  /**
   * Playback is now playing as long as there's data in the buffer.
   *
   * The `isPlaying` method should now return `true`.
   */
  playing: null;
  /**
   * We reached the end of the content.
   *
   * The `isEnded` method should now return `true`.
   */
  ended: null;
  /**
   * Sent when an error provoked the impossibility to continue playing the
   * content.
   *
   * The `getPlayerState` method should now return `Error`.
   */
  error: Error;
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

  variantUpdate: VariantInfo | undefined;

  audioTrackUpdate: AudioTrackInfo | undefined;

  audioTrackListUpdate: AudioTrackInfo[];

  variantListUpdate: VariantInfo[];
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
  Errored = "errored",
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

const DEFAULT_CONFIG: WaspHlsPlayerConfig = {
  bufferGoal: 15,

  segmentRequestTimeout: 20000,
  segmentBackoffBase: 300,
  segmentBackoffMax: 2000,
  multiVariantPlaylistRequestTimeout: 15000,
  multiVariantPlaylistBackoffBase: 300,
  multiVariantPlaylistBackoffMax: 2000,
  mediaPlaylistRequestTimeout: 15000,
  mediaPlaylistBackoffBase: 300,
  mediaPlaylistBackoffMax: 2000,
};

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
  private __worker__: Worker | null;

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

  private __config__: WaspHlsPlayerConfig;

  /**
   * Create a new WaspHlsPlayer, associating with a video element.
   *
   * Note that you will need to call `initialize` on it befor actually loading
   * the content and perform most other operations.
   *
   * @param {HTMLVideoElement} videoElement
   */
  constructor(
    videoElement: HTMLVideoElement,
    config?: Partial<WaspHlsPlayerConfig> | undefined
  ) {
    super();
    this.videoElement = videoElement;
    this.initializationStatus = InitializationStatus.Uninitialized;
    this.__worker__ = null;
    this.__contentMetadata__ = null;
    this.__logLevelChangeListener__ = null;
    this.__destroyAbortController__ = new AbortController();
    this.__config__ = { ...DEFAULT_CONFIG, ...(config ?? {}) };

    const onPause = () => {
      if (this.getPlayerState() === PlayerState.Loaded) {
        this.trigger("paused", null);
      }
    };
    const onEnded = () => {
      if (this.getPlayerState() === PlayerState.Loaded) {
        this.trigger("ended", null);
      }
    };
    const onPlay = () => {
      if (
        this.getPlayerState() === PlayerState.Loaded &&
        this.__contentMetadata__ !== null
      ) {
        this.trigger("playing", null);
      }
    };
    this.videoElement.addEventListener("pause", onPause);
    this.videoElement.addEventListener("play", onPlay);
    this.videoElement.addEventListener("ended", onEnded);

    this.__destroyAbortController__.signal.addEventListener("abort", () => {
      this.videoElement.removeEventListener("pause", onPause);
      this.videoElement.removeEventListener("play", onPlay);
      this.videoElement.removeEventListener("ended", onEnded);
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
   *   - Rejects if the initialization failed, with a
   *    `WaspInitializationError` describing the error encountered.
   * @param {Object} opts
   * @returns {Promise}
   */
  public initialize(opts: InitializationOptions): Promise<void> {
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
    } catch (err) {
      this.initializationStatus = InitializationStatus.Errored;
      return Promise.reject(err);
    }
  }

  public getConfig(): WaspHlsPlayerConfig {
    return this.__config__;
  }

  public updateConfig(overwrite: Partial<WaspHlsPlayerConfig>): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    this.__config__ = { ...this.__config__, ...overwrite };
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.UpdateConfig,
      value: this.__config__,
    });
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
      variants: [],
      audioTracks: [],
      currentAudioTrack: undefined,
      currVariant: undefined,
      lockedVariant: null,
      stopPlaybackObservations: null,
      isRebuffering: false,
      mediaOffset: undefined,
      wantedSpeed: 1,
      minimumPosition: undefined,
      maximumPosition: undefined,
      loadingAborter,
      error: null,
    };
    this.trigger("playerStateChange", PlayerState.Loading);
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LoadContent,
      value: { contentId, url },
    });
  }

  /**
   * Returns the "state" the `WaspHlsPlayer` is currently in.
   * @see PlayerState
   * @returns {string}
   */
  public getPlayerState(): PlayerState {
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
    this.videoElement.currentTime =
      position + (this.__contentMetadata__?.mediaOffset ?? 0);
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
   * Note that `isPlaying` returns true even if playback is stalled due to
   * rebuffering (you can check `isRebuffering` for this).
   * @returns {boolean}
   */
  public isPlaying(): boolean {
    return (
      this.getPlayerState() === PlayerState.Loaded && !this.videoElement.paused
    );
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
   * Returns `true` when playback has ended.
   * @returns {boolean}
   */
  public isEnded(): boolean {
    return this.videoElement.ended;
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
    this.videoElement.play().catch(() => {
      /* noop */
    });
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
      type: MainMessageType.UpdateWantedSpeed,
      value: {
        mediaSourceId: this.__contentMetadata__.mediaSourceId,
        wantedSpeed: speed,
      },
    });
  }

  public getSpeed(): number {
    return this.__contentMetadata__?.wantedSpeed ?? 1;
  }

  public getMinimumPosition(): number | undefined {
    return this.__contentMetadata__?.minimumPosition;
  }

  public getMaximumPosition(): number | undefined {
    return this.__contentMetadata__?.maximumPosition;
  }

  public getError(): Error | null {
    return this.__contentMetadata__?.error ?? null;
  }

  public getCurrentVariant(): VariantInfo | undefined {
    return this.__contentMetadata__?.currVariant ?? undefined;
  }

  public getVariantList(): VariantInfo[] {
    return this.__contentMetadata__?.variants ?? [];
  }

  public getAudioTrackList(): AudioTrackInfo[] {
    return this.__contentMetadata__?.audioTracks ?? [];
  }

  public getAudioTrack(): AudioTrackInfo | undefined {
    const id = this.__contentMetadata__?.currentAudioTrack?.id;
    if (id === undefined) {
      return undefined;
    }
    return this.getAudioTrackList()?.find((a) => a.id === id);
  }

  public setAudioTrack(trackId: string | null): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (this.__contentMetadata__ === null) {
      throw new Error("No content loaded");
    }
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.SetAudioTrack,
      value: {
        contentId: this.__contentMetadata__.contentId,
        trackId,
      },
    });
  }

  public lockVariant(variantId: string) {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (this.__contentMetadata__ === null) {
      throw new Error("No content loaded");
    }
    this.__contentMetadata__.lockedVariant = variantId;
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LockVariant,
      value: {
        contentId: this.__contentMetadata__.contentId,
        variantId,
      },
    });
  }

  public unlockVariant() {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or disposed.");
    }
    if (this.__contentMetadata__ === null) {
      throw new Error("No content loaded");
    }
    this.__contentMetadata__.lockedVariant = null;
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LockVariant,
      value: {
        contentId: this.__contentMetadata__.contentId,
        variantId: null,
      },
    });
  }

  public getLockedVariant(): string | null {
    return this.__contentMetadata__?.lockedVariant ?? null;
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
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.DisposePlayer,
      value: null,
    });
    this.__worker__ = null;
    if (this.__logLevelChangeListener__ !== null) {
      logger.removeEventListener(
        "onLogLevelChange",
        this.__logLevelChangeListener__
      );
    }
    this.videoElement.src = "";
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
      type: MainMessageType.Initialization,
      value: {
        hasMseInWorker:
          typeof MediaSource === "function" &&
          /* eslint-disable-next-line */
          (MediaSource as any).canConstructInDedicatedWorker === true,
        canDemuxMpeg2Ts:
          typeof MediaSource === "function" &&
          MediaSource.isTypeSupported(DEFAULT_MPEG2_TS_TYPE),
        wasmUrl,
        logLevel: logger.getLevel(),
        initialConfig: this.__config__,
      },
    });

    if (this.__logLevelChangeListener__ !== null) {
      logger.removeEventListener(
        "onLogLevelChange",
        this.__logLevelChangeListener__
      );
    }
    logger.addEventListener("onLogLevelChange", onLogLevelChange);
    this.__logLevelChangeListener__ = onLogLevelChange;

    worker.onmessage = (evt: MessageEvent<WorkerMessage>) => {
      const { data } = evt;
      if (
        typeof data !== "object" ||
        data === null ||
        typeof data.type === "undefined"
      ) {
        logger.error("unexpected Worker message");
        return;
      }

      switch (data.type) {
        case WorkerMessageType.Initialized:
          this.initializationStatus = InitializationStatus.Initialized;
          mayStillReject = false;
          resolveProm();
          break;
        case WorkerMessageType.InitializationError:
          if (mayStillReject) {
            const error = new WaspInitializationError(
              data.value.code,
              data.value.wasmHttpStatus,
              data.value.message ?? "Error while initializing the WaspHlsPlayer"
            );
            mayStillReject = false;
            rejectProm(error);
          }
          break;
        case WorkerMessageType.Seek:
          onSeekMessage(data, this.__contentMetadata__, this.videoElement);
          break;
        case WorkerMessageType.Flush:
          onFlushMessage(data, this.__contentMetadata__, this.videoElement);
          break;
        case WorkerMessageType.UpdatePlaybackRate:
          console.warn("UPDATE PBR", data.value.playbackRate);
          onUpdatePlaybackRateMessage(
            data,
            this.__contentMetadata__,
            this.videoElement
          );
          break;
        case WorkerMessageType.AttachMediaSource:
          onAttachMediaSourceMessage(
            data,
            this.__contentMetadata__,
            this.videoElement
          );
          this.__startListeningToLoadedEvent__();
          break;
        case WorkerMessageType.CreateMediaSource:
          onCreateMediaSourceMessage(
            data,
            this.__contentMetadata__,
            this.videoElement,
            worker
          );
          this.__startListeningToLoadedEvent__();
          break;
        case WorkerMessageType.UpdateMediaSourceDuration:
          onUpdateMediaSourceDurationMessage(
            data,
            this.__contentMetadata__,
            worker
          );
          break;
        case WorkerMessageType.ClearMediaSource:
          onClearMediaSourceMessage(
            data,
            this.__contentMetadata__,
            this.videoElement
          );
          break;
        case WorkerMessageType.CreateSourceBuffer:
          onCreateSourceBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case WorkerMessageType.AppendBuffer:
          onAppendBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case WorkerMessageType.RemoveBuffer:
          onRemoveBufferMessage(data, this.__contentMetadata__, worker);
          break;
        case WorkerMessageType.StartPlaybackObservation:
          onStartPlaybackObservationMessage(
            data,
            this.__contentMetadata__,
            this.videoElement,
            worker
          );
          break;
        case WorkerMessageType.StopPlaybackObservation:
          onStopPlaybackObservationMessage(data, this.__contentMetadata__);
          break;
        case WorkerMessageType.EndOfStream:
          onEndOfStreamMessage(data, this.__contentMetadata__, worker);
          break;
        case WorkerMessageType.MediaOffsetUpdate:
          onMediaOffsetUpdateMessage(data, this.__contentMetadata__);
          break;
        case WorkerMessageType.MultiVariantPlaylistParsed:
          if (
            onMultiVariantPlaylistParsedMessage(data, this.__contentMetadata__)
          ) {
            this.trigger("variantListUpdate", this.getVariantList());
            this.trigger("audioTrackListUpdate", this.getAudioTrackList());
          }
          break;
        case WorkerMessageType.TrackUpdate:
          if (onTrackUpdateMessage(data, this.__contentMetadata__)) {
            if (data.value.mediaType === MediaType.Audio) {
              this.trigger("audioTrackUpdate", this.getAudioTrack());
            }
          }
          break;
        case WorkerMessageType.VariantUpdate:
          if (onVariantUpdateMessage(data, this.__contentMetadata__)) {
            this.trigger("variantUpdate", this.getCurrentVariant());
          }
          break;
        case WorkerMessageType.Error: {
          const error = onErrorMessage(data, this.__contentMetadata__);
          if (error !== null) {
            logger.error("API: sending fatal error", error);
            this.trigger("error", error);
            this.trigger("playerStateChange", PlayerState.Error);
          }
          break;
        }
        case WorkerMessageType.Warning: {
          const error = onWarningMessage(data, this.__contentMetadata__);
          if (error !== null) {
            this.trigger("warning", error);
          }
          break;
        }
        case WorkerMessageType.ContentTimeBoundsUpdate:
          onContentTimeBoundsUpdateMessage(data, this.__contentMetadata__);
          break;
        case WorkerMessageType.ContentStopped:
          if (onContentStoppedMessage(data, this.__contentMetadata__)) {
            this.__contentMetadata__ = null;
            this.trigger("playerStateChange", PlayerState.Stopped);
          }
          break;
        case WorkerMessageType.RebufferingStarted:
          if (
            onRebufferingStartedMessage(
              data,
              this.__contentMetadata__,
              this.videoElement
            )
          ) {
            this.trigger("rebufferingStarted", null);
          }
          break;
        case WorkerMessageType.RebufferingEnded:
          if (
            onRebufferingEndedMessage(
              data,
              this.__contentMetadata__,
              this.videoElement
            )
          ) {
            this.trigger("rebufferingEnded", null);
          }
          break;

        case WorkerMessageType.AreTypesSupported:
          onAreTypesSupportedMessage(data, worker);
          break;

        default:
          assertNever(data);
      }
    };

    // TODO check on which case this is triggered
    worker.onerror = (ev: ErrorEvent) => {
      const error = ev.error instanceof Error ? ev.error : "Unknown Error";
      logger.error("API: Worker Error encountered", error);
      if (mayStillReject) {
        rejectProm(ev.error);
      }
      this.dispose();
    };
    function onLogLevelChange(level: LoggerLevel): void {
      postMessageToWorker(worker, {
        type: MainMessageType.UpdateLoggerLevel,
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
        this.trigger("playerStateChange", PlayerState.Loaded);
      },
      (reason) => {
        if (this.__contentMetadata__ !== null) {
          this.__contentMetadata__.loadingAborter = undefined;
        }
        const err = reason instanceof Error ? reason : "Unknown reason";
        logger.info("Could not load content:", err);
      }
    );
  }
}
