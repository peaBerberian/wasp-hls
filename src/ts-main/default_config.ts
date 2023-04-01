import { WaspHlsPlayerConfig } from "../ts-common/types";

/**
 * Default configuration object relied on by the `WaspHlsPlayer`.
 *
 * All of this is overwritable through the API.
 */
const DEFAULT_CONFIG: WaspHlsPlayerConfig = {
  /**
   * Amount of buffer, in seconds, to "build" ahead of the currently wated
   * position.
   *
   * Once that amount is reached, we'll stop loading new data until we go under
   * again.
   */
  bufferGoal: 15,
  /**
   * Number of milliseconds after which a segment request with no response will
   * be automatically cancelled due to a "timeout".
   *
   * Depending on the configuration, the segment request might then be retried.
   */
  segmentRequestTimeout: 20000,
  /**
   * If a segment request has to be retried, we will wait an amount of time
   * before restarting the request. That delay raises if the same segment
   * request fails multiple consecutive times, starting from around this value
   * in milliseconds to `segmentBackoffMax` milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  segmentBackoffBase: 300,
  /**
   * If a segment request has to be retried, we will wait an amount of time
   * before restarting the request. That delay raises if the same segment
   * request fails multiple consecutive times, starting from around
   * `segmentBackoffBase` milliseconds to this value in milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  segmentBackoffMax: 2000,
  /**
   * Number of milliseconds after which a Multivariant Playlist request with no
   * response will be automatically cancelled due to a "timeout".
   *
   * Depending on the configuration, the request might then be retried.
   */
  multiVariantPlaylistRequestTimeout: 15000,
  /**
   * If a Multivariant Playlist request has to be retried, we will wait an
   * amount of time before restarting the request. That delay raises if the same
   * request fails multiple consecutive times, starting from around this value
   * in milliseconds to `segmentBackoffMax` milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  multiVariantPlaylistBackoffBase: 300,
  /**
   * If a Multivariant Playlist request has to be retried, we will wait an
   * amount of time before restarting the request. That delay raises if the
   * same request fails multiple consecutive times, starting from around
   * `segmentBackoffBase` milliseconds to this value in milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  multiVariantPlaylistBackoffMax: 2000,
  /**
   * Number of milliseconds after which a Media Playlist request with no
   * response will be automatically cancelled due to a "timeout".
   *
   * Depending on the configuration, the request might then be retried.
   */
  mediaPlaylistRequestTimeout: 15000,
  /**
   * If a Media Playlist request has to be retried, we will wait an amount of
   * time before restarting the request. That delay raises if the same request
   * fails multiple consecutive times, starting from around this value in
   * milliseconds to `segmentBackoffMax` milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  mediaPlaylistBackoffBase: 300,
  /**
   * If a Media Playlist request has to be retried, we will wait an amount of
   * time before restarting the request. That delay raises if the same request
   * fails multiple consecutive times, starting from around `segmentBackoffBase`
   * milliseconds to this value in milliseconds.
   *
   * The step at which it raises is not configurable here, but can be resumed
   * as a power of 2 raise on the previous value each time.
   */
  mediaPlaylistBackoffMax: 2000,
};

export default DEFAULT_CONFIG;
