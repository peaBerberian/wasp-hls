import {
  MediaSourceReadyState,
  MediaType,
  OtherErrorCode,
  PlaybackTickReason,
  PlaylistType,
  RequestErrorReason,
  SourceBufferCreationErrorCode as WasmSourceBufferCreationErrorCode,
} from "../wasm/wasp_hls";
import { LoggerLevel } from "./logger";

export { MediaSourceReadyState, PlaybackTickReason };

/** Message sent from the main thread to the worker. */
export type MainMessage =
  | InitializationMainMessage
  | LoadContentMainMessage
  | StopContentMainMessage
  | DisposePlayerMainMessage
  | MediaSourceStateChangedMainMessage
  | CreateMediaSourceErrorMainMessage
  | SetMediaSourceDurationErrorMainMessage
  | CreateSourceBufferErrorMainMessage
  | MediaObservationMainMessage
  | SourceBufferOperationErrorMainMessage
  | SourceBufferOperationSuccessMainMessage
  | EndOfStreamErrorMainMessage
  | CodecsSupportUpdateMainMessage
  | UpdateWantedSpeedMainMessage
  | UpdateLoggerLevelMainMessage
  | LockVariantMainMessage
  | UpdateConfigMainMessage
  | SetAudioTrackMainMessage;

/**
 * Discriminants (value of the `type` property) for messages sent by the main
 * thread.
 *
 * Centralized here for easier type hinting, search, refactoring and format
 * updates.
 */
export const enum MainMessageType {
  Initialization = "init",
  LoadContent = "load",
  StopContent = "stop",
  DisposePlayer = "dispose",
  MediaSourceStateChanged = "ms-state",
  CreateMediaSourceError = "create-ms",
  CreateSourceBufferError = "create-sb",
  UpdateMediaSourceDurationError = "update-ms-dur-err",
  MediaObservation = "obs",
  SourceBufferOperationSuccess = "sb-s",
  SourceBufferOperationError = "sb-err",
  EndOfStreamError = "eos-err",
  UpdateWantedSpeed = "upd-speed",
  UpdateLoggerLevel = "upd-log",
  UpdateConfig = "upd-conf",
  LockVariant = "lock-var",
  SetAudioTrack = "set-audio",
  CodecsSupportUpdate = "codecs-support-upd",
}

/** Message sent from the worker to the main thread. */
export type WorkerMessage =
  // Related to WebWorker initialization

  | InitializedWorkerMessage
  | InitializationErrorWorkerMessage

  // Related to content information
  | MultiVariantPlaylistParsedWorkerMessage
  | ContentTimeBoundsUpdateWorkerMessage
  | MediaOffsetUpdateWorkerMessage
  | VariantUpdateWorkerMessage
  | TrackUpdateWorkerMessage
  | VariantLockStatusChangeWorkerMessage

  // HTMLMediaElement/MSE actions
  | SeekWorkerMessage
  | FlushWorkerMessage
  | UpdatePlaybackRateWorkerMessage
  | AttachMediaSourceWorkerMessage
  | CreateMediaSourceWorkerMessage
  | SetMediaSourceDurationWorkerMessage
  | ClearMediaSourceWorkerMessage
  | CreateSourceBufferWorkerMessage
  | AppendBufferWorkerMessage
  | RemoveBufferWorkerMessage
  | EndOfStreamWorkerMessage
  | AreTypesSupportedWorkerMessage

  // Playback conditions
  | StartPlaybackObservationWorkerMessage
  | StopPlaybackObservationWorkerMessage
  | RebufferingStartedWorkerMessage
  | RebufferingEndedWorkerMessage

  // Misc
  | ErrorWorkerMessage
  | WarningWorkerMessage
  | ContentStoppedWorkerMessage;

/**
 * Discriminants (value of the `type` property) for messages sent by the worker.
 *
 * Centralized here for easier type hinting, search, refactoring and format
 * updates.
 */
