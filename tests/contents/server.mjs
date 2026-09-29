/* eslint-env node */

/* eslint-disable no-console */

import { createServer } from "http";
import { spawn, spawnSync } from "child_process";
import * as path from "path";
import { fileURLToPath, pathToFileURL } from "url";
import * as fs from "fs";
import urls from "./static/urls.mjs";
import {
  ensureVodRecipe,
  getVodGeneratedRelativeFilePath,
  getVodRecipeIdFromGeneratedPath,
  getVodRecipeOutputDir,
  getVodScenarioResponse,
} from "./vod_fixtures.mjs";

/**
 * @typedef {import("http").ServerResponse} ServerResponse
 * @typedef {import("http").IncomingMessage} IncomingMessage
 * @typedef {import("child_process").ChildProcess} PackagingProcess
 * @typedef {import("./static/urls.mjs").StaticUrlItem} StaticUrlItem
 * @typedef {{process: PackagingProcess, timeShiftBufferDepth: number,
 * segmentDuration: number, playlistPath: string, emitProgramDateTime: boolean}} PackagingProcessInfo
 * @typedef {{ext: string, filePath: string, size: number, mtimeMs: number}} PreparedFile
 * @typedef {{startedAtMs: number}} EventScenarioState
 */

/** To activate if you're having content packaging issues. */
const ACTIVATE_PACKAGER_LOGS = false;

/** Path of the current file. */
const __filename = fileURLToPath(import.meta.url);
/** Directory of the current file. */
const __dirname = path.dirname(__filename);

/** Live contents are lazily packaged in this directory. */
const DEFAULT_PACKAGED_LIVE_OS_PATH = path.join(
  __dirname,
  "..",
  "..",
  "tmp",
  "testcontents",
  "live",
);

// Transform `urls` array into an Object where the key is the url of each
// element.
/** @type {Record<string, StaticUrlItem>} */
const routeObj = urls.reduce((acc, elt) => {
  acc[elt.url] = elt;
  return acc;
}, /** @type {Record<string, StaticUrlItem>} */ ({}));

const DEFAULT_CONTENT_SERVER_PORT = 3000;
const CONTENT_TYPE_M3U8 = "application/vnd.apple.mpegurl";
const EVENT_ENDLIST_SCENARIO_SEGMENT_DURATION_S = 2;
const EVENT_ENDLIST_SCENARIO_INITIAL_SEGMENT_COUNT = 4;
const EVENT_ENDLIST_SCENARIO_FINAL_SEGMENT_COUNT = 6;
// Synthetic live scenario used to exercise an EVENT playlist that later
// becomes terminal through EXT-X-ENDLIST. This is kept outside the packager
// because the packager shutdown path removes its output instead of finalizing
// the manifest.
const EVENT_ENDLIST_SCENARIO_PREFIX = "/live/scenario/event-endlist";

/** @type {PackagingProcessInfo | null} Content packaging process. */
let packagingProcessInfo = null;
let eventEndlistScenarioState = createEventEndlistScenarioState();
// Slow Windows CI machines can briefly make newly written live files
// unavailable. Retry opening them a few times before surfacing a 404.
const LIVE_FILE_OPEN_RETRY_COUNT = 20;
const LIVE_FILE_OPEN_RETRY_DELAY_MS = 25;

/**
 * Create simple HTTP server specifically designed to serve the contents defined
 * in this directory.
 *
 * Main endpoint groups:
 *   - `/`: HTML index of statically registered routes
 *   - `/live/*`: files produced by the live packager
 *   - `/vod/generated/*`: generated VoD assets backing fixtures
 *   - `/vod/scenario/*`: synthetic VoD manifests built on top of generated assets
 *   - `/start_packager`, `/stop_packager`, `/packager_status`: live packager control API
 *   - `/live/scenario/event-endlist/*`: synthetic live/Event manifest transition used
 *     by the EXT-X-ENDLIST integration test
 *
 * Route ordering matters: more specific synthetic scenario endpoints need to be
 * checked before generic prefixes like `/live/`.
 *
 * @param {{port?: number}} [params]
 * @returns {{listeningPromise: Promise<void>, close: () => Promise<void>}}
 */
