/**
 * dispatcher.ts
 * -------------
 *
 * JS wrapper around the wasm dispatcher.
 * Forwards browser/player events to wasm and owns the native pointer lifetime.
 */

import { Finalizer } from "./finalization_registry.js";
import {
  getWasmExports,
  withFloat64Array,
  withString,
  withUint8Array,
} from "./memory.js";
import { optionalIdToRaw } from "./helpers.js";
import type {
  InitialAudioTrackPreference,
  InitialAudioTrackSelection,
} from "../../ts-common/types.ts";
import type {
  AddSourceBufferErrorCode,
  MediaSourceReadyState,
  PushedSegmentErrorCode,
  TimerReason,
} from "./enums.js";
import { JsTimeRanges, MediaObservation, StartingPosition } from "./results.js";

const textEncoder = new TextEncoder();

/**
 * JS wrapper around `rs-core`'s Dispatcher instance, which is its meain export.
 */
export class Dispatcher {
  /** Raw pointer to the underlying wasm dispatcher. */
  private __ptr: number;

  /**
   * Creates the native dispatcher and binds it to this JS wrapper.
   * @param initial_bandwidth - The initial bandwidth we should start from in the
   * Adaptive BitRate logic, in bits per seconds.
   */
  constructor(initial_bandwidth: number) {
    this.__ptr = getWasmExports().wasp_dispatcher_new(initial_bandwidth);
    dispatcherFinalizer.register(this, this.__ptr);
  }

  /** Frees the native dispatcher immediately. Safe to call more than once. */
  public free(): void {
    if (this.__ptr !== 0) {
      dispatcherFinalizer.unregister(this);
      getWasmExports().wasp_dispatcher_free(this.__ptr);
      this.__ptr = 0;
    }
  }

  /**
   * Starts loading content from the given URL, optionally from a chosen position.
   * @param content_url - The content URL to load. Should be to a multivariant playlist
   * or to a media playlist.
   * @param starting_pos - Optional information on the initial position to start from. If
   * `null` will take one based on the playlist and HLS spec instead.
   */
  public load_content(
    content_url: string,
    starting_pos?: StartingPosition | null,
    initial_audio_track?: InitialAudioTrackPreference | null,
  ): void {
    withString(content_url, (ptr, len) => {
      const selectors =
        normalizeInitialAudioTrackPreference(initial_audio_track);
      const serialized = serializeInitialAudioTrackPreference(selectors);
      withUint8Array(serialized, (initialAudioPtr, initialAudioLen) => {
        getWasmExports().wasp_dispatcher_load_content(
          this.__ptr,
          ptr,
          len,
          starting_pos == null ? 0 : 1,
          starting_pos?.start_type ?? 0,
          starting_pos?.position ?? 0,
          initialAudioPtr,
          initialAudioLen,
        );
      });
    });
  }

  /** Returns the earliest seekable position when known. */
  public minimum_position(): number | undefined {
    const value = getWasmExports().wasp_dispatcher_minimum_position(this.__ptr);
    return Number.isNaN(value) ? undefined : value;
  }

  /** Returns the latest known media position when available. */
  public maximum_position(): number | undefined {
    const value = getWasmExports().wasp_dispatcher_maximum_position(this.__ptr);
    return Number.isNaN(value) ? undefined : value;
  }

  /** Updates the playback speed the dispatcher should target. */
  public set_wanted_speed(speed: number): void {
    getWasmExports().wasp_dispatcher_set_wanted_speed(this.__ptr, speed);
  }

  /** Updates the buffer goal used by the dispatcher. */
  public set_buffer_goal(buffer_goal: number): void {
    getWasmExports().wasp_dispatcher_set_buffer_goal(this.__ptr, buffer_goal);
  }

  /** Stops the dispatcher and any related ongoing work. */
  public stop(): void {
    getWasmExports().wasp_dispatcher_stop(this.__ptr);
  }

