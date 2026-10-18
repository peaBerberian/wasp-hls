// @ts-check

import { spawn } from "child_process";
import { mkdirSync } from "fs";
import { resolve } from "path";
import launchStaticServer from "../launch_static_server.mjs";
import {
  commandExists,
  outputDirHasMediaFiles,
  cleanupMediaFiles,
} from "./utils.mjs";
import { checkPortRange, buildPortMap } from "./ports.mjs";
import { resolveGpacBinary } from "./gpac_packager.mjs";
import { buildFfmpegArgs, spawnFfmpeg } from "./ffmpeg.mjs";
import { showConfigAndConfirm, askConfirmation } from "./ui.mjs";
import {
  setPackagerProc,
  setFfmpegProc,
  createChildExitPromise,
} from "./cleanup.mjs";

/**
 * @typedef {object} PackageConfig
 * @property {number}  basePort              - First UDP port used by the pipeline.
 * @property {string}  outputDir             - Directory where packaged content is written.
 * @property {boolean} noConfirm             - Skip interactive confirmation prompts.
 * @property {number}  segmentDuration       - Segment duration in seconds.
 * @property {number}  fragmentDuration      - Fragment duration in seconds.
 * @property {number}  frameRate             - Video frame rate in fps.
 * @property {number}  timeshiftBufferDepth  - DVR window depth in seconds.
 * @property {"mpegts"|"fmp4"} mediaFormat   - HLS media output format.
 * @property {"none"|"webvtt"|"ttml"} subtitleFormat - HLS subtitle output format.
 * @property {boolean} emitProgramDateTime - Emit HLS EXT-X-PROGRAM-DATE-TIME tags.
 * @property {boolean} serve                 - Start a local HTTP static server.
 * @property {number} serveHttpPort         - HTTP port used by the static server.
 * @property {boolean} lowLatency            - Enable LL-HLS packaging when supported.
 * @property {string}  [gpacPath]            - Explicit path to the gpac binary.
 * @property {string}  tmpDir                - Directory used to cache the gpac binary.
 */

/**
 * Create and package a live HLS stream.
 *
 * Orchestrates ffmpeg (media encoding) and GPAC (HLS segmentation). Runs until
 * interrupted or until one of the child processes exits unexpectedly.
 *
 * @param {PackageConfig} config
 * @returns {Promise<void>}
 */
export async function packageLiveContent(config) {
  validateFormatConfig(config);
  validateFeatureSupport(config);

  const { ok: portRangeOk, conflictDetected: portConflictDetected } =
    checkPortRange(config.basePort);

  if (!portRangeOk) {
    throw new Error(
      `Port range starting from ${config.basePort} would exceed valid port range (1-65535).`,
    );
  }

  ensureOutputDir(config);
  await checkIfOutputContainsMediaFiles(config.outputDir, config.noConfirm);

  if (!commandExists("ffmpeg")) {
    throw new Error(
      '"ffmpeg" needs to be installed and available in your PATH to run this script',
    );
  }

  const gpacCmd = await resolveGpacBinary(config.tmpDir, config.gpacPath);

  const ports = buildPortMap(config.basePort);

  await showConfigAndConfirm(config, gpacCmd, ports, portConflictDetected);

  console.log("Starting...");
  console.log("Cleaning up any existing media files before starting...");
  cleanupMediaFiles(config.outputDir);
  ensureOutputDir(config);
  const staticServer = config.serve
    ? launchStaticServer(config.outputDir, {
        httpPort: config.serveHttpPort,
        noCache: true,
        verbose: true,
        allowCors: true,
      })
    : null;
  if (staticServer !== null) {
    await staticServer.listeningPromise;
  }

  const gpacArgs = buildGpacArgs(config, ports, config.outputDir);

  console.log(`Starting GPAC with command: ${gpacCmd}`);
  const gpac = spawn(gpacCmd, gpacArgs, { stdio: "inherit" });
  setPackagerProc(gpac);
  console.log(`GPAC started with PID: ${gpac.pid}`);
  const gpacExited = createChildExitPromise("gpac", gpac, config.outputDir);

  const ffmpegArgs = buildFfmpegArgs({
    frameRate: config.frameRate,
    segmentDuration: config.segmentDuration,
    ports,
  });
  const ffmpeg = spawnFfmpeg(ffmpegArgs);
  setFfmpegProc(ffmpeg);
  const ffmpegExited = createChildExitPromise(
    "ffmpeg",
    ffmpeg,
    config.outputDir,
  );

  try {
    // Run until interrupted or one child crashes.
    await Promise.race([ffmpegExited, gpacExited]);
  } finally {
    staticServer?.close();
  }
}

