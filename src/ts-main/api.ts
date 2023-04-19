import assertNever from "../ts-common/assertNever";
import EventEmitter from "../ts-common/EventEmitter";
import idGenerator from "../ts-common/idGenerator";
import logger, { LoggerLevel } from "../ts-common/logger";
import noop from "../ts-common/noop";
import {
  AudioTrackInfo,
  MainMessageType,
  InitializationErrorCode,
  VariantInfo,
  WaspHlsPlayerConfig,
  WorkerMessage,
  WorkerMessageType,
} from "../ts-common/types";
import {
  MediaType,
  PlaylistNature,
  StartingPositionType,
} from "../wasm/wasp_hls";
import DEFAULT_CONFIG from "./default_config";
import { WaspError, WaspInitializationError } from "./errors";
import postMessageToWorker from "./postMessageToWorker";
import { ContentMetadata, PlayerState } from "./types";
import {
  canDemuxMpeg2Ts,
  potentiallyRelativeUrlToAbsoluteUrl,
  requestStopForContent,
  waitForLoad,
} from "./utils";
import {
  onAppendBufferMessage,
  onAttachMediaSourceMessage,
  onClearMediaSourceMessage,
  onErrorMessage,
  onContentInfoUpdateMessage,
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
  onMultivariantPlaylistParsedMessage,
  onVariantUpdateMessage,
  onTrackUpdateMessage,
  onFlushMessage,
  onAreTypesSupportedMessage,
  onVariantLockStatusChangeMessage,
} from "./worker-message-handlers";

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
   * Playback should now be playing as long as there's data in the buffer.
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
  error: WaspError;
  /**
   * Sent when a minor error arised, which does not prevent from playing the
   * current content.
   */
  warning: WaspError;
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
  /**
   * Sent when some information about the loaded content are updated, with those
   * information as a payload.
   */
  contentInfoUpdate: ContentInfoUpdatePayload;
  /**
   * Sent when the current HLS variant loaded by the `WaspHlsPlayer` changed.
   */
  variantUpdate: VariantInfo | undefined;
  /**
   * Sent when a variant becomes locked (in which case the payload corresponds
   * to the information on the locked variant) or unlocked (in which case the
   * payload is set to `null`).
   */
  variantLockUpdate: VariantInfo | null;
  /**
   * Sent when the list of available HLS variants changed.
   */
  variantListUpdate: VariantInfo[];
  /**
   * Sent when the current audio track loaded by the `WaspHlsPlayer` changed.
   */
  audioTrackUpdate: AudioTrackInfo | undefined;
  /**
   * Sent when the list of available audio tracks changed.
   */
  audioTrackListUpdate: AudioTrackInfo[];
}