export default function createContentServer({
  port = DEFAULT_CONTENT_SERVER_PORT,
} = {}) {
  /** @type {Set<import("net").Socket>} */
  const activeSockets = new Set();
  const contentServerBaseUrl = "http://127.0.0.1:" + String(port);
  const server = createServer(function (req, res) {
    if (req.url === undefined || req.method === undefined) {
      answerWithCORS(res, 400, "400 Bad Request");
      return;
    }
    const requestUrl = new URL(req.url, "http://127.0.0.1");

    if (req.url === "/") {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (req.method.toUpperCase() !== "GET") {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }

      const html = generateUrlListHtml(
        urls,
        "http://127.0.0.1:" + String(port),
      );
      answerWithCORS(res, 200, html);
      return;
    }

    // Scenario control endpoint. Tests call it before loading the content so
    // they always start from the non-finalized EVENT playlist.
    if (requestUrl.pathname === `${EVENT_ENDLIST_SCENARIO_PREFIX}/reset`) {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (
        req.method.toUpperCase() !== "POST" &&
        req.method.toUpperCase() !== "GET"
      ) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }

      eventEndlistScenarioState = createEventEndlistScenarioState();
      res.setHeader("Content-Type", "application/json");
      answerWithCORS(
        res,
        200,
        JSON.stringify({
          playlistUrl:
            `${contentServerBaseUrl}${EVENT_ENDLIST_SCENARIO_PREFIX}` +
            "/playlist.m3u8",
        }),
      );
      return;
    }

    // Synthetic playlist endpoint. It behaves like a small EVENT playlist
    // whose publication advances with wall-clock time after `/reset`.
    if (
      requestUrl.pathname === `${EVENT_ENDLIST_SCENARIO_PREFIX}/playlist.m3u8`
    ) {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (req.method.toUpperCase() !== "GET") {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }

      handleEventEndlistScenarioPlaylistRequest(
        res,
        contentServerBaseUrl,
        eventEndlistScenarioState,
      );
      return;
    }

    if (req.url.startsWith("/live/")) {
      handlePackagedLiveRequest(res, req, "/live/");
      return;
    }

    if (req.url.startsWith("/live-alt/")) {
      handlePackagedLiveRequest(res, req, "/live-alt/");
      return;
    }

    if (req.url.startsWith("/vod/generated/")) {
      handlePackagedVodRequest(res, req, requestUrl, "/vod/generated/");
      return;
    }

    if (req.url.startsWith("/vod/scenario/")) {
      handleVodScenarioRequest(
        res,
        req,
        requestUrl,
        "/vod/scenario/",
        contentServerBaseUrl,
      );
      return;
    }

    if (requestUrl.pathname === "/start_packager") {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (
        req.method.toUpperCase() !== "POST" &&
        req.method.toUpperCase() !== "GET"
      ) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }
      handleStartPackager(res, requestUrl);
      return;
    }

    if (requestUrl.pathname === "/packager_status") {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (req.method.toUpperCase() !== "GET") {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }

      res.setHeader("Content-Type", "application/json");
      const jsonResponse =
        packagingProcessInfo === null
          ? {
              active: false,
              info: null,
            }
          : {
              active: true,
              info: {
                pid: packagingProcessInfo.process.pid,
                playlistPath: packagingProcessInfo.playlistPath,
                timeShiftBufferDepth: packagingProcessInfo.timeShiftBufferDepth,
                segmentDuration: packagingProcessInfo.segmentDuration,
                emitProgramDateTime: packagingProcessInfo.emitProgramDateTime,
              },
            };
      answerWithCORS(res, 200, JSON.stringify(jsonResponse));
      return;
    }

    if (requestUrl.pathname === "/stop_packager") {
      if (req.method.toUpperCase() === "OPTIONS") {
        answerWithCORS(res, 200);
        res.end();
        return;
      }
      if (
        req.method.toUpperCase() !== "POST" &&
        req.method.toUpperCase() !== "GET"
      ) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 405, "405 Method Not Allowed");
        return;
      }

      handleStopPackager(res);
      return;
    }

    // Handle regular routes
    if (routeObj[req.url] == null) {
      res.setHeader("Content-Type", "text/plain");
      answerWithCORS(res, 404, "404 Page Not Found");
      return;
    }
    if (req.method.toUpperCase() === "OPTIONS") {
      answerWithCORS(res, 200);
      res.end();
      return;
    }
    if (req.method.toUpperCase() !== "GET") {
      res.setHeader("Content-Type", "text/plain");
      answerWithCORS(res, 405, "405 Method Not Allowed");
      return;
    }

    const urlObj = routeObj[req.url];
    let data;
    if (typeof urlObj.path === "string") {
      try {
        data = fs.readFileSync(urlObj.path);
      } catch (_err) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 404, "404 Page Not Found");
        return;
      }
    } else {
      data = urlObj.data;
      if (data === undefined) {
        answerWithCORS(res, 404, "404 Page Not Found");
        return;
      }
      try {
        data = Buffer.from(data);
      } catch (_e) {}
      answerWithCORS(res, 200, data);
      return;
    }

    const rangeHeader = req.headers["Range"] || req.headers["range"];
    let isPartial = false;
    if (typeof rangeHeader === "string" && rangeHeader.startsWith("bytes=")) {
      const dataLength = data.byteLength;
      const ranges = parseRangeHeader(rangeHeader, dataLength);
      data = data.slice(ranges[0], ranges[1] + 1);
      res.setHeader(
        "Content-Range",
        `bytes ${ranges[0]}-${ranges[1]}/${dataLength}`,
      );
      isPartial = true;
    }
    if (typeof urlObj.postProcess === "function") {
      data = urlObj.postProcess(data);
    }
    if (typeof urlObj.contentType === "string") {
      res.setHeader("Content-Type", urlObj.contentType);
    }
    const responseBody = Buffer.from(data);
    const delayMs = typeof urlObj.delayMs === "number" ? urlObj.delayMs : 0;
    if (delayMs > 0) {
      setTimeout(() => {
        answerWithCORS(res, isPartial ? 206 : 200, responseBody);
      }, delayMs);
      return;
    }
    answerWithCORS(res, isPartial ? 206 : 200, responseBody);
  });

  server.on("connection", (socket) => {
    activeSockets.add(socket);
    socket.on("close", () => {
      activeSockets.delete(socket);
    });
  });

  /** @type {Promise<void>} */
  const listeningPromise = new Promise((res) => {
    server.listen(port, function () {
      console.log(
        `Test Content Server started: http://localhost:${port ?? DEFAULT_CONTENT_SERVER_PORT}`,
      );
      console.log("");
      res();
    });
  });

  return {
    listeningPromise,
    async close() {
      await stopPackagingProcess();
      const wasOpen = server.listening;
      server.closeIdleConnections?.();
      server.closeAllConnections?.();
      /** @type {Promise<void>} */
      const closed = new Promise((resolve) => {
        server.close(() => resolve());
      });
      await closed;
      for (const socket of activeSockets) {
        socket.destroy();
      }
      activeSockets.clear();
      if (wasOpen) {
        console.log("Test Content Server stopped");
      }
    },
  };
}

