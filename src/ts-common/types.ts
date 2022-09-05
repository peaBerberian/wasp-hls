import {
  MediaSourceReadyState,
  PlaybackTickReason,
} from "../wasm/wasp_hls";

export {
  MediaSourceReadyState,
  PlaybackTickReason,
};

/** Message sent from the main thread to the worker. */
export type MainMessage = InitializationMainMessage |
                          LoadContentMainMessage |
                          StopContentMainMessage |
                          DisposePlayerMainMessage |
                          MediaSourceStateChangedMainMessage |
                          MediaObservationMainMessage |
                          SourceBufferOperationSuccess;

/** Message sent from the worker to the main thread. */
export type WorkerMessage =
  InitializedWorkerMessage |
  SeekWorkerMessage |
  ErrorWorkerMessage |
  AttachMediaSourceWorkerMessage |
  CreateMediaSourceWorkerMessage |
  SetMediaSourceDurationWorkerMessage |
  ClearMediaSourceWorkerMessage |
  CreateSourceBufferWorkerMessage |
  AppendBufferWorkerMessage |
  RemoveBufferWorkerMessage |
  EndOfStreamWorkerMessage |
  StartPlaybackObservationWorkerMessage |
  StopPlaybackObservationWorkerMessage;

export const enum WorkerErrorCode {
  AlreadyInitializedError,
  WasmInitializationError,
  UnitializedLoadError,
  UnitializedStopError,
}

/**
 * Message sent when the Worker has loaded the WASM code and everything is ready
 * to begin loading content.
 */
export interface InitializedWorkerMessage {
  type: "initialized";
  value: null;
}

// TODO content or global worker error?
export interface ErrorWorkerMessage {
  type: "error";
  value: {
    code: WorkerErrorCode;
    message?: string | undefined;
  };
}

/** Message sent when the Worker want to seek in the content */
export interface SeekWorkerMessage {
  type: "seek";

  /** The position to seek to, in seconds. */
  value: number;
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
  type: "attach-media-source";
  value: {
    handle: MediaProvider | undefined;
    src: string | undefined;
    mediaSourceId: number;
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
  type: "create-media-source";
  value: {
    mediaSourceId: number;
  };
}

/**
 * Sent when the Worker wants to update the `duration` property of the
 * MediaSource associated to the `mediaSourceId` given.
 */
export interface SetMediaSourceDurationWorkerMessage {
  type: "update-media-source-duration";
  value: {
    mediaSourceId: number;
    duration: number;
  };
}

/**
 * Sent when the MediaSource linked to the given `mediaSourceId` should be
 * disposed from the HTMLVideoElement if it was, and all of its associated
 * resources disposed.
 */
export interface ClearMediaSourceWorkerMessage {
  type: "clear-media-source";
  value: {
    mediaSourceId: number;
  };
}

/**
 * Sent when the Worker wants to create a SourceBuffer on the main thread and
 * want it to be attached to the MediaSource linked to the given
 * `mediaSourceId`.
 */
export interface CreateSourceBufferWorkerMessage {
  type: "create-source-buffer";
  value: {
    mediaSourceId: number;
    sourceBufferId: number;
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
  type: "append-buffer";
  value: {
    mediaSourceId: number;
    sourceBufferId: number;
    data: ArrayBuffer;
  };
}

/**
 * Sent when the SourceBuffer linked to the given `mediaSourceId` and
 * `SourceBufferId`, running on the main thread, succeeded to perform the last
 * operation given to it (either through an `AppendBufferWorkerMessage` or a
 * `RemoveBufferWorkerMessage`).
 *
 * TODO should perhaps be removed altogether, as the main thread is doing the
 * queuing itself now.
 * For now, this event is needed as the player also awaits it in its current
 * `endOfStream` logic, which probably should be refactored.
 */
export interface SourceBufferOperationSuccess {
  type: "source-buffer-updated";
  value: {
    mediaSourceId: number;
    sourceBufferId: number;
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
  type: "remove-buffer";
  value: {
    mediaSourceId: number;
    sourceBufferId: number;
    start: number;
    end: number;
  };
}

/**
 * Sent when the worker wants to start receiving regularly "playback
 * observations", which are key attributes associated to the HTMLVideoElement.
 */
export interface StartPlaybackObservationWorkerMessage {
  type: "start-playback-observation";
  value: {
    mediaSourceId: number;
  };
}

/**
 * Sent when the worker wants to stop receiving regularly "playback
 * observations", previously started through a
 * `StartPlaybackObservationWorkerMessage`
 */
export interface StopPlaybackObservationWorkerMessage {
  type: "stop-playback-observation";
  value: {
    mediaSourceId: number;
  };
}

// TODO end of buffer instead? Might be much simpler to implement
export interface EndOfStreamWorkerMessage {
  type: "end-of-stream";
  value: {
    mediaSourceId: number;
  };
}

/**
 * First message sent by the main thread to a worker, to initialize it.
 * Once a worker has been initialized, it should send back an
 * `InitializedWorkerMessage`.
 */
export interface InitializationMainMessage {
  type: "init";
  value: {
    /**
     * If `true` the current browser has the MSE-in-worker feature.
     * `false` otherwise.
     */
    hasWorkerMse: boolean;

    /** Url to the WASM part of the WaspHlsPlayer */
    wasmUrl: string;
  };
}

/**
 * Sent by the main thread to the worker when a new content should be loaded.
 */
export interface LoadContentMainMessage {
  type: "load";
  value: {
    /** URL to the HLS MultiVariant Playlist. */
    url: string;
  };
}

/**
 * Sent by the main thread to the worker when the last loaded content (through
 * a `LoadContentMainMessage`) should be stopped and all its resources disposed.
 */
export interface StopContentMainMessage {
  type: "stop";
  value: null;
}

/**
 * Sent by the main thread to a worker that will not be needed anymore.
 * It is expected that a Worker free all its resources when this message is
 * sent.
 */
export interface DisposePlayerMainMessage {
  type: "dispose";
  value: null;
}

/**
 * Sent by the main thread to a Worker when the MediaSource linked to the
 * `mediaSourceId` changed its readyState.
 *
 * This message is only sent if the MediaSource is created on the main thread.
 */
export interface MediaSourceStateChangedMainMessage {
  type: "media-source-state-changed";
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: number;
    /** The new state of the MediaSource. */
    state: MediaSourceReadyState;
  };
}

export interface MediaObservationMainMessage {
  type: "observation";
  value: {
    /** Identify the MediaSource in question. */
    mediaSourceId: number;

    reason: PlaybackTickReason;
    currentTime: number;
    readyState: number;
    buffered: Float64Array;
    paused: boolean;
    seeking: boolean;
  };
}
