/**
 * General type for all potential `code` property that can be set on an Error
 * returned by the `WaspHlsPlayer`.
 */
export const WaspErrorCode = {
  /** The `WaspHlsPlayer` was already initialized. */
  AlreadyInitializedError: "AlreadyInitializedError",
  /**
   * The WebAssembly file could not be fetched due to a request-related error.
   */
  WasmRequestError: "WasmRequestError",
  /** Any other, uncategorized, error that may happen during initialization. */
  UnknownInitializationError: "UnknownInitializationError",

  /**
   * The HTTP(s) status on the response indicated that a segment could not be
   * fetched.
   */
  SegmentBadHttpStatus: "SegmentBadHttpStatus",
  /**
   * The HTTP(s) request for a segment timeouted according to the current
   * configuration.
   */
  SegmentRequestTimeout: "SegmentRequestTimeout",
  /**
   * The HTTP(s) request itself failed to be performed (might be because we're
   * offline, might be because of security policies etc.) for a segment.
   */
  SegmentRequestError: "SegmentRequestError",
  /**
   * The HTTP(s) request itself failed to be performed for another, unknown,
   * reason for a segment.
   */
  SegmentRequestOtherError: "SegmentRequestOtherError",

  /**
   * The HTTP(s) status on the response indicated that the Multivariant Playlist
   * could not be fetched.
   */
  MultivariantPlaylistBadHttpStatus: "MultivariantPlaylistBadHttpStatus",
  /**
   * The HTTP(s) request for the Multivariant Playlist timeouted according to
   * the current configuration.
   */
  MultivariantPlaylistRequestTimeout: "MultivariantPlaylistRequestTimeout",
  /**
   * The HTTP(s) request itself failed to be performed (might be because we're
   * offline, might be because of security policies etc.) for the Multivariant
   * Playlist.
   */
  MultivariantPlaylistRequestError: "MultivariantPlaylistRequestError",
  /**
   * The HTTP(s) request itself failed to be performed for another, unknown,
   * reason for the Multivariant Playlist.
   */
  MultivariantPlaylistRequestOtherError:
    "MultivariantPlaylistRequestOtherError",

  /**
   * The HTTP(s) status on the response indicated that the Media Playlist
   * could not be fetched.
   */
  MediaPlaylistBadHttpStatus: "MediaPlaylistBadHttpStatus",
  /**
   * The HTTP(s) request for the Media Playlist timeouted according to
   * the current configuration.
   */
  MediaPlaylistRequestTimeout: "MediaPlaylistRequestTimeout",
  /**
   * The HTTP(s) request itself failed to be performed (might be because we're
   * offline, might be because of security policies etc.) for the Media
   * Playlist.
   */
  MediaPlaylistRequestError: "MediaPlaylistRequestError",
  /**
   * The HTTP(s) request itself failed to be performed for another, unknown,
   * reason for the Media Playlist.
   */
  MediaPlaylistRequestOtherError: "MediaPlaylistRequestOtherError",

  /**
   * An error arised when trying to either create the `MediaSource` or attempt
   * to attach it to the `MediaSource` HTMLMediaElement.
   */
  MediaSourceAttachmentError: "MediaSourceAttachmentError",
  /**
   * No supported variant was found in the Multivariant Playlist.
   */
  NoSupportedVariant: "NoSupportedVariant",
  /**
   * The variant locked through the `lockVariant` API was not found.
   */
  UnfoundLockedVariant: "UnfoundLockedVariant",
  /** An unknown error arised. */
  Unknown: "Unknown",

  /** The mime type communicated during SourceBuffer creation was not supported. */
  SourceBufferCantPlayType: "SourceBufferCantPlayType",
  /** An uncategorized error arised while creating a SourceBuffer. */
  SourceBufferCreationOtherError: "SourceBufferCreationOtherError",

  /**
   * The first line of the Multivariant Playlist is not #EXTM3U.
   *
   * Are you sure this is a Multivariant Playlist?
   */
  MultivariantPlaylistMissingExtM3uHeader:
    "MultivariantPlaylistMissingExtM3uHeader",
  /**
   * The Multivariant Playlist has no variant.
   *
   * Are you sure this is a Multivariant Playlist and not a Media Playlist?
   */
  MultivariantPlaylistWithoutVariant: "MultivariantPlaylistWithoutVariant",
  /**
   * An `EXT-X-STREAM-INF` tag announced in the Multivariant Playlist,
   * describing an HLS variant, had no URI associated to it. It should be
   * mandatory.
   */
  MultivariantPlaylistMissingUriLineAfterVariant:
    "MultivariantPlaylistMissingUriLineAfterVariant",
  /**
   * An `EXT-X-STREAM-INF` tag announced in the Multivariant Playlist,
   * describing an HLS variant, had no `BANDWIDTH` attribute associated to it.
   * It should be mandatory.
   */
  MultivariantPlaylistVariantMissingBandwidth:
    "MultivariantPlaylistVariantMissingBandwidth",
  /** A value in the Multivariant Playlist was in an invalid format. */
  MultivariantPlaylistInvalidValue: "MultivariantPlaylistInvalidValue",
  /**
   * An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
   * an HLS variant, had no `TYPE` attribute associated to it. It should be
   * mandatory.
   */
  MultivariantPlaylistMediaTagMissingType:
    "MultivariantPlaylistMediaTagMissingType",
  /**
   * An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
   * an HLS variant, had no `NAME` attribute associated to it. It should be
   * mandatory.
   */
  MultivariantPlaylistMediaTagMissingName:
    "MultivariantPlaylistMediaTagMissingName",
  /**
   * An `EXT-X-MEDIA` tag announced in the Multivariant Playlist, describing
   * an HLS variant, had no `GROUP-ID` attribute associated to it. It should be
   * mandatory.
   */
  MultivariantPlaylistMediaTagMissingGroupId:
    "MultivariantPlaylistMediaTagMissingGroupId",
  /**
   * An uncategorized error arised while parsing the Multivariant Playlist.
   */
  MultivariantPlaylistOtherParsingError:
    "MultivariantPlaylistOtherParsingError",

  /**
   * An `#EXTINF` tag announced in the Media Playlist was not in the right
   * format.
   */
  MediaPlaylistUnparsableExtInf: "MediaPlaylistUnparsableExtInf",
  /**
   * An `#EXT-X-MAP` tag in the Media Playlist didn't its mandatory `URI`
   * attribute.
   */
  MediaPlaylistUriMissingInMap: "MediaPlaylistUriMissingInMap",
  /**
   * There was no `#EXT-X-TARGETDURATION` tag in the Media Playlist.
   */
  MediaPlaylistMissingTargetDuration: "MediaPlaylistMissingTargetDuration",
  /**
   * One of the URI found in the MediaPlaylist wasn't associated to any
   * `#EXTINF` tag.
   */
  MediaPlaylistUriWithoutExtInf: "MediaPlaylistUriWithoutExtInf",
  /**
   * A `#EXT-X-BYTERANGE` tag or a `BYTERANGE` attribute in the Media Playlist
   * was not in the right format.
   */
  MediaPlaylistUnparsableByteRange: "MediaPlaylistUnparsableByteRange",
  /** Another uncategorized error happened while parsing the Media Playlist. */
  MediaPlaylistOtherParsingError: "MediaPlaylistOtherParsingError",

  /** An error arised when trying to transmux a segment, */
  SegmentTransmuxingError: "SegmentTransmuxingError",
  /** An uncategorized error arised when parsing a segment, */
  SegmentParsingOtherError: "SegmentParsingOtherError",

  /**
   * We could not add more data to the `SourceBuffer` because it is full.
   */
  SourceBufferFullError: "SourceBufferFullError",
  /**
   * An error arised when pushing a segment to the `SourceBuffer`.
   * Generally, this happens when the pushed segment is malformed.
   */
  SourceBufferAppendError: "SourceBufferAppendError",
  /**
   * An error arised when removing data from the SourceBuffer.
   */
  SourceBufferRemoveError: "SourceBufferRemoveError",
  /**
   * An uncategorized error arised when doing an operation on a `SourceBuffer`.
   */
  SourceBufferOtherError: "SourceBufferOtherError",
} as const;