export const enum WorkerMessageType {
  Initialized = "init",
  InitializationError = "init-err",
  Error = "err",
  Warning = "warn",
  ContentTimeBoundsUpdate = "time-upd",
  MultiVariantPlaylistParsed = "m-playlist",
  TrackUpdate = "track-upd",
  ContentStopped = "ctnt-stop",
  Seek = "seek",
  Flush = "flush",
  UpdatePlaybackRate = "upd-pbr",
  AttachMediaSource = "attach-ms",
  CreateMediaSource = "create-ms",
  UpdateMediaSourceDuration = "upd-ms-dur",
  ClearMediaSource = "clear-ms",
  CreateSourceBuffer = "creat-sb",
  AppendBuffer = "push-sb",
  RemoveBuffer = "rem-sb",
  StartPlaybackObservation = "start-obs",
  StopPlaybackObservation = "stop-obs",
  EndOfStream = "eos",
  RebufferingStarted = "rebuf-start",
  RebufferingEnded = "rebuf-end",
  MediaOffsetUpdate = "media-off-upd",
  VariantUpdate = "variant-upd",
  VariantLockStatusChange = "variant-lck-upd",
  AreTypesSupported = "are-types-supp",
}

/**
 * Error codes generated for `InitializationErrorWorkerMessage` messages.
 */
export const enum InitializationErrorCode {
  /**
   * The corresponding worker received a `InitializationMainMessage` despite
   * already being initialized.
   */
  AlreadyInitializedError,
  /**
   * The corresponding worker did not succeed to load the WebAssembly part of
   * the WaspHlsPlayer due to the impossibility of requesting it.
   */
  WasmRequestError,
  // /**
  //  * The corresponding worker did not succeed to load the WebAssembly part of
  //  * the WaspHlsPlayer due to an HTTP response not in the 200s.
  //  */
  // WasmRequestBadStatus,
  /**
   * The corresponding worker did not succeed to load the WebAssembly part of
   * the WaspHlsPlayer due to a timeout during its HTTP request.
   */
  WasmRequestTimeout,
  /** Any other, uncategorized, error. */
  UnknownError,
}

/**
 * Message sent when the Worker has loaded the WASM code and everything is ready
 * to begin loading content.
 */
export interface InitializedWorkerMessage {
  type: WorkerMessageType.Initialized;
  value: null;
}

/**
 * Message sent when the Worker has encountered a global error and may
 * consequently not be able to operate anymore.
 */
export interface InitializationErrorWorkerMessage {
  type: WorkerMessageType.InitializationError;
  value: {
    /**
     * Code describing the error encountered.
     */
    code: InitializationErrorCode;
    /**
     * If set, human-readable string describing the error, for debugging
     * purposes.
     */
    message?: string | undefined;
    wasmHttpStatus?: number | undefined;
  };
}

/**
 * Message sent when the Worker has encountered a major error which provoked the
 * end of the current content.
 */
export interface ErrorWorkerMessage {
  type: WorkerMessageType.Error;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     *
     * Unset if the error is not linked to a content or if no content
     * are loaded yet.
     */
    contentId: string;

    errorInfo:
      | UnitializedErrorWorkerInfo
      | PlaylistParsingErrorWorkerInfo
      | PlaylistRequestErrorWorkerInfo
      | SegmentRequestErrorWorkerInfo
      | SourceBufferCreationErrorWorkerInfo
      | OtherErrorWorkerInfo;

    /**
     * If set, human-readable string describing the error, for debugging
     * purposes.
     */
    message?: string | undefined;
  };
}

/**
 * Message sent when the Worker has encountered a minor error.
 * If a content was playing, we're still able to continue playback.
 */
export interface WarningWorkerMessage {
  type: WorkerMessageType.Warning;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     *
     * Unset if the error is not linked to a content or if no content
     * are loaded yet.
     */
    contentId?: string;

