/* eslint-disable @typescript-eslint/naming-convention, max-classes-per-file */
import type {
  AddSourceBufferErrorCode,
  AttachMediaSourceErrorCode,
  EndOfStreamErrorCode,
  MediaSourceDurationUpdateErrorCode,
  MediaSourceReadyState,
  PlaybackTickReason,
  PushedSegmentErrorCode,
  RemoveBufferErrorCode,
  RemoveMediaSourceErrorCode,
  SegmentParsingErrorCode,
  StartingPositionType,
  TimerReason,
} from "../ts-common/wasmTypes";

export class AddSourceBufferResult {
  private constructor();
  free(): void;
  static error(
    err: AddSourceBufferErrorCode,
    desc?: string | null,
  ): AddSourceBufferResult;
  static success(val: number): AddSourceBufferResult;
}

export class AppendBufferResult {
  private constructor();
  free(): void;
  static error(
    err: SegmentParsingErrorCode,
    desc?: string | null,
  ): AppendBufferResult;
  static success(
    start?: number | null,
    duration?: number | null,
  ): AppendBufferResult;
}

export class AttachMediaSourceResult {
  private constructor();
  free(): void;
  static error(
    err: AttachMediaSourceErrorCode,
    desc?: string | null,
  ): AttachMediaSourceResult;
  static success(): AttachMediaSourceResult;
}

export class JsTimeRanges {
  constructor(buffered: Float64Array);
  free(): void;
  end(idx: number): number | undefined;
  end_unchecked(idx: number): number;
  len(): number;
  start(idx: number): number | undefined;
  start_unchecked(idx: number): number;
}

export class MediaObservation {
  constructor(
    reason: PlaybackTickReason,
    current_time: number,
    ready_state: number,
    buffered: JsTimeRanges,
    paused: boolean,
    seeking: boolean,
    ended: boolean,
    duration: number,
    audio_buffered?: JsTimeRanges | null,
    video_buffered?: JsTimeRanges | null,
  );
  free(): void;
}

export class Dispatcher {
  constructor(initial_bandwidth: number);
  free(): void;
  load_content(
    content_url: string,
    starting_pos?: StartingPosition | null,
  ): void;
  lock_variant(variant_id: number): void;
  maximum_position(): number | undefined;
  minimum_position(): number | undefined;
  on_append_buffer_error(
    source_buffer_id: number,
    code: PushedSegmentErrorCode,
    buffered: JsTimeRanges,
  ): void;
  on_codecs_support_update(): void;
  on_media_source_state_change(state: MediaSourceReadyState): void;
  on_playback_tick(observation: MediaObservation): void;
  on_remove_buffer_error(
    source_buffer_id: number,
    buffered: JsTimeRanges,
  ): void;
  on_request_failed(
    request_id: number,
    has_timeouted: boolean,
    status?: number | null,
  ): void;
  on_request_finished(
    request_id: number,
    resource_id: number,
    resource_size: number,
    final_url: string,
    duration_ms: number,
  ): void;
  on_source_buffer_creation_error(
    source_buffer_id: number,
    code: AddSourceBufferErrorCode,
    msg: string,
  ): void;
  on_source_buffer_update(
    source_buffer_id: number,
    buffered: JsTimeRanges,
  ): void;
  on_timer_ended(id: number, reason: TimerReason): void;
  set_audio_track(track_id?: number | null): void;
  set_buffer_goal(buffer_goal: number): void;
  set_media_playlist_backoff_base(base: number): void;
  set_media_playlist_backoff_max(max: number): void;
  set_media_playlist_request_max_retry(max_retry: number): void;
  set_media_playlist_request_timeout(timeout: number): void;
  set_multi_variant_playlist_backoff_base(base: number): void;
  set_multi_variant_playlist_backoff_max(max: number): void;
  set_multi_variant_playlist_request_max_retry(max_retry: number): void;
  set_multi_variant_playlist_request_timeout(timeout: number): void;
  set_segment_backoff_base(base: number): void;
  set_segment_backoff_max(max: number): void;
  set_segment_request_max_retry(max_retry: number): void;
  set_segment_request_timeout(timeout: number): void;
  set_wanted_speed(speed: number): void;
  stop(): void;
  unlock_variant(): void;
}

export class EndOfStreamResult {
  private constructor();
  free(): void;
  static error(
    err: EndOfStreamErrorCode,
    desc?: string | null,
  ): EndOfStreamResult;
  static success(): EndOfStreamResult;
}

export class MediaSourceDurationUpdateResult {
  private constructor();
  free(): void;
  static error(
    err: MediaSourceDurationUpdateErrorCode,
    desc?: string | null,
  ): MediaSourceDurationUpdateResult;
  static success(): MediaSourceDurationUpdateResult;
}

export class RemoveBufferResult {
  private constructor();
  free(): void;
  static error(
    err: RemoveBufferErrorCode,
    desc?: string | null,
  ): RemoveBufferResult;
  static success(): RemoveBufferResult;
}

export class RemoveMediaSourceResult {
  private constructor();
  free(): void;
  static error(
    err: RemoveMediaSourceErrorCode,
    desc?: string | null,
  ): RemoveMediaSourceResult;
  static success(): RemoveMediaSourceResult;
}

export class StartingPosition {
  constructor(start_type: StartingPositionType, position: number);
  free(): void;
}

export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
}

export default function initializeWasm(
  module_or_path?:
    | { module_or_path: InitInput | Promise<InitInput> }
    | InitInput
    | Promise<InitInput>,
): Promise<InitOutput>;
