import QueuedSourceBuffer from "../ts-common/QueuedSourceBuffer";
import { AudioTrackInfo, VariantInfo } from "../ts-common/types";
import { WaspError } from "./errors";
import PlaybackObserver from "./observePlayback";

/**
 * Structure storing metadata associated to a content being played by a
 * `WaspHlsPlayer`.
 */
export interface ContentMetadata {
  /**
   * Unique identifier identifying the loaded content.
   *
   * This identifier should be unique for any instances of `WaspHlsPlayer`
   * created in the current JavaScript realm.
   *
   * Identifying a loaded content this way allows to ensure that messages
   * exchanged with a Worker concern the same content, mostly in cases of race
   * conditions.
   */
  contentId: string;

  /**
   * Unique identifier identifying a MediaSource attached to the
   * HTMLVideoElement.
   *
   * This identifier should be unique for the worker in question.
   *
   * Identifying a MediaSource this way allows to ensure that messages
   * exchanged with a Worker concern the same MediaSource instance.
   *
   * `null` when no `MediaSource` is created now.
   */
  mediaSourceId: string | null;

  /**
   * `MediaSource` instance linked to the current content being played.
   *
   * `null` when either:
   *   - no `MediaSource` instance is active for now
   *   - the `MediaSource` has been created on the Worker side.
   *
   * You can know whether a `MediaSource` is currently created at all by
   * refering to `mediaSourceId` instead.
   */
  mediaSource: MediaSource | null;

  /**
   * Callback that should be called when the `MediaSource` linked to the current
   * content becomes unattached - whether the `MediaSource` has been created in
   * this realm or in the worker.
   */
  disposeMediaSource: (() => void) | null;

  /**
   * Describe `SourceBuffer` instances currently associated to the current
   * `MediaSource` that have been created in this realm (and not in the Worker).
   */
  sourceBuffers: Array<{
    /**
     * Id uniquely identifying this SourceBuffer.
     * It is generated from the Worker and it is unique for all SourceBuffers
     * created after associated with the linked `mediaSourceId`.
     */
    sourceBufferId: number;
    /**
     * QueuedSourceBuffer associated to this SourceBuffers.
     * This is the abstraction used to push and remove data to the SourceBuffer.
     */
    queuedSourceBuffer: QueuedSourceBuffer;
  }>;

  /**
   * Class allowing to produce playback observations.
   * `null` if no "playback observation" is currently listened to.
   */
  playbackObserver: PlaybackObserver | null;

  /** If `true`, we are currently in a rebuffering period. */
  isRebuffering: boolean;

  /**
   * Offset allowing to convert from the position as announced by the media
   * element's `currentTime` property, to the actual content's position.
   *
   * To obtain the content's position from the `currentTime` property, just
   * remove `mediaOffset` (seconds) from the latter.
   *
   * To obtain the media element's time from a content's time, just add
   * `mediaOffset` to the latter.
   */
  mediaOffset: number | undefined;

  /**
   * The `playbackRate` wanted when not in a rebuffering phase.
   */
  wantedSpeed: number;

  /**
   * Minimum position in seconds with a reachable segment currently in the
   * content.
   * `undefined` if unknown.
   */
  minimumPosition: number | undefined;

  /**
   * Maximum position in seconds with a reachable segment currently in the
   * content.
   * `undefined` if unknown.
   */
  maximumPosition: number | undefined;

  /**
   * Information on the currently loaded HLS variant.
   * `undefined` if unknown.
   */
  currVariant: VariantInfo | undefined;

  /**
   * List of all available on HLS variants.
   * Empty array if unknown.
   */
  variants: VariantInfo[];

  /**
   * List of all available audio tracks.
   * Empty array if unknown.
   */
  audioTracks: AudioTrackInfo[];

  /**
   * Information on the currently loaded audio track.
   * `undefined` if unknown or if it has no audio track.
   */
  currentAudioTrack:
    | {
        id: number;
        isSelected: boolean;
      }
    | undefined;

  /**
   * Variant actively locked if one, or `null` if no variant is actively locked
   * on the worker-side.
   */
  lockedVariant: VariantInfo | null;

  /**
   * `AbortController` allowing to cancel the content's loading operation.
   * Set to `undefined` immediately after the content is loaded.
   */
  loadingAborter: AbortController | undefined;

  /**
   * Error encountered in the content which led to the playback being completely
   * interrupted.
   *
   * `null` if no such error was encountered yet.
   */
  error: WaspError | null;
}

/** Enumerates the various "states" the WaspHlsPlayer can be in. */
export const enum PlayerState {
  /** No content is currently loaded or waiting to load. */
  Stopped = "Stopped",
  /** A content is currently being loaded but not ready for playback yet. */
  Loading = "Loading",
  /** A content is loaded. */
  Loaded = "Loaded",
  /** The last content loaded failed on error. */
  Error = "Error",
}

export { AudioTrackInfo, VariantInfo };