    errorInfo:
      | UnitializedErrorWorkerInfo
      | PlaylistParsingErrorWorkerInfo
      | PlaylistRequestErrorWorkerInfo
      | SegmentRequestErrorWorkerInfo
      | SourceBufferCreationErrorWorkerInfo
      | OtherErrorWorkerInfo;

    /**
     * If set, human-readable string describing the error, for debugging
     * purposes.
     */
    message?: string | undefined;
  };
}

/**
 * Error sent when the worker received an order despite not being initialized.
 */
export interface UnitializedErrorWorkerInfo {
  type: "unitialized";
  value: {
    call?: string;
  };
}

/**
 * Error linked to an error while parsing a MultiVariant playlist or media
 * playlist.
 */
export interface PlaylistParsingErrorWorkerInfo {
  type: "playlist-parse";
  value: {
    type: PlaylistType;
    mediaType?: MediaType | undefined;
  };
}

/**
 * Error linked to a MultiVariant playlist or media playlist HTTP(S) request's
 * failure.
 */
export interface PlaylistRequestErrorWorkerInfo {
  type: "playlist-request";
  value: {
    type: PlaylistType;
    url: string;
    mediaType?: MediaType;
    reason: RequestErrorReason;
    status?: number;
  };
}

/**
 * Error linked to a segment HTTP(S) request's failure.
 */
export interface SegmentRequestErrorWorkerInfo {
  type: "segment-request";
  value: {
    url: string;
    isInit: boolean;
    start?: number | undefined;
    duration?: number | undefined;
    mediaType: MediaType;
    byteRange?: [number, number] | undefined;
    reason: RequestErrorReason;
    status?: number | undefined;
  };
}

export interface SourceBufferCreationErrorWorkerInfo {
  type: "source-buffer-creation-error";
  value: {
    code: WasmSourceBufferCreationErrorCode;
  };
}

/** Errors that do not corresponds to other Error categorizations. */
export interface OtherErrorWorkerInfo {
  type: "other-error";
  value: {
    code: OtherErrorCode;
  };
}

/**
 * Message sent when the Worker has new information on the content being played.
 */
export interface ContentTimeBoundsUpdateWorkerMessage {
  type: WorkerMessageType.ContentTimeBoundsUpdate;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    /**
     * Current minimum position, in playlist time and in seconds, for which
     * segments are declared in the playlist.
     */
    minimumPosition: number | undefined;
    /**
     * Current maximum position, in playlist time and in seconds, for which
     * segments are declared in the playlist.
     */
    maximumPosition: number | undefined;
  };
}

export interface MultiVariantPlaylistParsedWorkerMessage {
  type: WorkerMessageType.MultiVariantPlaylistParsed;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    variants: VariantInfo[];
    audioTracks: AudioTrackInfo[];
  };
}

export interface TrackUpdateWorkerMessage {
  type: WorkerMessageType.TrackUpdate;
  value: {
    /**
     * The identifier for the content for which the message was sent.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    mediaType: MediaType;
    audioTrack?:
      | {
          current: number;
          isSelected: boolean;
        }
      | undefined;
  };
}

export interface VariantLockStatusChangeWorkerMessage {
  type: WorkerMessageType.VariantLockStatusChange;
  value: {
    /**
     * The identifier for the content for which the message was sent.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    lockedVariant: number | null;
  };
}

export interface VariantInfo {
  id: number;
  width: number;
  height: number;
  frameRate: number;
  bandwidth: number;
}

export interface AudioTrackInfo {
  id: number;
  language?: string | undefined;
  assocLanguage?: string | undefined;
  name: string;
  channels?: number | undefined;
}

/**
 * Message sent when the Worker has succesfully stopped a content.
 */
export interface ContentStoppedWorkerMessage {
  type: WorkerMessageType.ContentStopped;
  value: {
    /** The identifier for the content which was stopped. */
    contentId: string;
  };
}

/** Message sent when the Worker want to seek in the content */
export interface SeekWorkerMessage {
  type: WorkerMessageType.Seek;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only seek if the same MediaSource is still being
     * used.
     */
    mediaSourceId: string;
    /**
     * The position in seconds at which the worker wants to seek to in seconds
     * to put on the HTMLMediaElement's `currentTime` property.
     */
    position: number;
  };
}

/** Message sent when the Worker want to flush the media element */
export interface FlushWorkerMessage {
  type: WorkerMessageType.Flush;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only seek if the same MediaSource is still being
     * used.
     */
    mediaSourceId: string;
  };
}