/**
 * @param {PackageConfig} config
 */
function validateFormatConfig(config) {
  if (config.mediaFormat !== "mpegts" && config.mediaFormat !== "fmp4") {
    throw new Error(`Unsupported media format: ${config.mediaFormat}`);
  }
  if (
    config.subtitleFormat !== "none" &&
    config.subtitleFormat !== "webvtt" &&
    config.subtitleFormat !== "ttml"
  ) {
    throw new Error(`Unsupported subtitle format: ${config.subtitleFormat}`);
  }
}

/**
 * @param {PackageConfig} config
 */
function validateFeatureSupport(config) {
  if (config.subtitleFormat !== "none") {
    throw new Error(
      `GPAC live packaging currently only supports --subtitle-format none. Received: ${config.subtitleFormat}`,
    );
  }
}

/**
 * @param {PackageConfig} config
 */
function ensureOutputDir(config) {
  try {
    mkdirSync(config.outputDir, { recursive: true });
    config.outputDir = resolve(config.outputDir);
  } catch {
    throw new Error(`Failed to create output directory: ${config.outputDir}`);
  }
}

/**
 * @param {string} outputDir
 * @param {boolean} noConfirm
 * @returns {Promise.<void>}
 */
async function checkIfOutputContainsMediaFiles(outputDir, noConfirm) {
  if (!outputDirHasMediaFiles(outputDir)) {
    return;
  }

  console.log("⚠️  WARNING: Output directory contains existing media files!");
  console.log(`   Directory: ${outputDir}`);
  console.log("   These files will be removed before starting.");
  console.log();

  if (!noConfirm) {
    if (!(await askConfirmation("Continue and remove existing files?"))) {
      throw new Error("Cancelled by user.");
    }
    console.log();
  }
}

/**
 * Assemble the full GPAC argument list.
 *
 * @param {PackageConfig} config
 * @param {ReturnType<import("./ports.mjs").buildPortMap>} ports
 * @param {string} outputDir
 * @returns {string[]}
 */
export function buildGpacArgs(config, ports, outputDir) {
  const out = outputDir;
  const inputs = [
    {
      port: ports.p720,
      playlist: "h264_720p.m3u8",
      representation: "h264_720p",
      bitrate: "2500000",
    },
    {
      port: ports.p480,
      playlist: "h264_480p.m3u8",
      representation: "h264_480p",
      bitrate: "1200000",
    },
    {
      port: ports.p360,
      playlist: "h264_360p.m3u8",
      representation: "h264_360p",
      bitrate: "600000",
    },
    {
      port: ports.audio1,
      playlist: "audio_eng.m3u8",
      representation: "English",
      bitrate: "128000",
      language: "en",
      group: "audio",
    },
    {
      port: ports.audio2,
      playlist: "audio_fra.m3u8",
      representation: "French",
      bitrate: "128000",
      language: "fr",
      group: "audio",
    },
    {
      port: ports.audio3,
      playlist: "audio_arm.m3u8",
      representation: "Armenian",
      bitrate: "128000",
      language: "hy",
      group: "audio",
    },
  ];

  const inputArgs = inputs.flatMap((input) => {
    let source =
      // GPAC expects a trailing "/" before filter options on socket URLs.
      `udp://127.0.0.1:${input.port}/` +
      `:#HLSPL=${input.playlist}` +
      `:#Representation=${input.representation}` +
      `:#Bitrate=${input.bitrate}`;

    if (input.group) {
      source += `:#HLSGroup=${input.group}`;
    }
    if (input.language) {
      source += `:#Language=${input.language}`;
    }

    return ["-i", source];
  });

  const outputOptions = [
    `${out}/master.m3u8` +
      // Publish complete files so HTTP requests cannot read an empty playlist during a rewrite.
      `:atomic=true` +
      `:profile=live` +
      `:dmode=dynamic` +
      `:segdur=${config.segmentDuration}` +
      `:cdur=${config.fragmentDuration}` +
      `:refresh=${config.segmentDuration}` +
      `:tsb=${config.timeshiftBufferDepth}` +
      (config.mediaFormat === "mpegts" ? ":muxtype=ts" : "") +
      (config.emitProgramDateTime ? ":hlsc" : "") +
      (config.lowLatency ? ":llhls=br" : ""),
  ];

  return [...inputArgs, "-o", ...outputOptions];
}