/**
 * @param {ServerResponse} res
 * @param {IncomingMessage} req
 * @param {string} basePath
 */
function handlePackagedLiveRequest(res, req, basePath) {
  if (req.method?.toUpperCase() === "OPTIONS") {
    answerWithCORS(res, 200);
    res.end();
    return;
  }
  if (req.method?.toUpperCase() !== "GET") {
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(res, 405, "405 Method Not Allowed");
    return;
  }

  const baseDir = DEFAULT_PACKAGED_LIVE_OS_PATH;
  const relativeUrl = (req.url ?? "").substring(basePath.length);
  prepareStaticFile(baseDir, relativeUrl).then(
    (file) => {
      if (file === null) {
        answerWithCORS(res, 404, "404 Not Found");
        return;
      }
      const mimeType =
        file.ext === "m3u8"
          ? "application/vnd.apple.mpegurl"
          : "application/octet-stream";
      const stream = fs.createReadStream(file.filePath);
      res.writeHead(200, {
        "Content-Type": mimeType,
        Connection: "close",
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Headers": "*",
        "Access-Control-Allow-Credentials": "true",
        "Access-Control-Allow-Methods": "GET, OPTIONS",
        "Cross-Origin-Resource-Policy": "cross-origin",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
      });
      stream.on("error", (err) => {
        console.error(
          `Live stream error ${req.url}:`,
          err instanceof Error ? (err.stack ?? err.message) : err,
        );
      });
      stream.pipe(res);
    },
    (err) => {
      console.error(
        `Live request failed ${req.url}:`,
        err instanceof Error ? (err.stack ?? err.message) : err,
      );
      res.setHeader("Content-Type", "text/plain");
      answerWithCORS(
        res,
        500,
        "Error: " + (err instanceof Error ? err.toString() : "Unknown Error"),
      );
    },
  );
}

/**
 * @param {ServerResponse} res
 * @param {string} contentServerBaseUrl
 * @param {EventScenarioState} state
 */