  /** Locks playback to a specific variant. */
  public lock_variant(variant_id: number): void {
    getWasmExports().wasp_dispatcher_lock_variant(this.__ptr, variant_id);
  }

  /** Clears any previously locked variant. */
  public unlock_variant(): void {
    getWasmExports().wasp_dispatcher_unlock_variant(this.__ptr);
  }

  /** Selects the audio track to use, or clears the selection. */
  public set_audio_track(track_id?: number | null): void {
    getWasmExports().wasp_dispatcher_set_audio_track(
      this.__ptr,
      optionalIdToRaw(track_id ?? undefined),
    );
  }

  /** Sets the maximum retry count for segment requests. */
  public set_segment_request_max_retry(max_retry: number): void {
    getWasmExports().wasp_dispatcher_set_segment_request_max_retry(
      this.__ptr,
      max_retry,
    );
  }

  /** Sets the timeout used for segment requests. */
  public set_segment_request_timeout(timeout: number): void {
    getWasmExports().wasp_dispatcher_set_segment_request_timeout(
      this.__ptr,
      timeout,
    );
  }

  /** Sets the base delay used for segment-request backoff. */
  public set_segment_backoff_base(base: number): void {
    getWasmExports().wasp_dispatcher_set_segment_backoff_base(this.__ptr, base);
  }

  /** Sets the maximum delay used for segment-request backoff. */
  public set_segment_backoff_max(max: number): void {
    getWasmExports().wasp_dispatcher_set_segment_backoff_max(this.__ptr, max);
  }

  /** Sets the maximum retry count for multivariant playlist requests. */
  public set_multi_variant_playlist_request_max_retry(max_retry: number): void {
    getWasmExports().wasp_dispatcher_set_multi_variant_playlist_request_max_retry(
      this.__ptr,
      max_retry,
    );
  }

  /** Sets the timeout used for multivariant playlist requests. */
  public set_multi_variant_playlist_request_timeout(timeout: number): void {
    getWasmExports().wasp_dispatcher_set_multi_variant_playlist_request_timeout(
      this.__ptr,
      timeout,
    );
  }

  /** Sets the base delay used for multivariant-playlist backoff. */
  public set_multi_variant_playlist_backoff_base(base: number): void {
    getWasmExports().wasp_dispatcher_set_multi_variant_playlist_backoff_base(
      this.__ptr,
      base,
    );
  }

  /** Sets the maximum delay used for multivariant-playlist backoff. */
  public set_multi_variant_playlist_backoff_max(max: number): void {
    getWasmExports().wasp_dispatcher_set_multi_variant_playlist_backoff_max(
      this.__ptr,
      max,
    );
  }

  /** Sets the maximum retry count for media playlist requests. */
  public set_media_playlist_request_max_retry(max_retry: number): void {
    getWasmExports().wasp_dispatcher_set_media_playlist_request_max_retry(
      this.__ptr,
      max_retry,
    );
  }

  /** Sets the timeout used for media playlist requests. */
  public set_media_playlist_request_timeout(timeout: number): void {
    getWasmExports().wasp_dispatcher_set_media_playlist_request_timeout(
      this.__ptr,
      timeout,
    );
  }

  /** Sets the base delay used for media-playlist backoff. */
  public set_media_playlist_backoff_base(base: number): void {
    getWasmExports().wasp_dispatcher_set_media_playlist_backoff_base(
      this.__ptr,
      base,
    );
  }

  /** Sets the maximum delay used for media-playlist backoff. */
  public set_media_playlist_backoff_max(max: number): void {
    getWasmExports().wasp_dispatcher_set_media_playlist_backoff_max(
      this.__ptr,
      max,
    );
  }

  /** Notifies wasm that a network request completed successfully. */
  public on_request_finished(
    request_id: number,
    resource_id: number,
    resource_size: number,
    final_url: string,
    duration_ms: number,
  ): void {
    withString(final_url, (ptr, len) => {
      getWasmExports().__web_event__request_finished(
        this.__ptr,
        request_id,
        resource_id,
        resource_size,
        ptr,
        len,
        duration_ms,
      );
    });
  }