/** Payload sent with a `contentInfoUpdate` event. */
export interface ContentInfoUpdatePayload {
  /** New minimum position reachable in the current content. */
  minimumPosition: number | undefined;
  /** New maximum position reachable in the current content. */
  maximumPosition: number | undefined;
  /** if `true` the content is a still pending live content. */
  isLive: boolean;
  /** if `true` the content is a finished VOD content. */
  isVod: boolean;
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
  /**
   * An initial bandwidth estimate which will be relied on initially, in bits
   * per second.
   */
  initialBandwidth?: number | undefined;
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
   * Note that you will need to call `initialize` on it before actually loading
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
      let resolveProm = noop;
      let rejectProm = noop;
      const ret = new Promise<void>((resolve, reject) => {
        resolveProm = resolve;
        rejectProm = reject;
      });
      this.__startWorker__(opts, resolveProm, rejectProm);
      return ret;
    } catch (err) {
      this.initializationStatus = InitializationStatus.Errored;
      return Promise.reject(err);
    }
  }

  /**
   * Returns active `WaspHlsPlayerConfig`.
   *
   * You can update its properties by communicating those you want to update to
   * `updateConfig`.
   * @returns {Object}
   */
  public getConfig(): WaspHlsPlayerConfig {
    return this.__config__;
  }

  /**
   * Update active `WaspHlsPlayerConfig`.
   *
   * You can just give to this method a subset of the configuration's
   * properties, in which case only those keys will be updated.
   * @param {Object} overwrite
   */
  public updateConfig(overwrite: Partial<WaspHlsPlayerConfig>): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
    }
    for (const [key, value] of Object.entries(overwrite)) {
      if (value !== undefined) {
        this.__config__[key as keyof WaspHlsPlayerConfig] = value;
      }
    }
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.UpdateConfig,
      value: this.__config__,
    });
  }

  /**
   * Loads a new HLS content whose MultivariantPlaylist's URL is given in
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
   * NOTE: should we return a promise here?
   * @param {string} url
   * @param {Object} opts
   */
  public load(url: string, opts?: LoadOptions | undefined): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
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
      playbackObserver: null,
      isRebuffering: false,
      mediaOffset: undefined,
      wantedSpeed: 1,
      minimumPosition: undefined,
      maximumPosition: undefined,
      playlistType: undefined,
      loadingAborter,
      error: null,
    };
    this.trigger("playerStateChange", PlayerState.Loading);

    let startingPosition;
    if (opts?.startingPosition !== undefined) {
      if (typeof opts.startingPosition === "number") {
        startingPosition = {
          startingType: StartingPositionType.Absolute,
          position: opts.startingPosition,
        };
      } else {
        const position = opts.startingPosition.position;
        switch (opts.startingPosition.startType) {
          case "Absolute":
            startingPosition = {
              startingType: StartingPositionType.Absolute,
              position,
            };
            break;
          case "FromBeginning":
            startingPosition = {
              startingType: StartingPositionType.FromBeginning,
              position,
            };
            break;
          case "FromEnd":
            startingPosition = {
              startingType: StartingPositionType.FromEnd,
              position,
            };
            break;
        }
      }
    }
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LoadContent,
      value: { contentId, url, startingPosition },
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
   *
   * @returns {number}
   */
  public getPosition(): number {
    if (
      this.__contentMetadata__ === null ||
      this.getPlayerState() !== PlayerState.Loaded
    ) {
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

  /**
   * Returns the offset to convert from playlist time (the time the content is
   * at and most `WaspHlsPlayer` API are) into media time (the time the media
   * element on the page is at) by additionning that offset to the former value
   * (playlist time + media offset = media time).
   *
   * You may want to rely on that value when directly exploiting time
   * information from the media element.
   *
   * Returns `undefined` if that offset is unknown yet.
   *
   * @returns {number|undefined}
   */
  public getMediaOffset(): number | undefined {
    return this.__contentMetadata__?.mediaOffset ?? undefined;
  }

  /**
   * Returns `true` when there's both a content loaded and playback is not
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
    if (this.getPlayerState() !== PlayerState.Loaded) {
      return false;
    }
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
  public resume(): Promise<void> {
    if (this.getPlayerState() !== PlayerState.Loaded) {
      throw new Error("Cannot resume: no content loaded.");
    }
    return this.videoElement.play();
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
      throw new Error("The Player is not initialized or is disposed.");
    }
    if (this.__contentMetadata__ !== null) {
      requestStopForContent(this.__contentMetadata__, this.__worker__);
    }
  }

  /**
   * Update the wanted playback speed, `1` meaning "regular" playback, `2`
   * meaning two times faster `0.5` meaning playing at half the speed etc.
   *
   * @param {number} speed
   */
  public setSpeed(speed: number): void {
    // There's two ways in which we could do this:
    //
    //   - We could mutate directly the HTMLMediaElement's `playbackRate`
    //     attribute in the main thread, unless the content is currently
    //     rebuffering (in which case it is set when we exit rebuffering).
    //     The core logic then knows about it through the usual
    //     `MediaObservation`.
    //
    //     This has the advantage of being simpler, allowing direct feedback to
    //     the application, being persisted between contents and being able to
    //     update it even when no content is playing. We also don't need to
    //     introduce a supplementary option to update the speed at loading time.
    //
    //   - We could send the order to update the playback rate to the worker
    //     which then sends back the order to update the playback rate on the
    //     `HTMLMediaElement` on the main thread.
    //     The advantages here is that the worker knows about the playback rate
    //     change before it actually happens and the core logic totally controls
    //     the playback rate, which may lead to more predictable behavior in the
    //     core logic, which is inherently more complex.
    //
    // For now I went with the second solution
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
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

  /**
   * Returns the wanted playback speed, `1` meaning "regular" playback, `2`
   * meaning two times faster `0.5` meaning playing at half the speed etc.
   * @returns {number}
   */
  public getSpeed(): number {
    return this.__contentMetadata__?.wantedSpeed ?? 1;
  }

  /**
   * Returns the minimum position with a reachable segment currently in the
   * content.
   *
   * Returns `undefined` if unknown or if no content is loaded.
   * @returns {number|undefined} s
   */
  public getMinimumPosition(): number | undefined {
    return this.__contentMetadata__?.minimumPosition;
  }

  /**
   * Returns the maximum position with a reachable segment currently in the
   * content.
   *
   * Returns `undefined` if unknown or if no content is loaded.
   * @returns {number|undefined} s
   */
  public getMaximumPosition(): number | undefined {
    return this.__contentMetadata__?.maximumPosition;
  }

  /**
   * Returns `true` if the currently-loaded content is a still pending live
   * content whose new segment might still get generated.
   * On such content, the maximum and minimum positions are very likely to
   * evolve.
   *
   * Return `false` if the content is not a live content, if no content is
   * loaded or if it is unknown if the current content is a live content.
   *
   * @returns {boolean}
   */
  public isLive(): boolean {
    return this.__contentMetadata__?.playlistType === PlaylistNature.Live;
  }

  /**
   * Returns `true` if the currently-loaded content is completely finished
   * VoD content whose minimum and maximum positions is very unlikely to change.
   *
   * Return `false` if the content is not a VoD content, if no content is
   * loaded or if it is unknown if the current content is a VoD content.
   *
   * @returns {boolean}
   */
  public isVod(): boolean {
    return this.__contentMetadata__?.playlistType === PlaylistNature.VoD;
  }

  /**
   * Returns the Error that interrupted the playback of the current content or
   * `null` either if no Error arised or if no content is loaded.
   * @returns {Error|null}
   */
  public getError(): WaspError | null {
    return this.__contentMetadata__?.error ?? null;
  }

  /**
   * Returns the information on the currently loaded HLS variant.
   * Returns `undefined` if unknown or if no content is loaded.
   *
   * @returns {Object|undefined}
   */
  public getCurrentVariant(): VariantInfo | undefined {
    return this.__contentMetadata__?.currVariant ?? undefined;
  }

  /**
   * Returns a list of all available on HLS variants.
   * Returns an empty array if unknown or if no content is loaded.
   *
   * @returns {Array.<Object>}
   */
  public getVariantList(): VariantInfo[] {
    return this.__contentMetadata__?.variants ?? [];
  }

  /**
   * Returns a list of all available on audio tracks.
   * Returns an empty array if unknown or if no content is loaded.
   *
   * @returns {Array.<Object>}
   */
  public getAudioTrackList(): AudioTrackInfo[] {
    return this.__contentMetadata__?.audioTracks ?? [];
  }

  /**
   * Returns the information on the currently loaded audio track.
   * Returns `undefined` if unknown, if no content is loaded or if the content
   * has no audio track.
   *
   * @returns {Object|undefined}
   */
  public getCurrentAudioTrack(): AudioTrackInfo | undefined {
    const id = this.__contentMetadata__?.currentAudioTrack?.id;
    if (id === undefined) {
      return undefined;
    }
    return this.getAudioTrackList()?.find((a) => a.id === id);
  }

  /**
   * Set the wanted audio track through its `id` property or indicate that you
   * don't want to force an audio track (you want to rely on the content
   * default's ones) by communicating `null` instead.
   *
   * Note that if relying on `lockVariant`, updating the audio track might lead
   * to the automatic unlocking of the variant if and only if the wanted track
   * has no equivalent in the locked variant. In that case, you will receive a
   * `variantLockUpdate` event indicating that the lock is no more in place.
   *
   * @param {number|null} trackId - The value of the `id` property of the
   * track you want to set or `null` if you want to rely on the
   * content's default instead.
   */
  public setAudioTrack(trackId: number | null): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
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

  /**
   * Lock in place the wanted HLS variant through its `id` property.
   *
   * The `WaspHlsPlayer` will then only load media data linked to this variant
   * until either a new content is loaded, a track change has necessitated the
   * selection of another variant, or if `unlockVariant` was called.
   *
   * You will receive `variantLockUpdate` events both when the lock is
   * effectively put in place (unless it was already put in place for that same
   * variant initially), and when/if it is unlocked.
   *
   * @param {number} variantId - The value of the `id` property of the
   * variant you want to force.
   */
  public lockVariant(variantId: number): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
    }
    if (this.__contentMetadata__ === null) {
      throw new Error("No content loaded");
    }
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LockVariant,
      value: {
        contentId: this.__contentMetadata__.contentId,
        variantId,
      },
    });
  }

  /**
   * Remove a variant lock previously set through the `lockVariant` method.
   *
   * You will receive `variantLockUpdate` events when the lock is effectively
   * removed, unless it was already removed.
   */
  public unlockVariant(): void {
    if (this.__worker__ === null) {
      throw new Error("The Player is not initialized or is disposed.");
    }
    if (this.__contentMetadata__ === null) {
      throw new Error("No content loaded");
    }
    postMessageToWorker(this.__worker__, {
      type: MainMessageType.LockVariant,
      value: {
        contentId: this.__contentMetadata__.contentId,
        variantId: null,
      },
    });
  }

  /**
   * Get variant actively locked through the `lockVariant` method, or `null` if
   * no variant is locked.
   *
   * Note that this method might not be up-to-data immediately after the methods
   * `lockVariant` and `unlockVariant` are called as it returns the currenly
   * known situation on the worker-side. This is because communication of those
   * new settings are done asynchronously (they have to be communicated from the
   * main thread where this API lives to the Worker where the core logic
   * actually runs).
   *
   * @returns {Object|null}
   */
  public getLockedVariant(): VariantInfo | null {
    return this.__contentMetadata__?.lockedVariant ?? null;
  }

  /**
   * Free all resources taken by the `WaspHlsPlayer`.
   */
  public dispose(): void {
    this.__destroyAbortController__.abort();
    if (this.__worker__ === null) {
      return;
    }
    if (this.__contentMetadata__ !== null) {
      requestStopForContent(this.__contentMetadata__, this.__worker__);
    }
    // NOTE: is this still needed? What about GC once it is set to `null`?
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
  }

  /**
   * Initialize Worker logic and bind its events.
   * @param {Object} opts - Options given in the `initialize` call.
   * @param {Function} resolveProm - Callback to call once the initialization
   * succeeded.
   * @param {Function} resolveProm - Callback to call if the initialization
   * failed, with the corresponding error in argument.
   */
  private __startWorker__(
    opts: InitializationOptions,
    resolveProm: () => void,
    rejectProm: (err: WaspInitializationError) => void
  ): void {
    let mayStillReject = true;
    this.__worker__ = new Worker(opts.workerUrl);
    const worker = this.__worker__;
    postMessageToWorker(worker, {
      type: MainMessageType.Initialization,
      value: {
        hasMseInWorker:
          typeof MediaSource === "function" &&
          /* eslint-disable-next-line */
          (MediaSource as any).canConstructInDedicatedWorker === true,
        canDemuxMpeg2Ts: canDemuxMpeg2Ts(),
        wasmUrl: potentiallyRelativeUrlToAbsoluteUrl(opts.wasmUrl),
        initialBandwidth: opts.initialBandwidth ?? 0,
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
          break;
        case WorkerMessageType.CreateMediaSource:
          onCreateMediaSourceMessage(
            data,
            this.__contentMetadata__,
            this.videoElement,
            worker
          );
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
          onEndOfStreamMessage(data, this.__contentMetadata__);
          break;
        case WorkerMessageType.MediaOffsetUpdate:
          onMediaOffsetUpdateMessage(data, this.__contentMetadata__);
          break;
        case WorkerMessageType.MultivariantPlaylistParsed:
          if (
            onMultivariantPlaylistParsedMessage(data, this.__contentMetadata__)
          ) {
            this.trigger("variantListUpdate", this.getVariantList());
            this.trigger("audioTrackListUpdate", this.getAudioTrackList());
          }
          break;
        case WorkerMessageType.TrackUpdate:
          if (onTrackUpdateMessage(data, this.__contentMetadata__)) {
            if (data.value.mediaType === MediaType.Audio) {
              this.trigger("audioTrackUpdate", this.getCurrentAudioTrack());
            }
          }
          break;
        case WorkerMessageType.VariantUpdate:
          if (onVariantUpdateMessage(data, this.__contentMetadata__)) {
            this.trigger("variantUpdate", this.getCurrentVariant());
          }
          break;

        case WorkerMessageType.VariantLockStatusChange:
          if (
            onVariantLockStatusChangeMessage(data, this.__contentMetadata__)
          ) {
            this.trigger("variantLockUpdate", this.getLockedVariant());
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
            logger.warn("API: Triggering warning", error);
            this.trigger("warning", error);
          }
          break;
        }
        case WorkerMessageType.ContentInfoUpdate:
          if (onContentInfoUpdateMessage(data, this.__contentMetadata__)) {
            this.trigger("contentInfoUpdate", {
              minimumPosition: this.getMinimumPosition(),
              maximumPosition: this.getMaximumPosition(),
              isLive: this.isLive(),
              isVod: this.isVod(),
            });
          }
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
            if (this.getPlayerState() === PlayerState.Loaded) {
              this.trigger("rebufferingStarted", null);
            }
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
            if (this.getPlayerState() === PlayerState.Loaded) {
              this.trigger("rebufferingEnded", null);
            } else if (this.__contentMetadata__?.loadingAborter !== undefined) {
              // If still loading, send loaded event as soon as the
              // `HTMLMediaElement` say we can (it should generally be directly
              // as the Worker has more drastic conditions)
              waitForLoad(
                this.videoElement,
                this.__contentMetadata__.loadingAborter.signal
              ).then(
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
                  const err =
                    reason instanceof Error ? reason : "Unknown reason";
                  logger.info("Could not load content:", err);
                }
              );
            }
          }
          break;

        case WorkerMessageType.AreTypesSupported:
          onAreTypesSupportedMessage(data, worker);
          break;

        default:
          assertNever(data);
      }
    };

    worker.onerror = (ev: ErrorEvent) => {
      const error = ev.error instanceof Error ? ev.error : "Unknown Error";
      logger.error("API: Worker Error encountered", error);
      if (mayStillReject) {
        rejectProm(
          new WaspInitializationError(
            InitializationErrorCode.UnknownError,
            undefined,
            /* eslint-disable @typescript-eslint/no-unsafe-member-access*/
            /* eslint-disable @typescript-eslint/no-unsafe-argument */
            ev.error?.message ?? undefined
            /* eslint-enable @typescript-eslint/no-unsafe-member-access*/
            /* eslint-enable @typescript-eslint/no-unsafe-argument */
          )
        );
      }
      this.dispose();
    };

    /**
     * Update the logger level on the Worker side.
     * @param {number} level
     */
    function onLogLevelChange(level: LoggerLevel): void {
      postMessageToWorker(worker, {
        type: MainMessageType.UpdateLoggerLevel,
        value: level,
      });
    }
  }
}

/** Options that can be given to a `load` call`. */
export interface LoadOptions {
  /**
   * Optional position to start at.
   *
   * Either as a position directly in playlist time in seconds, or as an object
   * allowing to give relative position to a given base.
   */
  startingPosition?: StartingPosition | number | undefined;
}

/**
 * Format of the more complex way to express a starting position.
 */
export type StartingPosition =
  | AbsoluteStartingPosition
  | FromBeginningStartingPosition
  | FromEndStartingPosition;

/**
 * Object to set when wanting an absolute position, in terms of playlist time in
 * seconds.
 */
interface AbsoluteStartingPosition {
  startType: "Absolute";
  position: number;
}

/**
 * Object to set when wanting a position in seconds relative to the initial
 * minimum position of the content.
 */
interface FromBeginningStartingPosition {
  startType: "FromBeginning";
  position: number;
}

/**
 * Object to set when wanting a position in seconds relative to the initial
 * maximum position of the content.
 * Here a higher `position` will mean further BEFORE the maximum position, and
 * thus an earlier position.
 */
interface FromEndStartingPosition {
  startType: "FromEnd";
  position: number;
}
