// @ts-check

import { spawn } from "child_process";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import {
  TEXT_TRACK_LABEL,
  TEXT_TRACK_SEGMENT_PREFIX,
  TEXT_TRACK_CUE_SPACING,
  TEXT_TRACK_CUE_DURATION,
  TEXT_TRACK_INITIAL_AHEAD_DURATION,
} from "./constants.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * Build the list of text-track asset descriptors.
 *
 * Produces one English WebVTT track delivered over UDP.
 *
 * @param {{ text1: number }} ports
 * @param {"none"|"webvtt"|"ttml"} subtitleFormat
 * @returns {Array<{ sourcePath: string, segmentPrefix: string,
 *                   inputFormat: string, liveWriterMode: string, port: number,
 *                   playlistName: string, readinessFiles: string[],
 *                   outputFields: (outputDir: string) => string }>}
 */
export function createTextTrackAssets(ports, subtitleFormat) {
  if (subtitleFormat === "none") {
    return [];
  }

  const commonAssetData = {
    sourcePath: `udp://127.0.0.1:${ports.text1}`,
    inputFormat: "webvtt",
    liveWriterMode: "webvtt",
    port: ports.text1,
    playlistName: "text_en.m3u8",
  };

  if (subtitleFormat === "ttml") {
    return [
      {
        ...commonAssetData,
        segmentPrefix: TEXT_TRACK_SEGMENT_PREFIX + "_ttml",
        readinessFiles: [`${TEXT_TRACK_SEGMENT_PREFIX}_ttml_init.mp4`],
        outputFields: (outputDir) =>
          `init_segment=${outputDir}/${TEXT_TRACK_SEGMENT_PREFIX}_ttml_init.mp4,` +
          `segment_template=${outputDir}/${TEXT_TRACK_SEGMENT_PREFIX}_ttml_$Number$.m4s,` +
          "output_format=ttml+mp4",
      },
    ];
  }

  return [
    {
      ...commonAssetData,
      segmentPrefix: TEXT_TRACK_SEGMENT_PREFIX + "_vtt",
      readinessFiles: [`${TEXT_TRACK_SEGMENT_PREFIX}_vtt_1.vtt`],
      outputFields: (outputDir) =>
        `segment_template=${outputDir}/${TEXT_TRACK_SEGMENT_PREFIX}_vtt_$Number$.vtt`,
    },
  ];
}

/**
 * Spawn a UDP writer process for each text-track asset.
 * Each writer runs as a separate Node.js script file, receiving its
 * configuration through environment variables — no inline script strings.
 *
 * @param {ReturnType<typeof createTextTrackAssets>} textTrackAssets
 * @param {number} segmentDuration
 * @returns {import("child_process").ChildProcess[]}
 */
export function startLiveTextTrackWriters(textTrackAssets, segmentDuration) {
  /** @type {Record<string, string>} */
  const sharedEnv = {
    ...process.env,
    TT_LABEL: TEXT_TRACK_LABEL,
    TT_CUE_SPACING: String(TEXT_TRACK_CUE_SPACING),
    TT_CUE_DURATION: String(TEXT_TRACK_CUE_DURATION),
    TT_INITIAL_AHEAD: String(TEXT_TRACK_INITIAL_AHEAD_DURATION),
    TT_SEGMENT_DURATION: String(segmentDuration),
  };

  return textTrackAssets.map((asset) => {
    const workerScript = join(
      __dirname,
      `live_${asset.liveWriterMode}_writer.mjs`,
    );
    return spawn(process.execPath, [workerScript], {
      stdio: "inherit",
      env: { ...sharedEnv, TT_PORT: String(asset.port) },
    });
  });
}