  /** Notifies wasm that a network request failed. */
  public on_request_failed(
    request_id: number,
    has_timeouted: boolean,
    status?: number | null,
  ): void {
    getWasmExports().__web_event__request_failed(
      this.__ptr,
      request_id,
      has_timeouted ? 1 : 0,
      optionalIdToRaw(status ?? undefined),
    );
  }

  /** Forwards a `MediaSource` ready-state update to wasm. */
  public on_media_source_state_change(state: MediaSourceReadyState): void {
    getWasmExports().__web_event__media_source_state_change(this.__ptr, state);
  }

  /** Forwards a `SourceBuffer` buffered-range update to wasm. */
  public on_source_buffer_update(
    source_buffer_id: number,
    buffered: JsTimeRanges,
  ): void {
    withFloat64Array(buffered.buffered, (ptr, len) => {
      getWasmExports().__web_event__source_buffer_update(
        this.__ptr,
        source_buffer_id,
        ptr,
        len,
      );
    });
  }

  /** Reports a source-buffer creation error to wasm. */
  public on_source_buffer_creation_error(
    source_buffer_id: number,
    code: AddSourceBufferErrorCode,
    msg: string,
  ): void {
    withString(msg, (ptr, len) => {
      getWasmExports().__web_event__source_buffer_creation_error(
        this.__ptr,
        source_buffer_id,
        code,
        ptr,
        len,
      );
    });
  }

  /** Reports an append-buffer failure to wasm. */
  public on_append_buffer_error(
    source_buffer_id: number,
    code: PushedSegmentErrorCode,
    buffered: JsTimeRanges,
  ): void {
    withFloat64Array(buffered.buffered, (ptr, len) => {
      getWasmExports().__web_event__append_buffer_error(
        this.__ptr,
        source_buffer_id,
        code,
        ptr,
        len,
      );
    });
  }

  /** Reports a remove-buffer failure to wasm. */
  public on_remove_buffer_error(
    source_buffer_id: number,
    buffered: JsTimeRanges,
  ): void {
    withFloat64Array(buffered.buffered, (ptr, len) => {
      getWasmExports().__web_event__remove_buffer_error(
        this.__ptr,
        source_buffer_id,
        ptr,
        len,
      );
    });
  }

  /** Forwards the latest playback observation snapshot to wasm. */
  public on_playback_tick(observation: MediaObservation): void {
    withFloat64Array(
      observation.buffered.buffered,
      (bufferedPtr, bufferedLen) => {
        const audioBuffered = observation.audio_buffered?.buffered;
        const videoBuffered = observation.video_buffered?.buffered;
        const invoke = (
          audioPtr: number,
          audioLen: number,
          videoPtr: number,
          videoLen: number,
        ) =>
          getWasmExports().__web_event__playback_tick(
            this.__ptr,
            observation.reason,
            observation.current_time,
            observation.ready_state,
            bufferedPtr,
            bufferedLen,
            observation.paused ? 1 : 0,
            observation.seeking ? 1 : 0,
            observation.ended ? 1 : 0,
            observation.duration,
            audioPtr,
            audioLen,
            videoPtr,
            videoLen,
          );
        if (audioBuffered != null) {
          return withFloat64Array(audioBuffered, (audioPtr, audioLen) => {
            if (videoBuffered != null) {
              return withFloat64Array(videoBuffered, (videoPtr, videoLen) =>
                invoke(audioPtr, audioLen, videoPtr, videoLen),
              );
            }
            return invoke(audioPtr, audioLen, 0, 0xffffffff);
          });
        }
        if (videoBuffered != null) {
          return withFloat64Array(videoBuffered, (videoPtr, videoLen) =>
            invoke(0, 0xffffffff, videoPtr, videoLen),
          );
        }
        return invoke(0, 0xffffffff, 0, 0xffffffff);
      },
    );
  }