/** Message sent when the Worker wants to change the media element's playback rate. */
export interface UpdatePlaybackRateWorkerMessage {
  type: WorkerMessageType.UpdatePlaybackRate;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only change the playback rate if the same
     * MediaSource is still being used.
     */
    mediaSourceId: string;
    /**
     * The position in seconds at which the worker wants to seek to in seconds
     * to put on the HTMLMediaElement's `currentTime` property.
     */
    playbackRate: number;
  };
}

/**
 * Sent when the Worker created a MediaSource itself and want to attach it to
 * the HTMLVideoElement.
 *
 * A worker either send the `AttachMediaSourceWorkerMessage` or the
 * `CreateMediaSourceWorkerMessage` for MediaSource attachment, depending on if
 * a MediaSource instance is accessible from a Worker.
 */
export interface AttachMediaSourceWorkerMessage {
  type: WorkerMessageType.AttachMediaSource;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    /**
     * The `MediaSource`'s handle to attach to the HTMLMediaElement.
     * Can be `undefined` in wich case a `src is provided instead.
     */
    handle: MediaProvider | undefined;
    /**
     * The `MediaSource`'s local URL to link to the HTMLMediaElement.
     * Can be `undefined` in wich case a `handle is provided instead.
     */
    src: string | undefined;
    /**
     * Identify the corresponding MediaSource created by the WebWorker.
     * The main thread should keep that value for ensuring that future messages
     * do concern that MediaSource.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the Worker wants to create a MediaSource on the main thread and
 * want it to be attached to the HTMLVideoElement.
 *
 * A worker either send the `AttachMediaSourceWorkerMessage` or the
 * `CreateMediaSourceWorkerMessage` for MediaSource attachment, depending on if
 * a MediaSource instance is accessible from a Worker.
 */
export interface CreateMediaSourceWorkerMessage {
  type: WorkerMessageType.CreateMediaSource;
  value: {
    /**
     * The identifier for the content on which an error was received.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    /**
     * Identify the corresponding MediaSource to create.
     * The main thread should keep that value for ensuring that future messages
     * do concern that MediaSource.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the Worker wants to update the `duration` property of the
 * MediaSource associated to the `mediaSourceId` given.
 */
export interface SetMediaSourceDurationWorkerMessage {
  type: WorkerMessageType.UpdateMediaSourceDuration;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only change the duration if the same MediaSource
     * is still being used.
     */
    mediaSourceId: string;
    /** The new `duration` to set on the  `MediaSource`, in seconds. */
    duration: number;
  };
}

/**
 * Sent when the MediaSource linked to the given `mediaSourceId` should be
 * disposed from the HTMLVideoElement if it was, and all of its associated
 * resources disposed.
 */
export interface ClearMediaSourceWorkerMessage {
  type: WorkerMessageType.ClearMediaSource;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only clear the MediaSource if it is
     * still the one being used.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the Worker wants to create a SourceBuffer on the main thread and
 * want it to be attached to the MediaSource linked to the given
 * `mediaSourceId`.
 */
export interface CreateSourceBufferWorkerMessage {
  type: WorkerMessageType.CreateSourceBuffer;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only create a SourceBuffer if it is still the
     * MediaSource being used.
     */
    mediaSourceId: string;
    /**
     * Id uniquely identifying this SourceBuffer.
     * It is generated from the Worker and it is unique for all SourceBuffers
     * created after associated with the `mediaSourceId`.
     */
    sourceBufferId: number;
    /**
     * "Content-Type" associated to the SourceBuffer, that may have to be used
     * when initializing the latter.
     */
    contentType: string;
  };
}

/**
 * Sent when the Worker wants to append binary data on the SourceBuffer
 * corresponding to the `sourceBufferId` given.
 *
 * Note that the worker does not take into account the potential queue that
 * should be awaited to perform such operations.
 * As such, any queue mechanism associated to this message should be performed
 * in the main thread.
 */
export interface AppendBufferWorkerMessage {
  type: WorkerMessageType.AppendBuffer;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only push data if it is still the MediaSource
     * being used.
     */
    mediaSourceId: string;
    /**
     * Id uniquely identifying this SourceBuffer.
     * It should be the same `sourceBufferId` than the one on the
     * `CreateSourceBufferWorkerMessage`.
     */
    sourceBufferId: number;
    /** Raw data to append to the SourceBuffer. */
    data: BufferSource;
  };
}

/**
 * Sent when the Worker wants to remove data from the SourceBuffer
 * corresponding to the `sourceBufferId` given.
 *
 * Note that the worker does not take into account the potential queue that
 * should be awaited to perform such operations.
 * As such, any queue mechanism associated to this message should be performed
 * in the main thread.
 */
export interface RemoveBufferWorkerMessage {
  type: WorkerMessageType.RemoveBuffer;
  value: {
    /**
     * Identify the MediaSource currently used by the worker.
     * The main thread should only remove buffer if it is still the MediaSource
     * being used.
     */
    mediaSourceId: string;
    /**
     * Id uniquely identifying this SourceBuffer.
     * It should be the same `sourceBufferId` than the one on the
     * `CreateSourceBufferWorkerMessage`.
     */
    sourceBufferId: number;
    /** Range start, in seconds, of the data that should be removed. */
    start: number;
    /** Range end, in seconds, of the data that should be removed. */
    end: number;
  };
}

/**
 * Sent when the worker wants to start receiving regularly "playback
 * observations", which are key attributes associated to the HTMLVideoElement.
 */
export interface StartPlaybackObservationWorkerMessage {
  type: WorkerMessageType.StartPlaybackObservation;
  value: {
    /**
     * Playback observations are linked to an unique MediaSource.
     *
     * This `mediaSourceId` should be the same `mediaSourceId` than the one on
     * the `CreateMediaSourceWorkerMessage` for it.
     *
     * Adding such identifier to this seemlingly unrelated event allows to
     * protect against potential race conditions.
     *
     * If `mediaSourceId` don't match, the message should be ignored.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the worker wants to stop receiving regularly "playback
 * observations", previously started through a
 * `StartPlaybackObservationWorkerMessage`
 */
export interface StopPlaybackObservationWorkerMessage {
  type: WorkerMessageType.StopPlaybackObservation;
  value: {
    /**
     * Playback observations are linked to an unique MediaSource.
     *
     * This `mediaSourceId` should be the same `mediaSourceId` than the one on
     * the `CreateMediaSourceWorkerMessage` for it.
     *
     * Adding such identifier to this seemlingly unrelated event allows to
     * protect against potential race conditions.
     *
     * If `mediaSourceId` don't match, the message should be ignored.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the worker wants to call the `endOfStream` method of the
 * `MediaSource`, thus ending the stream.
 */
export interface EndOfStreamWorkerMessage {
  type: WorkerMessageType.EndOfStream;
  value: {
    /**
     * This `mediaSourceId` should be the same `mediaSourceId` than the one on
     * the `CreateMediaSourceWorkerMessage` for it.
     *
     * Adding such identifier to this seemlingly unrelated event allows to
     * protect against potential race conditions.
     *
     * If `mediaSourceId` don't match, the message should be ignored.
     */
    mediaSourceId: string;
  };
}

/**
 * Sent when the worker wants to call the `MediaSource.isTypeSupported` static
 * method to check codec support.
 *
 * A `CodecsSupportUpdateMainMessage` is then expected in response from the main
 * thread.
 */
export interface AreTypesSupportedWorkerMessage {
  type: WorkerMessageType.AreTypesSupported;
  value: {
    mimeTypes: string[];
  };
}

/**
 * Sent when the worker wants to enter a rebuffering period, to build back
 * buffer.
 */
export interface RebufferingStartedWorkerMessage {
  type: WorkerMessageType.RebufferingStarted;
  value: {
    /**
     * This `mediaSourceId` should be the same `mediaSourceId` than the one on
     * the `CreateMediaSourceWorkerMessage` for it.
     *
     * Adding such identifier to this seemlingly unrelated event allows to
     * protect against potential race conditions.
     *
     * If `mediaSourceId` don't match, the message should be ignored.
     */
    mediaSourceId: string;

    /**
     * If `true`, the playback rate has to be set to `0` as long as rebuffering
     * is pending.
     */
    updatePlaybackRate: boolean;
  };
}

/**
 * Sent when the worker wants to exit a rebuffering period previously started
 * through a `RebufferingStartedWorkerMessage`.
 */
export interface RebufferingEndedWorkerMessage {
  type: WorkerMessageType.RebufferingEnded;
  value: {
    /**
     * This `mediaSourceId` should be the same `mediaSourceId` than the one on
     * the `CreateMediaSourceWorkerMessage` for it.
     *
     * Adding such identifier to this seemlingly unrelated event allows to
     * protect against potential race conditions.
     *
     * If `mediaSourceId` don't match, the message should be ignored.
     */
    mediaSourceId: string;
  };
}

/**
 * Message sent when the Worker has updated its offset to convert playlist time,
 * as anounced in the MediaPlaylist (and which should be preferred for a user
 * interface) into media time, which is the time actually present on the
 * HTMLMediaElement.
 */
export interface MediaOffsetUpdateWorkerMessage {
  type: WorkerMessageType.MediaOffsetUpdate;
  value: {
    /**
     * A unique identifier for the content being loaded, that will have to be
     * present on the various events concerning that content.
     */
    contentId: string;
    /**
     * Offset that can be added to the playlist time to obtain the time on the
     * `HTMLMediaElement` and vice-versa, in seconds.
     */
    offset: number;
  };
}

export interface VariantUpdateWorkerMessage {
  type: WorkerMessageType.VariantUpdate;
  value: {
    /**
     * A unique identifier for the content being loaded, that will have to be
     * present on the various events concerning that content.
     */
    contentId: string;
    variantId: number | undefined;
  };
}

/**
 * First message sent by the main thread to a worker, to initialize it.
 * Once a worker has been initialized, it should send back an
 * `InitializedWorkerMessage`.
 */
export interface InitializationMainMessage {
  type: MainMessageType.Initialization;
  value: {
    /**
     * If `true` the current browser has the MSE-in-worker feature.
     * `false` otherwise.
     */
    hasMseInWorker: boolean;

    /**
     * If `true`, mpeg2 transport stream can be used a segment container on the
     * current environment.
     *
     * If `false`, they have to be transmuxed first.
     */
    canDemuxMpeg2Ts: boolean;

    /** Url to the WASM part of the WaspHlsPlayer */
    wasmUrl: string;

    /** The initial logger level to set. */
    logLevel: LoggerLevel;

    /** Initial configuration for the player. */
    initialConfig: WaspHlsPlayerConfig;
  };
}

/**
 * Sent by the main thread to the worker when a new content should be loaded.
 */
export interface LoadContentMainMessage {
  type: MainMessageType.LoadContent;
  value: {
    /**
     * A unique identifier for the content being loaded, that will have to be
     * present on the various events concerning that content.
     */
    contentId: string;
    /** URL to the HLS MultiVariant Playlist. */
    url: string;
  };
}

/**
 * Sent by the main thread to the worker when the last loaded content (through
 * a `LoadContentMainMessage`) should be stopped and all its resources disposed.
 */
export interface StopContentMainMessage {
  type: MainMessageType.StopContent;
  value: {
    /**
     * The identifier for the content that should be stopped.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
  };
}

/**
 * Sent by the main thread to a worker that will not be needed anymore.
 * It is expected that a Worker free all its resources when this message is
 * sent.
 */
export interface DisposePlayerMainMessage {
  type: MainMessageType.DisposePlayer;
  value: null;
}

/**
 * Sent by the main thread to a Worker when the MediaSource linked to the
 * `mediaSourceId` changed its readyState.
 *
 * This message is only sent if the MediaSource is created on the main thread.
 */
export interface MediaSourceStateChangedMainMessage {
  type: MainMessageType.MediaSourceStateChanged;
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: string;
    /** The new state of the MediaSource. */
    state: MediaSourceReadyState;
  };
}

/**
 * Sent by the main thread to a Worker when the creation of a MediaSource, due
 * to a previously-received `CreateMediaSourceWorkerMessage`, failed.
 */
export interface CreateMediaSourceErrorMainMessage {
  type: MainMessageType.CreateMediaSourceError;
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: string;
    /** The error's message. */
    message: string;
    /** The error's name. */
    name?: string | undefined;
  };
}

/** Codes that should be sent alongside a `CreateSourceBufferErrorMainMessage`. */
export enum SourceBufferCreationErrorCode {
  /**
   * The given `mediaSourceId` was right but there was no MediaSource on the
   * main thread.
   *
   * This looks like the MediaSource has been created on the worker but the
   * SourceBuffer is asked to be created on the main thread, which is an error.
   */
  NoMediaSource,
  /**
   * An error arised when creating the SourceBuffer through the MediaSource.
   */
  AddSourceBufferError,
}

/**
 * Sent by the main thread to a Worker when the creation of a SourceBuffer, due
 * to a previously-received `CreateSourceBufferWorkerMessage`, failed.
 */
export interface CreateSourceBufferErrorMainMessage {
  type: MainMessageType.CreateSourceBufferError;
  value: {
    mediaSourceId: string;
    /** Identify the SourceBuffer in question. */
    sourceBufferId: number;
    /** Error code to better specify the error encountered. */
    code: SourceBufferCreationErrorCode;
    /** The error's message. */
    message: string;
    /** The error's name. */
    name?: string | undefined;
  };
}

/**
 * Sent by the main thread to a Worker when the update of a MediaSource's
 * duration, due to a previously-received `SetMediaSourceDurationWorkerMessage`,
 * failed.
 */
export interface SetMediaSourceDurationErrorMainMessage {
  type: MainMessageType.UpdateMediaSourceDurationError;
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: string;
    /** The error's message. */
    message: string;
    /** The error's name. */
    name?: string | undefined;
  };
}

export interface MediaObservationMainMessage {
  type: MainMessageType.MediaObservation;
  value: MediaObservation;
}

export interface MediaObservation {
  /** Identify the MediaSource in question. */
  mediaSourceId: string;

  reason: PlaybackTickReason;
  currentTime: number;
  readyState: number;
  buffered: Float64Array;
  paused: boolean;
  seeking: boolean;
  ended: boolean;
  duration: number;
}

/**
 * Sent when the SourceBuffer linked to the given `mediaSourceId` and
 * `SourceBufferId`, running on the main thread, succeeded to perform the last
 * operation given to it (either through an `AppendBufferWorkerMessage` or a
 * `RemoveBufferWorkerMessage`).
 */
export interface SourceBufferOperationSuccessMainMessage {
  type: MainMessageType.SourceBufferOperationSuccess;
  value: {
    /**
     * Identify the MediaSource which contains the SourceBuffer concerned by
     * this update.
     */
    mediaSourceId: string;
    /**
     * Id uniquely identifying this SourceBuffer.
     * It should be the same `sourceBufferId` than the one on the
     * `CreateSourceBufferWorkerMessage`.
     */
    sourceBufferId: number;
  };
}

/**
 * Sent by the main thread to a Worker when the last operation performed on a
 * SourceBuffer either an "append" operation, provoked by a
 * `AppendBufferWorkerMessage` or a "remove" operation, provoked by a
 * `RemoveBufferWorkerMessage`.
 */
export interface SourceBufferOperationErrorMainMessage {
  type: MainMessageType.SourceBufferOperationError;
  value: {
    /** Identify the SourceBuffer in question. */
    sourceBufferId: number;
    /** The error's message. */
    message: string;
    /** The error's name. */
    name?: string | undefined;
  };
}

/** Codes that should be sent alongside a `EndOfStreamErrorMainMessage`. */
export enum EndOfStreamErrorCode {
  /**
   * The given `mediaSourceId` was right but there was no MediaSource on the
   * main thread.
   *
   * This looks like the MediaSource has been created on the worker but the
   * the worker wants to call `endOfStream` on the main thread, which is an
   * error.
   */
  NoMediaSource,
  /** An error arised when calling `endOfStream` on the MediaSource. */
  EndOfStreamError,
}

/**
 * Sent by the main thread to a Worker when the creation of a SourceBuffer, due
 * to a previously-received `CreateSourceBufferWorkerMessage`, failed.
 */
export interface EndOfStreamErrorMainMessage {
  type: MainMessageType.EndOfStreamError;
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: string;
    /** Error code to better specify the error encountered. */
    code: EndOfStreamErrorCode;
    /** The error's message. */
    message: string;
    /** The error's name. */
    name?: string | undefined;
  };
}

/**
 * Sent by the main thread to a Worker to report about codec support, generally
 * (but not obligatorily) in response to a `AreTypesSupportedWorkerMessage`
 * worker message.
 */
export interface CodecsSupportUpdateMainMessage {
  type: MainMessageType.CodecsSupportUpdate;
  value: {
    mimeTypes: Partial<Record<string, boolean>>;
  };
}

/**
 * Sent by the main thread to a Worker when the user wanted to update the
 * playback rate.
 */
export interface UpdateWantedSpeedMainMessage {
  type: MainMessageType.UpdateWantedSpeed;
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: string;
    /** The wanted speed in question. */
    wantedSpeed: number;
  };
}

export interface UpdateLoggerLevelMainMessage {
  type: MainMessageType.UpdateLoggerLevel;
  value: LoggerLevel;
}

export interface UpdateConfigMainMessage {
  type: MainMessageType.UpdateConfig;
  value: Partial<WaspHlsPlayerConfig>;
}

export interface LockVariantMainMessage {
  type: MainMessageType.LockVariant;
  value: {
    /**
     * The identifier for the content that should be stopped.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    variantId: number | null;
  };
}

export interface SetAudioTrackMainMessage {
  type: MainMessageType.SetAudioTrack;
  value: {
    /**
     * The identifier for the content that should be stopped.
     * This is the same `contentId` value that on the related
     * `LoadContentMainMessage`.
     */
    contentId: string;
    trackId: number | null;
  };
}

export interface WaspHlsPlayerConfig {
  bufferGoal: number;

  // Request options

  segmentRequestTimeout: number | null | undefined;
  segmentBackoffBase: number;
  segmentBackoffMax: number;
  multiVariantPlaylistRequestTimeout: number | null | undefined;
  multiVariantPlaylistBackoffBase: number;
  multiVariantPlaylistBackoffMax: number;
  mediaPlaylistRequestTimeout: number;
  mediaPlaylistBackoffBase: number;
  mediaPlaylistBackoffMax: number;
}