async function handleEventEndlistScenarioPlaylistRequest(
  res,
  contentServerBaseUrl,
  state,
) {
  try {
    await ensureVodRecipe("fmp4-muxed-av");
    const { ended, segmentCount } =
      getEventEndlistScenarioPublicationState(state);
    res.setHeader("Content-Type", CONTENT_TYPE_M3U8);
    answerWithCORS(
      res,
      200,
      buildEventEndlistScenarioPlaylist(contentServerBaseUrl, segmentCount, {
        ended,
      }),
    );
  } catch (error) {
    console.error(
      "Event ENDLIST scenario request failed:",
      error instanceof Error ? (error.stack ?? error.message) : error,
    );
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(
      res,
      500,
      "Error: " + (error instanceof Error ? error.toString() : "Unknown Error"),
    );
  }
}

/**
 * @param {string} contentServerBaseUrl
 * @param {number} segmentCount
 * @param {{ended: boolean}} options
 */
function buildEventEndlistScenarioPlaylist(
  contentServerBaseUrl,
  segmentCount,
  { ended },
) {
  // Reuse generated fMP4 assets so this scenario only controls manifest
  // evolution, not media generation.
  const lines = [
    "#EXTM3U",
    "#EXT-X-VERSION:7",
    "#EXT-X-TARGETDURATION:2",
    "#EXT-X-MEDIA-SEQUENCE:0",
    "#EXT-X-PLAYLIST-TYPE:EVENT",
    "#EXT-X-INDEPENDENT-SEGMENTS",
    `#EXT-X-MAP:URI="${contentServerBaseUrl}/vod/generated/fmp4-muxed-av/init.mp4"`,
  ];

  for (let i = 0; i < segmentCount; i++) {
    lines.push("#EXTINF:2.000000,");
    lines.push(
      `${contentServerBaseUrl}/vod/generated/fmp4-muxed-av/seg-${String(i).padStart(3, "0")}.m4s`,
    );
  }

  if (ended) {
    lines.push("#EXT-X-ENDLIST");
  }

  return `${lines.join("\n")}\n`;
}

/**
 * @param {EventScenarioState} state
 */
function getEventEndlistScenarioPublicationState(state) {
  const elapsedSeconds = Math.max(0, (Date.now() - state.startedAtMs) / 1000);
  const segmentCount = Math.min(
    EVENT_ENDLIST_SCENARIO_FINAL_SEGMENT_COUNT,
    EVENT_ENDLIST_SCENARIO_INITIAL_SEGMENT_COUNT +
      Math.floor(elapsedSeconds / EVENT_ENDLIST_SCENARIO_SEGMENT_DURATION_S),
  );
  const ended =
    elapsedSeconds >=
    (EVENT_ENDLIST_SCENARIO_FINAL_SEGMENT_COUNT -
      EVENT_ENDLIST_SCENARIO_INITIAL_SEGMENT_COUNT +
      1) *
      EVENT_ENDLIST_SCENARIO_SEGMENT_DURATION_S;
  return { ended, segmentCount };
}

/**
 * @param {ServerResponse} res
 * @param {IncomingMessage} req
 * @param {URL} requestUrl
 * @param {string} basePath
 */
function handlePackagedVodRequest(res, req, requestUrl, basePath) {
  if (req.method?.toUpperCase() === "OPTIONS") {
    answerWithCORS(res, 200);
    res.end();
    return;
  }
  if (req.method?.toUpperCase() !== "GET") {
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(res, 405, "405 Method Not Allowed");
    return;
  }

  const relativeUrl = requestUrl.pathname.substring(basePath.length);
  const recipeId = getVodRecipeIdFromGeneratedPath(relativeUrl);
  const relativeFilePath = getVodGeneratedRelativeFilePath(relativeUrl);
  const outputDir = recipeId === null ? null : getVodRecipeOutputDir(recipeId);

  if (recipeId === null || relativeFilePath === null || outputDir === null) {
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(res, 404, "404 Not Found");
    return;
  }

  ensureVodRecipe(recipeId).then(
    async () => {
      const file = await prepareStaticFile(outputDir, relativeFilePath);
      if (file === null) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 404, "404 Not Found");
        return;
      }
      streamPreparedFile(req, res, file);
    },
    (err) => {
      console.error(
        `VoD generated request failed ${req.url}:`,
        err instanceof Error ? (err.stack ?? err.message) : err,
      );
      res.setHeader("Content-Type", "text/plain");
      answerWithCORS(
        res,
        500,
        "Error: " + (err instanceof Error ? err.toString() : "Unknown Error"),
      );
    },
  );
}