  /** Signals that a timer managed by wasm has completed. */
  public on_timer_ended(id: number, reason: TimerReason): void {
    getWasmExports().__web_event__timer_ended(this.__ptr, id, reason);
  }

  /** Signals that codec support information changed and should be re-checked. */
  public on_codecs_support_update(): void {
    getWasmExports().__web_event__codecs_support_update(this.__ptr);
  }
}

function normalizeInitialAudioTrackPreference(
  preference: InitialAudioTrackPreference | null | undefined,
): InitialAudioTrackSelection[] {
  const selections = Array.isArray(preference)
    ? preference
    : preference == null
      ? []
      : [preference];
  return selections.filter(
    (selection) => !isEmptyInitialAudioTrackSelection(selection),
  );
}

function isEmptyInitialAudioTrackSelection(
  selection: InitialAudioTrackSelection,
): boolean {
  return (
    selection.language === undefined &&
    selection.assocLanguage === undefined &&
    selection.name === undefined &&
    selection.channels === undefined &&
    (selection.characteristics === undefined ||
      selection.characteristics.length === 0)
  );
}

function serializeInitialAudioTrackPreference(
  selections: InitialAudioTrackSelection[],
): Uint8Array {
  const entries = selections.map(serializeInitialAudioTrackSelectionEntry);
  const totalLength =
    4 + entries.reduce((sum, entry) => sum + entry.byteLength, 0);
  const bytes = new Uint8Array(totalLength);
  const view = new DataView(bytes.buffer);
  let offset = 0;
  view.setUint32(offset, selections.length, true);
  offset += 4;
  for (const entry of entries) {
    bytes.set(entry, offset);
    offset += entry.byteLength;
  }
  return bytes;
}

function serializeInitialAudioTrackSelectionEntry(
  selection: InitialAudioTrackSelection,
): Uint8Array {
  const language = encodeOptionalString(selection.language);
  const assocLanguage = encodeOptionalString(selection.assocLanguage);
  const name = encodeOptionalString(selection.name);
  const characteristics = selection.characteristics?.map(encodeString) ?? [];
  const totalLength =
    language.byteLength +
    assocLanguage.byteLength +
    name.byteLength +
    4 +
    characteristics.reduce((sum, value) => sum + value.byteLength, 0) +
    (selection.channels === undefined ? 1 : 5);
  const bytes = new Uint8Array(totalLength);
  const view = new DataView(bytes.buffer);
  let offset = 0;

  offset = writeBytes(bytes, offset, language);
  offset = writeBytes(bytes, offset, assocLanguage);
  offset = writeBytes(bytes, offset, name);

  view.setUint32(offset, characteristics.length, true);
  offset += 4;
  for (const characteristic of characteristics) {
    offset = writeBytes(bytes, offset, characteristic);
  }

  if (selection.channels === undefined) {
    view.setUint8(offset, 0);
  } else {
    view.setUint8(offset, 1);
    offset += 1;
    view.setUint32(offset, selection.channels, true);
  }

  return bytes;
}

function encodeOptionalString(value: string | undefined): Uint8Array {
  if (value === undefined) {
    const bytes = new Uint8Array(4);
    new DataView(bytes.buffer).setUint32(0, 0xffffffff, true);
    return bytes;
  }
  return encodeString(value);
}

function encodeString(value: string): Uint8Array {
  const payload = textEncoder.encode(value);
  const bytes = new Uint8Array(4 + payload.byteLength);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, payload.byteLength, true);
  bytes.set(payload, 4);
  return bytes;
}

function writeBytes(
  destination: Uint8Array,
  offset: number,
  value: Uint8Array,
): number {
  destination.set(value, offset);
  return offset + value.byteLength;
}

/** Shared fallback cleanup for dispatcher instances dropped by JS without `free()`. */
const dispatcherFinalizer = new Finalizer((ptr) => {
  getWasmExports().wasp_dispatcher_free(ptr);
});
