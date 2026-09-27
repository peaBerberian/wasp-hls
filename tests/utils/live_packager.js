import sleep from "./sleep.js";

const PACKAGER_READY_TIMEOUT_MS = 120_000;
const PACKAGER_STOP_TIMEOUT_MS = 10_000;
const PACKAGER_STOP_POLL_INTERVAL_MS = 200;

export function createLivePackagerClient(port = __TEST_CONTENT_SERVER__.PORT) {
  function getBaseUrl() {
    return "http://" + __TEST_CONTENT_SERVER__.URL + ":" + port;
  }

  function getStartPackagerUrl() {
    return getBaseUrl() + "/start_packager";
  }

  function getStopPackagerUrl() {
    return getBaseUrl() + "/stop_packager";
  }

  function getPackagerStatusUrl() {
    return getBaseUrl() + "/packager_status";
  }

  function extractPlaylistReferences(playlistText) {
    return playlistText
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0 && !line.startsWith("#"));
  }

  function extractMediaTagUris(playlistText) {
    return playlistText
      .split("\n")
      .map((line) => {
        const trimmedLine = line.trim();
        if (!trimmedLine.startsWith("#EXT-X-MEDIA:")) {
          return null;
        }
        const match = /URI="([^"]+)"/.exec(trimmedLine);
        return match === null ? null : match[1];
      })
      .filter((uri) => uri !== null);
  }

  async function fetchText(url) {
    const response = await fetch(url);
    return {
      status: response.status,
      ok: response.ok,
      text: await response.text(),
    };
  }

  async function fetchBinary(url) {
    const response = await fetch(url);
    await response.arrayBuffer();
    return {
      ok: response.ok,
    };
  }

  async function consumeFetchResponse(responsePromise) {
    const response = await responsePromise;
    await response.arrayBuffer();
    return response;
  }

  async function fetchCurrentPackagerStatus() {
    const response = await fetch(getPackagerStatusUrl());
    return await response.json();
  }

  async function waitForPackagerStop() {
    const startedAt = Date.now();

    while (Date.now() - startedAt < PACKAGER_STOP_TIMEOUT_MS) {
      const status = await fetchCurrentPackagerStatus();
      if (!status.active) {
        return;
      }
      await sleep(PACKAGER_STOP_POLL_INTERVAL_MS);
    }

    throw new Error("Timed out waiting for live packager shutdown");
  }

  async function waitForStableLiveOutput(playlistUrl) {
    for (let attempt = 0; attempt < 60; attempt++) {
      const master = await fetchText(playlistUrl);
      if (!master.ok) {
        await sleep(1000);
        continue;
      }

      const variantUrl = extractPlaylistReferences(master.text)
        .filter((line) => line.endsWith(".m3u8"))
        .map((line) => new URL(line, playlistUrl).href)[0];
      const audioUrl = extractMediaTagUris(master.text).map(
        (line) => new URL(line, playlistUrl).href,
      )[0];
      if (variantUrl == null || audioUrl == null) {
        await sleep(1000);
        continue;
      }

      const [variantPlaylist, audioPlaylist] = await Promise.all([
        fetchText(variantUrl),
        fetchText(audioUrl),
      ]);
      if (!variantPlaylist.ok || !audioPlaylist.ok) {
        await sleep(1000);
        continue;
      }

      const variantSegmentRef = extractPlaylistReferences(
        variantPlaylist.text,
      )[0];
      const audioSegmentRef = extractPlaylistReferences(audioPlaylist.text)[0];
      if (variantSegmentRef == null || audioSegmentRef == null) {
        await sleep(1000);
        continue;
      }

      const [variantSegment, audioSegment] = await Promise.all([
        fetchBinary(new URL(variantSegmentRef, variantUrl).href),
        fetchBinary(new URL(audioSegmentRef, audioUrl).href),
      ]);
      if (variantSegment.ok && audioSegment.ok) {
        return;
      }

      await sleep(1000);
    }

    throw new Error("Live packager output did not become fetchable");
  }

  async function startLivePackager() {
    await startLivePackagerWithOptions();
  }

  async function startLivePackagerWithOptions({
    emitProgramDateTime = false,
  } = {}) {
    const query = new URLSearchParams({
      enableTextTrack: "1",
    });
    if (emitProgramDateTime) {
      query.set("emitProgramDateTime", "1");
    }
    await consumeFetchResponse(
      fetch(`${getStartPackagerUrl()}?${query.toString()}`, { method: "POST" }),
    );
  }

  async function stopLivePackager() {
    await consumeFetchResponse(fetch(getStopPackagerUrl(), { method: "POST" }));
    await waitForPackagerStop();
  }

  async function waitForPackagerReady() {
    const startedAt = Date.now();

    while (Date.now() - startedAt < PACKAGER_READY_TIMEOUT_MS) {
      await sleep(1500);
      const status = await fetchCurrentPackagerStatus();

      if (!status.active) {
        throw new Error("Live packager stopped before it became ready");
      }

      const playlistUrl = getBaseUrl() + status.info.playlistPath;
      const segmentDuration = status.info.segmentDuration;
      const timeShiftBufferDepth = status.info.timeShiftBufferDepth;
      const emitProgramDateTime = status.info.emitProgramDateTime;

      const playlistResponse = await fetchText(playlistUrl);
      if (playlistResponse.status === 404) {
        continue;
      }
      if (!playlistResponse.ok) {
        throw new Error(
          `Unexpected status while requesting generated playlist: ${playlistResponse.status}`,
        );
      }

      await sleep((timeShiftBufferDepth ?? 1) * 1000);
      await waitForStableLiveOutput(playlistUrl);

      return {
        playlistUrl,
        segmentDuration,
        timeShiftBufferDepth,
        emitProgramDateTime,
      };
    }

    throw new Error("Timed out waiting for live packager readiness");
  }

  return {
    startLivePackager,
    startLivePackagerWithOptions,
    stopLivePackager,
    waitForPackagerReady,
  };
}
export const {
  startLivePackager,
  startLivePackagerWithOptions,
  stopLivePackager,
  waitForPackagerReady,
} = createLivePackagerClient();