/**
 * @param {ServerResponse} res
 * @param {IncomingMessage} req
 * @param {URL} requestUrl
 * @param {string} basePath
 * @param {string} contentServerBaseUrl
 */
function handleVodScenarioRequest(
  res,
  req,
  requestUrl,
  basePath,
  contentServerBaseUrl,
) {
  if (req.method?.toUpperCase() === "OPTIONS") {
    answerWithCORS(res, 200);
    res.end();
    return;
  }
  if (req.method?.toUpperCase() !== "GET") {
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(res, 405, "405 Method Not Allowed");
    return;
  }

  const relativeUrl = requestUrl.pathname.substring(basePath.length);
  const slashIndex = relativeUrl.indexOf("/");
  if (slashIndex <= 0 || slashIndex === relativeUrl.length - 1) {
    res.setHeader("Content-Type", "text/plain");
    answerWithCORS(res, 404, "404 Not Found");
    return;
  }

  const scenarioId = relativeUrl.substring(0, slashIndex);
  const scenarioPath = relativeUrl.substring(slashIndex + 1);
  getVodScenarioResponse(scenarioId, scenarioPath, contentServerBaseUrl).then(
    (response) => {
      if (response === null) {
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 404, "404 Not Found");
        return;
      }
      if (typeof response.contentType === "string") {
        res.setHeader("Content-Type", response.contentType);
      }
      if (response.headers !== undefined) {
        for (const [headerName, headerValue] of Object.entries(
          response.headers,
        )) {
          res.setHeader(headerName, headerValue);
        }
      }
      answerWithCORS(res, response.status ?? 200, response.body);
    },
    (err) => {
      console.error(
        `VoD scenario request failed ${req.url}:`,
        err instanceof Error ? (err.stack ?? err.message) : err,
      );
      res.setHeader("Content-Type", "text/plain");
      answerWithCORS(
        res,
        500,
        "Error: " + (err instanceof Error ? err.toString() : "Unknown Error"),
      );
    },
  );
}

/**
 * Handle the /start_packager endpoint
 * @param {ServerResponse} res
 * @param {URL} requestUrl
 */
