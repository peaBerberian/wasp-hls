import { WaspHlsPlayerConfig } from "../ts-common/types";

/**
 * Default configuration object relied on by the `WaspHlsPlayer`.
 *
 * All of this is overwritable through the API.
 * @see WaspHlsPlayerConfig
 */
const DEFAULT_CONFIG: WaspHlsPlayerConfig = {
  bufferGoal: 15,
  segmentMaxRetry: 5,
  segmentRequestTimeout: 20000,
  segmentBackoffBase: 300,
  segmentBackoffMax: 2000,
  multiVariantPlaylistMaxRetry: 2,
  multiVariantPlaylistRequestTimeout: 15000,
  multiVariantPlaylistBackoffBase: 300,
  multiVariantPlaylistBackoffMax: 2000,
  mediaPlaylistMaxRetry: 3,
  mediaPlaylistRequestTimeout: 15000,
  mediaPlaylistBackoffBase: 300,
  mediaPlaylistBackoffMax: 2000,
};

export default DEFAULT_CONFIG;
