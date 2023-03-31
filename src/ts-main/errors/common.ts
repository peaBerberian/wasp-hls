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
   * The HTTP(s) status on the response indicated that the MultiVariant Playlist
   * could not be fetched.
   */
  MultiVariantPlaylistBadHttpStatus: "MultiVariantPlaylistBadHttpStatus",
  /**
   * The HTTP(s) request for the MultiVariant Playlist timeouted according to
   * the current configuration.
   */
  MultiVariantPlaylistRequestTimeout: "MultiVariantPlaylistRequestTimeout",
  /**
   * The HTTP(s) request itself failed to be performed (might be because we're
   * offline, might be because of security policies etc.) for the MultiVariant
   * Playlist.
   */
  MultiVariantPlaylistRequestError: "MultiVariantPlaylistRequestError",
  /**
   * The HTTP(s) request itself failed to be performed for another, unknown,
   * reason for the MultiVariant Playlist.
   */
  MultiVariantPlaylistRequestOtherError:
    "MultiVariantPlaylistRequestOtherError",

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
   * An error arised when trying to attach the `MediaSource` instance to the
   * HTMLMediaElement.
   */
  MediaSourceAttachmentError: "MediaSourceAttachmentError",
  /**
   * No supported variant was found in the MultiVariant Playlist.
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
   * The first line of the MultiVariant Playlist is not #EXTM3U.
   *
   * Are you sure this is a MultiVariant Playlist?
   */
  MultiVariantPlaylistMissingExtM3uHeader:
    "MultiVariantPlaylistMissingExtM3uHeader",
  /**
   * The MultiVariant Playlist has no variant.
   *
   * Are you sure this is a MultiVariant Playlist and not a Media Playlist?
   */
  MultiVariantPlaylistWithoutVariant: "MultiVariantPlaylistWithoutVariant",
  /**
   * An `EXT-X-STREAM-INF` tag announced in the MultiVariant Playlist,
   * describing an HLS variant, had no URI associated to it. It should be
   * mandatory.
   */
  MultiVariantPlaylistMissingUriLineAfterVariant:
    "MultiVariantPlaylistMissingUriLineAfterVariant",
  /**
   * An `EXT-X-STREAM-INF` tag announced in the MultiVariant Playlist,
   * describing an HLS variant, had no `BANDWIDTH` attribute associated to it.
   * It should be mandatory.
   */
  MultiVariantPlaylistVariantMissingBandwidth:
    "MultiVariantPlaylistVariantMissingBandwidth",
  /** A value in the MultiVariant Playlist was in an invalid format. */
  MultiVariantPlaylistInvalidValue: "MultiVariantPlaylistInvalidValue",
  /**
   * An `EXT-X-MEDIA` tag announced in the MultiVariant Playlist, describing
   * an HLS variant, had no `TYPE` attribute associated to it. It should be
   * mandatory.
   */
  MultiVariantPlaylistMediaTagMissingType:
    "MultiVariantPlaylistMediaTagMissingType",
  /**
   * An `EXT-X-MEDIA` tag announced in the MultiVariant Playlist, describing
   * an HLS variant, had no `NAME` attribute associated to it. It should be
   * mandatory.
   */
  MultiVariantPlaylistMediaTagMissingName:
    "MultiVariantPlaylistMediaTagMissingName",
  /**
   * An `EXT-X-MEDIA` tag announced in the MultiVariant Playlist, describing
   * an HLS variant, had no `GROUP-ID` attribute associated to it. It should be
   * mandatory.
   */
  MultiVariantPlaylistMediaTagMissingGroupId:
    "MultiVariantPlaylistMediaTagMissingGroupId",
  MultiVariantPlaylistOtherError: "MultiVariantPlaylistOtherError",

  /** An error arised when trying to transmux a segment, */
  SegmentTransmuxingError: "SegmentTransmuxingError",
  /** An uncategorized error arised when parsing a segment, */
  SegmentParsingOtherError: "SegmentParsingOtherError",

  /**
   * A `QuotaExceededError` happens on a SourceBuffer.
   * This generally happens when the SourceBuffer is full.
   */
  SourceBufferQuotaExceededError: "SourceBufferQuotaExceededError",
  /**
   * An uncategorized error arised when doing an operation on a `SourceBuffer`.
   * Generally, this happens when pushed segments are malformed.
   */
  SourceBufferOtherError: "SourceBufferOtherError",
} as const;