async function handleStartPackager(res, requestUrl) {
  try {
    if (packagingProcessInfo && !packagingProcessInfo.process.killed) {
      await stopPackagingProcess();
    }

    const emitProgramDateTime =
      requestUrl.searchParams.get("emitProgramDateTime") === "1";
    const scriptPath = path.join(
      __dirname,
      "..",
      "..",
      "scripts",
      "packager",
      "main.mjs",
    );
    const packagerArgs = [
      scriptPath,
      "--no-confirmation",
      "--segment-duration",
      "2",
      "--timeshift-buffer-depth",
      "40",
      "--base-port",
      "35951",
      "--output-dir",
      DEFAULT_PACKAGED_LIVE_OS_PATH,
    ];
    if (emitProgramDateTime) {
      packagerArgs.push("--program-date-time");
    }
    const proc = spawn(process.execPath, packagerArgs, {
      stdio: ["ignore", "pipe", "pipe"], // Don't inherit stdio, capture output
      cwd: __dirname,
    });

    packagingProcessInfo = {
      process: proc,
      timeShiftBufferDepth: 40,
      segmentDuration: 2,
      playlistPath: "/live/master.m3u8",
      emitProgramDateTime,
    };
    attachPackagerLogDrain(packagingProcessInfo.process);

    packagingProcessInfo.process.on("error", (error) => {
      console.error("ERROR: Content packaging script error:", error);
      packagingProcessInfo = null;
    });

    packagingProcessInfo.process.on("exit", () => {
      packagingProcessInfo = null;
    });

    res.setHeader("Content-Type", "application/json");
    answerWithCORS(
      res,
      200,
      JSON.stringify({
        success: true,
        message: "Content packaging script started",
        info: {
          playlistPath: packagingProcessInfo.playlistPath,
          timeShiftBufferDepth: packagingProcessInfo.timeShiftBufferDepth,
          segmentDuration: packagingProcessInfo.segmentDuration,
          emitProgramDateTime,
        },
      }),
    );
  } catch (error) {
    console.error("ERROR: Failed to start content packaging script:", error);
    res.setHeader("Content-Type", "application/json");
    answerWithCORS(
      res,
      500,
      JSON.stringify({
        success: false,
        message: "Failed to start content packaging script",
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
}

/**
 * Handle the /stop_packager endpoint
 * @param {ServerResponse} res
 */
async function handleStopPackager(res) {
  try {
    if (packagingProcessInfo && !packagingProcessInfo.process.killed) {
      await stopPackagingProcess();

      res.setHeader("Content-Type", "application/json");
      answerWithCORS(
        res,
        200,
        JSON.stringify({
          success: true,
          message: "content packaging script stopped",
        }),
      );
    } else {
      res.setHeader("Content-Type", "application/json");
      answerWithCORS(
        res,
        200,
        JSON.stringify({
          success: true,
          message: "No content packaging script running",
        }),
      );
    }
  } catch (error) {
    console.error("ERROR: Failed to stop content packaging script:", error);
    res.setHeader("Content-Type", "application/json");
    answerWithCORS(
      res,
      500,
      JSON.stringify({
        success: false,
        message: "Failed to stop content packaging script",
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
}

/**
 * Add CORS headers, Content-Length, body, HTTP status and answer with the
 * Response Object given.
 * @param {ServerResponse} res
 * @param {number} status
 * @param {string | Buffer} [body]
 */
function answerWithCORS(res, status, body) {
  if (Buffer.isBuffer(body)) {
    res.setHeader("Content-Length", body.byteLength);
  } else if (typeof body === "string") {
    res.setHeader("Content-Length", Buffer.byteLength(body));
  }
  res.writeHead(status, {
    Connection: "close",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Headers": "*",
    "Access-Control-Allow-Credentials": "true",
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Cross-Origin-Resource-Policy": "cross-origin",
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
  });
  if (body !== undefined) {
    res.end(body);
  } else {
    res.end();
  }
  return;
}

/**
 * Parse value of the "Range" header into an array of two numbers, which are
 * specifically the start and end range wanted included.
 * @param {string} rangeHeader
 * @param {number} dataLength
 * @returns {[number, number]}
 */
function parseRangeHeader(rangeHeader, dataLength) {
  const rangesStr = rangeHeader.substring(6).split("-");
  if (
    (rangesStr[0] != "" && Number.isNaN(+rangesStr[0])) ||
    (rangesStr[1] != "" && Number.isNaN(+rangesStr[1]))
  ) {
    throw new Error("Invalid range request");
  }
  const rangesNb = rangesStr.map((x) => (x === "" ? null : +x));
  if (rangesNb[1] == null) {
    return [rangesNb[0] ?? 0, dataLength - 1];
  }
  if (rangesNb[0] !== null && rangesNb[1] <= rangesNb[0]) {
    return [0, 0];
  }
  if (rangesNb[0] == null || rangesNb[0] === 0) {
    if (rangesNb[1] == null) {
      return [0, dataLength - 1];
    }
    return [0, rangesNb[1]];
  }
  return [rangesNb[0], rangesNb[1]];
}

/**
 * Generate default HTML page listing the URL statically served.
 * @param {StaticUrlItem[]} urls - Information on URLs statically served.
 * @param {string} baseUrl - Root URL where those are served.
 * @returns {string} - HTML page where those URL can be inspected and browsed.
 */
function generateUrlListHtml(urls, baseUrl) {
  const html = `<!DOCTYPE html>
<html>
<head>
    <title>URLs exposed by our content server (${urls.length} items)</title>
    <style>
        body { font-family: monospace; margin: 20px; }
        .search { margin-bottom: 20px; }
        .search input { width: 300px; padding: 5px; }
        .search select { padding: 5px; margin-left: 10px; }
        .item { border: 1px solid #ccc; margin: 10px 0; padding: 10px; }
        .url { font-weight: bold; }
        .path { color: #666; }
        .type { background: #f0f0f0; padding: 2px 5px; font-size: 0.8em; }
        .extra { margin-top: 5px; font-size: 0.9em; color: #333; }
    </style>
</head>
<body>
    <h1>Pre-registered URLs (${urls.length} items)</h1>
    <p>This Website lists the URL of static assets served by our content server.<br>
       Note that many of those assets may be unplayable as they're originally intended for specific integration tests which may not even need to have the correspoding content playing.<br>
       PS: This page only includes URLs to static VoD contents. Assets dynamically created, even by this server, are not listed here.</p>

    <div class="search">
        <input type="text" id="search" placeholder="Search...">
        <select id="typeFilter">
            <option value="">All types</option>
        </select>
        <span id="count"></span>
    </div>

    <div id="results"></div>

    <script>
        const data = ${JSON.stringify(urls)};
        let filtered = data;

        // Populate content type filter
        const types = [...new Set(data.map(i => i.contentType))].sort();
        const typeFilter = document.getElementById('typeFilter');
        types.forEach(type => {
            if (!type) {
              return;
            }
            const option = document.createElement('option');
            option.value = type;
            option.textContent = type;
            typeFilter.appendChild(option);
        });

        function render() {
            const results = document.getElementById('results');
            const count = document.getElementById('count');

            count.textContent = \`(\${filtered.length} shown)\`;

            results.innerHTML = filtered.map(item => {
                const extra = Object.keys(item)
                    .filter(k => !['url', 'path', 'contentType'].includes(k))
                    .map(k => \`<div class="extra"><strong>\${k}:</strong> \${JSON.stringify(item[k])}</div>\`)
                    .join('');

                return \`<div class="item">
                    <div class="url"><a href="${baseUrl}\${item.url}">\${item.url}</a></div>
                    <div class="path">\${item.path || 'N/A'}</div>
                    <span class="type">\${item.contentType || 'unknown'}</span>
                    \${extra}
                </div>\`;
            }).join('');
        }

        function filter() {
            const search = document.getElementById('search').value.toLowerCase();
            const type = document.getElementById('typeFilter').value;

            filtered = data.filter(item => {
                const matchSearch = !search ||
                    (item.url && item.url.toLowerCase().includes(search)) ||
                    (item.path && item.path.toLowerCase().includes(search)) ||
                    (item.contentType && item.contentType.toLowerCase().includes(search));

                const matchType = !type || item.contentType === type;

                return matchSearch && matchType;
            });

            render();
        }

        document.getElementById('search').addEventListener('input', filter);
        document.getElementById('typeFilter').addEventListener('change', filter);

        render();
    </script>
</body>
</html>`;

  return html;
}

/**
 * @param {string} baseDir
 * @param {string} url
 */
async function prepareStaticFile(baseDir, url) {
  const filePath = path.resolve(baseDir, url);
  const normalizedBase = path.resolve(baseDir);
  const relative = path.relative(normalizedBase, filePath);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    return null;
  }
  for (let attempt = 0; attempt < LIVE_FILE_OPEN_RETRY_COUNT; attempt++) {
    try {
      const stats = await fs.promises.stat(filePath);
      if (stats.isDirectory()) {
        return null;
      }
      const ext = path.extname(filePath).substring(1).toLowerCase();
      return {
        ext,
        filePath,
        size: stats.size,
        mtimeMs: stats.mtimeMs,
      };
    } catch (error) {
      if (
        error instanceof Error &&
        "code" in error &&
        error.code === "ENOENT"
      ) {
        if (attempt + 1 >= LIVE_FILE_OPEN_RETRY_COUNT) {
          return null;
        }
        await sleep(LIVE_FILE_OPEN_RETRY_DELAY_MS);
        continue;
      }
      throw error;
    }
  }
  return null;
}

/**
 * @param {IncomingMessage} req
 * @param {ServerResponse} res
 * @param {PreparedFile} file
 */
function streamPreparedFile(req, res, file) {
  const mimeType = getMimeTypeForExtension(file.ext);
  const rangeHeader = req.headers["Range"] || req.headers["range"];

  if (typeof rangeHeader === "string" && rangeHeader.startsWith("bytes=")) {
    fs.promises
      .readFile(file.filePath)
      .then((data) => {
        const ranges = parseRangeHeader(rangeHeader, data.byteLength);
        const partialData = data.slice(ranges[0], ranges[1] + 1);
        res.setHeader(
          "Content-Range",
          `bytes ${ranges[0]}-${ranges[1]}/${data.byteLength}`,
        );
        if (mimeType !== undefined) {
          res.setHeader("Content-Type", mimeType);
        }
        answerWithCORS(res, 206, partialData);
      })
      .catch((error) => {
        console.error(
          `Failed to serve ranged request for ${file.filePath}:`,
          error instanceof Error ? (error.stack ?? error.message) : error,
        );
        res.setHeader("Content-Type", "text/plain");
        answerWithCORS(res, 500, "500 Internal Server Error");
      });
    return;
  }

  const stream = fs.createReadStream(file.filePath);
  res.writeHead(200, {
    ...(mimeType !== undefined ? { "Content-Type": mimeType } : {}),
    "Content-Length": file.size,
    Connection: "close",
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Headers": "*",
    "Access-Control-Allow-Credentials": "true",
    "Access-Control-Allow-Methods": "GET, OPTIONS",
  });
  stream.on("error", (err) => {
    console.error(
      `Static file stream error ${file.filePath}:`,
      err instanceof Error ? (err.stack ?? err.message) : err,
    );
  });
  stream.pipe(res);
}

/**
 * @param {string} ext
 */
function getMimeTypeForExtension(ext) {
  switch (ext) {
    case "m3u8":
      return "application/vnd.apple.mpegurl";
    case "mp4":
      return "video/mp4";
    case "m4s":
      return "video/iso.segment";
    case "ts":
      return "video/mp2t";
    case "aac":
      return "audio/aac";
    default:
      return "application/octet-stream";
  }
}

// If true, this script is called directly
if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const args = process.argv.slice(2);
  let port;
  for (let argOffset = 0; argOffset < args.length; argOffset++) {
    const currentArg = args[argOffset];
    switch (currentArg) {
      case "-h":
      case "--help":
        displayHelp();
        process.exit(0);
        break;

      case "-p":
      case "--port":
        {
          argOffset++;
          port = +args[argOffset];
          if (isNaN(port)) {
            console.error("ERROR: Given port option is not a number\n");
            displayHelp();
            process.exit(1);
          }
        }
        break;

      case "--":
        argOffset = args.length;
        break;
      default: {
        console.error('ERROR: unknown option: "' + currentArg + '"\n');
        displayHelp();
        process.exit(1);
      }
    }
  }
  try {
    createContentServer({
      port,
    }).listeningPromise.catch((err) => {
      console.error(`ERROR: ${err}\n`);
      process.exit(1);
    });
  } catch (err) {
    console.error(`ERROR: ${err}\n`);
    process.exit(1);
  }
}

function createEventEndlistScenarioState() {
  return {
    // Requests observe publication state, they do not drive it.
    startedAtMs: Date.now(),
  };
}

/**
 * @param {PackagingProcess} proc
 */
function attachPackagerLogDrain(proc) {
  if (ACTIVATE_PACKAGER_LOGS) {
    proc.stdout?.on("data", (data) => {
      console.log("Content packaging script stdout:", data.toString());
    });
    proc.stderr?.on("data", (data) => {
      console.error("Content packaging script stderr:", data.toString());
    });
  } else {
    // Unconsumed pipes can block the packager once their buffers fill.
    proc.stdout?.resume();
    proc.stderr?.resume();
  }
}

/**
 * @param {number} ms
 * @returns {Promise<void>}
 */
function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

/**
 * @param {PackagingProcess | null} proc
 */
function forceKillProcessTree(proc) {
  if (!proc || proc.pid === undefined) {
    return;
  }
  try {
    if (process.platform === "win32") {
      spawnSync("taskkill", ["/pid", String(proc.pid), "/t", "/f"], {
        stdio: "ignore",
      });
    } else {
      process.kill(-proc.pid);
    }
  } catch (_err) {
    try {
      proc.kill("SIGKILL");
    } catch (_innerErr) {
      /* process already exited */
    }
  }
}

async function stopPackagingProcess() {
  const processInfo = packagingProcessInfo;
  if (!processInfo || processInfo.process.killed) {
    packagingProcessInfo = null;
    return false;
  }

  const proc = processInfo.process;
  const exitPromise = new Promise((resolve) => {
    proc.once("exit", () => resolve(true));
    proc.once("error", () => resolve(true));
  });

  try {
    proc.kill("SIGINT");
  } catch (_err) {
    forceKillProcessTree(proc);
  }

  const forceKillTimer = setTimeout(() => {
    forceKillProcessTree(proc);
  }, 5000);
  forceKillTimer.unref?.();

  await Promise.race([exitPromise, sleep(6500)]);
  clearTimeout(forceKillTimer);
  if (packagingProcessInfo?.process === proc) {
    packagingProcessInfo = null;
  }
  return true;
}

/**
 * Display through `console.log` an helping message relative to how to run this
 * script.
 */
function displayHelp() {
  console.log(
    `server.mjs: Run the content server for integration tests.

Usage: node server.mjs [OPTIONS]

Options:
  -p, --port  Port on which served contents are available.`,
  );
}
