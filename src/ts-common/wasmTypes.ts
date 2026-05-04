/**
 * Shared enum definitions consumed by the TS code and the handwritten wasm
 * runtime declarations.
 *
 * Keep these values in sync with the Rust/wasm-bindgen interface.
 */
export enum AddSourceBufferErrorCode {
  NoMediaSourceAttached = 0,
  MediaSourceIsClosed = 1,
  QuotaExceededError = 2,
  TypeNotSupportedError = 3,
  EmptyMimeType = 4,
  UnknownError = 5,
}

export enum AttachMediaSourceErrorCode {
  UnknownError = 0,
  NoContentLoaded = 1,
}

export enum EndOfStreamErrorCode {
  NoMediaSourceAttached = 0,
  UnknownError = 1,
}

export enum LogLevel {
  Error = 0,
  Warn = 1,
  Info = 2,
  Debug = 3,
}

export enum MediaPlaylistParsingErrorCode {
  UnparsableExtInf = 0,
  UriMissingInMap = 1,
  MissingTargetDuration = 2,
  UriWithoutExtInf = 3,
  UnparsableByteRange = 4,
  Unknown = 5,
}

export enum MediaSourceDurationUpdateErrorCode {
  NoMediaSourceAttached = 0,
  UnknownError = 1,
}

export enum MediaSourceReadyState {
  Closed = 0,
  Ended = 1,
  Open = 2,
}

export enum MediaType {
  Audio = 0,
  Video = 1,
}

export enum MultivariantPlaylistParsingErrorCode {
  MissingExtM3uHeader = 0,
  MultivariantPlaylistWithoutVariant = 1,
  MissingUriLineAfterVariant = 2,
  UnableToReadVariantUri = 3,
  VariantMissingBandwidth = 4,
  InvalidValue = 5,
  MediaTagMissingType = 6,
  MediaTagMissingName = 7,
  MediaTagMissingGroupId = 8,
  UnableToReadLine = 9,
  Unknown = 10,
}

export enum OtherErrorCode {
  NoSupportedVariant = 0,
  UnfoundLockedVariant = 1,
  MediaSourceAttachmentError = 2,
  Unknown = 3,
}

export enum PlaybackTickReason {
  Init = 0,
  RegularInterval = 1,
  Seeking = 2,
  Seeked = 3,
  LoadedData = 4,
  LoadedMetadata = 5,
  CanPlay = 6,
  CanPlayThrough = 7,
  Ended = 8,
  Pause = 9,
  Play = 10,
  RateChange = 11,
  Stalled = 12,
}

export enum PlaylistNature {
  Event = 0,
  VoD = 1,
  Live = 2,
  Unknown = 3,
}

export enum PushedSegmentErrorCode {
  BufferFull = 0,
  UnknownError = 1,
}

export enum RemoveBufferErrorCode {
  SourceBufferNotFound = 0,
  UnknownError = 1,
}

export enum RemoveMediaSourceErrorCode {
  NoMediaSourceAttached = 0,
  UnknownError = 1,
}

export enum RequestErrorReason {
  Timeout = 0,
  Status = 1,
  Error = 2,
  Other = 3,
}

export enum SegmentParsingErrorCode {
  NoResource = 0,
  NoSourceBuffer = 1,
  TransmuxerError = 2,
  UnknownError = 3,
}

export enum SourceBufferCreationErrorCode {
  AlreadyCreatedWithSameType = 0,
  CantPlayType = 1,
  EmptyMimeType = 2,
  MediaSourceIsClosed = 3,
  NoMediaSourceAttached = 4,
  QuotaExceededError = 5,
  Unknown = 6,
}

export enum StartingPositionType {
  Absolute = 0,
  FromBeginning = 1,
  FromEnd = 2,
}

export enum TimerReason {
  MediaPlaylistRefresh = 0,
  RetryRequest = 1,
}
