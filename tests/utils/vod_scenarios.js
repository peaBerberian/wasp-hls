function getBaseUrl() {
  return (
    "http://" + __TEST_CONTENT_SERVER__.URL + ":" + __TEST_CONTENT_SERVER__.PORT
  );
}

const SCENARIO_ENTRY_FILES = {
  "fmp4-direct-media": "playlist.m3u8",
  "fmp4-direct-media-redirect": "playlist.m3u8",
  "mpegts-direct-media": "playlist.m3u8",
  "fmp4-multivariant-no-codecs": "master.m3u8",
  "fmp4-multivariant-master-redirect": "master.m3u8",
  "mpegts-multivariant-no-codecs": "master.m3u8",
  "fmp4-player-api": "playlist.m3u8",
  "fmp4-direct-media-endlist-without-playlist-type": "playlist.m3u8",
  "fmp4-player-api-program-date-time": "playlist.m3u8",
  "fmp4-direct-media-byterange": "playlist.m3u8",
  "fmp4-player-api-ext-x-start": "playlist.m3u8",
  "fmp4-player-api-ext-x-start-imprecise": "playlist.m3u8",
  "fmp4-audio-only": "master.m3u8",
  "fmp4-alt-audio": "master.m3u8",
  "fmp4-multivariant-alt-audio": "master.m3u8",
  "fmp4-shared-audio-muxed": "master.m3u8",
  "fmp4-error-missing-target-duration": "playlist.m3u8",
  "fmp4-error-unparsable-extinf": "playlist.m3u8",
  "fmp4-error-uri-missing-in-map": "playlist.m3u8",
  "fmp4-error-uri-without-extinf": "playlist.m3u8",
  "fmp4-error-unparsable-byterange": "playlist.m3u8",
  "fmp4-error-media-variable-definition": "playlist.m3u8",
  "fmp4-error-media-duplicate-singleton": "playlist.m3u8",
  "fmp4-error-media-conflicting-tag-types": "playlist.m3u8",
  "fmp4-error-master-missing-uri-after-variant": "master.m3u8",
  "fmp4-error-master-missing-uri-after-variant-comment": "master.m3u8",
  "fmp4-error-master-variant-missing-bandwidth": "master.m3u8",
  "fmp4-error-master-duplicate-singleton": "master.m3u8",
  "fmp4-error-master-conflicting-tag-types": "master.m3u8",
  "fmp4-error-master-invalid-value": "master.m3u8",
  "fmp4-error-master-missing-required-attribute": "master.m3u8",
  "fmp4-error-master-variable-definition": "master.m3u8",
  "fmp4-error-master-without-variant": "master.m3u8",
  "fmp4-error-master-other-parsing-error": "master.m3u8",
  "fmp4-error-top-level-missing-extm3u": "master.m3u8",
};

export function getVodScenarioUrl(scenarioId) {
  const entryFile = SCENARIO_ENTRY_FILES[scenarioId];
  if (entryFile === undefined) {
    throw new Error(`Unknown VoD scenario: ${scenarioId}`);
  }
  return `${getBaseUrl()}/vod/scenario/${scenarioId}/${entryFile}`;
}

export async function ensureVodScenarioReady(scenarioId) {
  const url = getVodScenarioUrl(scenarioId);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `Unexpected status ${response.status} while preparing VoD scenario "${scenarioId}"`,
    );
  }
  await response.text();
  return url;
}
