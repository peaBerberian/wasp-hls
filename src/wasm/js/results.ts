import type {
  AddSourceBufferErrorCode,
  AttachMediaSourceErrorCode,
  EndOfStreamErrorCode,
  MediaSourceDurationUpdateErrorCode,
  PlaybackTickReason,
  RemoveBufferErrorCode,
  RemoveMediaSourceErrorCode,
  SegmentParsingErrorCode,
  StartingPositionType,
} from "./enums.js";
import type {
  AddSourceBufferValue,
  AppendBufferValue,
  HostResult,
  ISafeU64,
} from "./types.js";

class SimpleResult<Value, ErrorCode extends number> implements HostResult<
  Value,
  ErrorCode
> {
  constructor(
    public value: Value | undefined,
    public errorCode: ErrorCode | undefined,
    public description: string | undefined,
  ) {}

  public free(): void {}
}

export class AddSourceBufferResult extends SimpleResult<
  AddSourceBufferValue,
  AddSourceBufferErrorCode
> {
  public static success(
    id: number,
    needsTransmux: boolean,
  ): AddSourceBufferResult {
    return new AddSourceBufferResult(
      { id, needsTransmux },
      undefined,
      undefined,
    );
  }

  public static error(
    error: AddSourceBufferErrorCode,
    description?: string | null,
  ): AddSourceBufferResult {
    return new AddSourceBufferResult(
      undefined,
      error,
      description ?? undefined,
    );
  }
}

export class AppendBufferResult extends SimpleResult<
  AppendBufferValue,
  SegmentParsingErrorCode
> {
  public static success(
    start?: ISafeU64 | undefined,
    end?: ISafeU64 | undefined,
    timescale?: number | undefined,
  ): AppendBufferResult {
    return new AppendBufferResult(
      {
        start: start ?? undefined,
        end: end ?? undefined,
        timescale: timescale ?? undefined,
      },
      undefined,
      undefined,
    );
  }

  public static error(
    error: SegmentParsingErrorCode,
    description?: string | null,
  ): AppendBufferResult {
    return new AppendBufferResult(undefined, error, description ?? undefined);
  }
}

export class AttachMediaSourceResult extends SimpleResult<
  true,
  AttachMediaSourceErrorCode
> {
  public static success(): AttachMediaSourceResult {
    return new AttachMediaSourceResult(true, undefined, undefined);
  }

  public static error(
    error: AttachMediaSourceErrorCode,
    description?: string | null,
  ): AttachMediaSourceResult {
    return new AttachMediaSourceResult(
      undefined,
      error,
      description ?? undefined,
    );
  }
}

export class EndOfStreamResult extends SimpleResult<
  true,
  EndOfStreamErrorCode
> {
  public static success(): EndOfStreamResult {
    return new EndOfStreamResult(true, undefined, undefined);
  }

  public static error(
    error: EndOfStreamErrorCode,
    description?: string | null,
  ): EndOfStreamResult {
    return new EndOfStreamResult(undefined, error, description ?? undefined);
  }
}

export class MediaSourceDurationUpdateResult extends SimpleResult<
  true,
  MediaSourceDurationUpdateErrorCode
> {
  public static success(): MediaSourceDurationUpdateResult {
    return new MediaSourceDurationUpdateResult(true, undefined, undefined);
  }

  public static error(
    error: MediaSourceDurationUpdateErrorCode,
    description?: string | null,
  ): MediaSourceDurationUpdateResult {
    return new MediaSourceDurationUpdateResult(
      undefined,
      error,
      description ?? undefined,
    );
  }
}

export class RemoveBufferResult extends SimpleResult<
  true,
  RemoveBufferErrorCode
> {
  public static success(): RemoveBufferResult {
    return new RemoveBufferResult(true, undefined, undefined);
  }

  public static error(
    error: RemoveBufferErrorCode,
    description?: string | null,
  ): RemoveBufferResult {
    return new RemoveBufferResult(undefined, error, description ?? undefined);
  }
}

export class RemoveMediaSourceResult extends SimpleResult<
  true,
  RemoveMediaSourceErrorCode
> {
  public static success(): RemoveMediaSourceResult {
    return new RemoveMediaSourceResult(true, undefined, undefined);
  }

  public static error(
    error: RemoveMediaSourceErrorCode,
    description?: string | null,
  ): RemoveMediaSourceResult {
    return new RemoveMediaSourceResult(
      undefined,
      error,
      description ?? undefined,
    );
  }
}

export class JsTimeRanges {
  constructor(public buffered: Float64Array) {}

  public free(): void {}

  public len(): number {
    return this.buffered.length / 2;
  }

  public start(idx: number): number | undefined {
    return this.buffered[idx * 2];
  }

  public end(idx: number): number | undefined {
    return this.buffered[idx * 2 + 1];
  }

  public start_unchecked(idx: number): number {
    return this.start(idx) ?? 0;
  }

  public end_unchecked(idx: number): number {
    return this.end(idx) ?? 0;
  }
}

export class MediaObservation {
  constructor(
    public reason: PlaybackTickReason,
    public current_time: number,
    public ready_state: number,
    public buffered: JsTimeRanges,
    public paused: boolean,
    public seeking: boolean,
    public ended: boolean,
    public duration: number,
    public audio_buffered?: JsTimeRanges | null,
    public video_buffered?: JsTimeRanges | null,
  ) {
    this.audio_buffered = audio_buffered ?? undefined;
    this.video_buffered = video_buffered ?? undefined;
  }

  public free(): void {}
}

export class StartingPosition {
  constructor(
    public start_type: StartingPositionType,
    public position: number,
  ) {}

  public free(): void {}
}
