/* eslint-env node */

import { spawn } from "child_process";
import * as fs from "fs";
import { createHash } from "crypto";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const GENERATED_VOD_ROOT = path.join(
  __dirname,
  "..",
  "..",
  "tmp",
  "testcontents",
  "vod",
);

/**
 * @typedef {{id: string, playlistName: string, durationSeconds: number,
 * segmentDurationSeconds: number, frameRate: number, videoSize: string,
 * videoBitrate: string, audioBitrate: string, audioFrequency: number,
 * streams?: "video" | "audio"} &
 * ({segmentType: "fmp4", segmentExtension: "m4s", initFileName: string} |
 * {segmentType: "mpegts", segmentExtension: "ts"})} VodRecipe
 * @typedef {{outputDir: string, playlistPath: string}} GeneratedRecipe
 * @typedef {{baseUrl: string}} PlaylistContext
 * @typedef {PlaylistContext & {scenarioId: string,
 * forRecipe: (recipeId: string) => PlaylistContext}} ScenarioContext
 * @typedef {{body: string | Buffer, contentType?: string, status?: number,
 * headers?: Record<string, string>}} FixtureResponse
 * @typedef {{entryPath: string, recipeId: string,
 * getFile: (relativePath: string, context: ScenarioContext) =>
 * Promise<FixtureResponse | null>}} VodScenario
 * @typedef {{schemaVersion: number, recipeId: string, fingerprint: string}} RecipeMetadata
 */

const RECIPE_SCHEMA_VERSION = 1;
const RECIPE_METADATA_FILE = ".recipe.json";
/** @type {Map<string, Promise<GeneratedRecipe>>} */
const generationPromises = new Map();

const CONTENT_TYPE_M3U8 = "application/vnd.apple.mpegurl";

/** @type {Record<string, VodRecipe>} */
const RECIPES = {
  "fmp4-abr": {
    id: "fmp4-abr",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 60,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 880,
  },
  "fmp4-muxed-av": {
    id: "fmp4-muxed-av",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 880,
  },
  "mpegts-muxed-av": {
    id: "mpegts-muxed-av",
    playlistName: "main.m3u8",
    segmentType: "mpegts",
    segmentExtension: "ts",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 660,
  },
  "fmp4-video-only": {
    id: "fmp4-video-only",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 880,
    streams: "video",
  },
  "fmp4-video-only-low": {
    id: "fmp4-video-only-low",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "640x360",
    videoBitrate: "800k",
    audioBitrate: "128k",
    audioFrequency: 880,
    streams: "video",
  },
  "fmp4-video-only-mid": {
    id: "fmp4-video-only-mid",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 880,
    streams: "video",
  },
  "fmp4-video-only-high": {
    id: "fmp4-video-only-high",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "1280x720",
    videoBitrate: "2800k",
    audioBitrate: "128k",
    audioFrequency: 880,
    streams: "video",
  },
  "fmp4-audio-en": {
    id: "fmp4-audio-en",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 440,
    streams: "audio",
  },
  "fmp4-audio-fr": {
    id: "fmp4-audio-fr",
    playlistName: "main.m3u8",
    segmentType: "fmp4",
    segmentExtension: "m4s",
    initFileName: "init.mp4",
    durationSeconds: 12,
    segmentDurationSeconds: 2,
    frameRate: 24,
    videoSize: "960x540",
    videoBitrate: "1600k",
    audioBitrate: "128k",
    audioFrequency: 554.37,
    streams: "audio",
  },
};

/** @type {Record<string, VodScenario>} */
const SCENARIOS = {
  "fmp4-direct-media": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        await readGeneratedMediaPlaylist("fmp4-muxed-av"),
        context,
      );
    },
  },
  "fmp4-direct-media-redirect": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath === "playlist.m3u8") {
        return createRedirectResponse("redirected/playlist.m3u8");
      }
      if (relativePath === "redirected/playlist.m3u8") {
        return createRelativeMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
        );
      }
      return await readGeneratedRecipeAssetResponse(
        "fmp4-muxed-av",
        stripPrefix(relativePath, "redirected/"),
      );
    },
  },
  "mpegts-direct-media": {
    entryPath: "playlist.m3u8",
    recipeId: "mpegts-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        await readGeneratedMediaPlaylist("mpegts-muxed-av"),
        context,
      );
    },
  },
  "fmp4-multivariant-no-codecs": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-multivariant-master-redirect": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath === "master.m3u8") {
        return createRedirectResponse("redirected/master.m3u8");
      }
      if (relativePath === "redirected/master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "redirected/variant.m3u8") {
        return createRelativeMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
        );
      }
      return await readGeneratedRecipeAssetResponse(
        "fmp4-muxed-av",
        stripPrefix(relativePath, "redirected/"),
      );
    },
  },
  "mpegts-multivariant-no-codecs": {
    entryPath: "master.m3u8",
    recipeId: "mpegts-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:3\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("mpegts-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-player-api": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        await readGeneratedMediaPlaylist("fmp4-muxed-av"),
        context,
      );
    },
  },
  "fmp4-direct-media-endlist-without-playlist-type": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        removeFirstLineMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXT-X-PLAYLIST-TYPE:"),
        ),
        context,
      );
    },
  },
  "fmp4-player-api-program-date-time": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectProgramDateTime(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          "2024-01-02T03:04:05.000Z",
        ),
        context,
      );
    },
  },
  "fmp4-direct-media-byterange": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return {
        body: await createFmp4ByteRangePlaylist(context),
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
  "fmp4-player-api-ext-x-start": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectExtXStart(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          "#EXT-X-START:TIME-OFFSET=6,PRECISE=YES",
        ),
        context,
      );
    },
  },
  "fmp4-player-api-ext-x-start-imprecise": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectExtXStart(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          "#EXT-X-START:TIME-OFFSET=5,PRECISE=NO",
        ),
        context,
      );
    },
  },
  "fmp4-alt-audio": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-video-only",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"\n' +
            '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="French",LANGUAGE="fr",DEFAULT=NO,AUTOSELECT=YES,URI="audio-fr.m3u8"\n' +
            '#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540,AUDIO="audio-main"\n' +
            "video.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "video.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-video-only"),
          context.forRecipe("fmp4-video-only"),
        );
      }
      if (relativePath === "audio-en.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-audio-en"),
          context.forRecipe("fmp4-audio-en"),
        );
      }
      if (relativePath === "audio-fr.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-audio-fr"),
          context.forRecipe("fmp4-audio-fr"),
        );
      }
      return null;
    },
  },
  "fmp4-multivariant-alt-audio": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-video-only-mid",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,URI="audio-en.m3u8"\n' +
            '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="French",LANGUAGE="fr",DEFAULT=NO,AUTOSELECT=YES,URI="audio-fr.m3u8"\n' +
            '#EXT-X-STREAM-INF:BANDWIDTH=928000,RESOLUTION=640x360,AUDIO="audio-main"\n' +
            "video-low.m3u8\n" +
            '#EXT-X-STREAM-INF:BANDWIDTH=1728000,RESOLUTION=960x540,AUDIO="audio-main"\n' +
            "video-mid.m3u8\n" +
            '#EXT-X-STREAM-INF:BANDWIDTH=2928000,RESOLUTION=1280x720,AUDIO="audio-main"\n' +
            "video-high.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "video-low.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-video-only-low"),
          context.forRecipe("fmp4-video-only-low"),
        );
      }
      if (relativePath === "video-mid.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-video-only-mid"),
          context.forRecipe("fmp4-video-only-mid"),
        );
      }
      if (relativePath === "video-high.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-video-only-high"),
          context.forRecipe("fmp4-video-only-high"),
        );
      }
      if (relativePath === "audio-en.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-audio-en"),
          context.forRecipe("fmp4-audio-en"),
        );
      }
      if (relativePath === "audio-fr.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-audio-fr"),
          context.forRecipe("fmp4-audio-fr"),
        );
      }
      return null;
    },
  },
  "fmp4-shared-audio-muxed": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-INDEPENDENT-SEGMENTS\n" +
            '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="Main",DEFAULT=YES,AUTOSELECT=YES\n' +
            '#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540,AUDIO="audio-main"\n' +
            "media.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "media.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context.forRecipe("fmp4-muxed-av"),
        );
      }
      return null;
    },
  },
  "fmp4-error-missing-target-duration": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        removeFirstLineMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXT-X-TARGETDURATION:"),
        ),
        context,
      );
    },
  },
  "fmp4-error-unparsable-extinf": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        replaceFirstLineMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXTINF:"),
          "#EXTINF:not-a-number,",
        ),
        context,
      );
    },
  },
  "fmp4-error-uri-missing-in-map": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        replaceFirstLineMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXT-X-MAP:"),
          '#EXT-X-MAP:BYTERANGE="720@0"',
        ),
        context,
      );
    },
  },
  "fmp4-error-uri-without-extinf": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        removeFirstLineMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXTINF:"),
        ),
        context,
      );
    },
  },
  "fmp4-error-unparsable-byterange": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectLineBeforeFirstMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXTINF:"),
          "#EXT-X-BYTERANGE:not-a-range",
        ),
        context,
      );
    },
  },
  "fmp4-error-media-variable-definition": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectLineAfterExtM3u(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          '#EXT-X-DEFINE:IMPORT="cdn"',
        ),
        context,
      );
    },
  },
  "fmp4-error-media-duplicate-singleton": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectLineAfterFirstMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXT-X-VERSION:"),
          "#EXT-X-VERSION:8",
        ),
        context,
      );
    },
  },
  "fmp4-error-media-conflicting-tag-types": {
    entryPath: "playlist.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath !== "playlist.m3u8") {
        return null;
      }
      return createMediaPlaylistResponse(
        injectLineAfterFirstMatching(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          (line) => line.startsWith("#EXT-X-TARGETDURATION:"),
          '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="English",URI="audio.m3u8"',
        ),
        context,
      );
    },
  },
  "fmp4-error-master-missing-uri-after-variant": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath !== "master.m3u8") {
        return null;
      }
      return {
        body:
          "#EXTM3U\n" +
          "#EXT-X-VERSION:7\n" +
          "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n",
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
  "fmp4-error-master-missing-uri-after-variant-comment": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath !== "master.m3u8") {
        return null;
      }
      return {
        body:
          "#EXTM3U\n" +
          "#EXT-X-VERSION:7\n" +
          "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
          "#comment\n",
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
  "fmp4-error-master-variant-missing-bandwidth": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-STREAM-INF:RESOLUTION=960x540\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-error-master-duplicate-singleton": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-VERSION:8\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-error-master-conflicting-tag-types": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
            "variant.m3u8\n" +
            "#EXT-X-TARGETDURATION:4\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-error-master-invalid-value": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000,AUDIO=audio-main\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-error-master-missing-required-attribute": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            "#EXT-X-VERSION:7\n" +
            '#EXT-X-MEDIA:GROUP-ID="audio-main",NAME="English",URI="audio.m3u8"\n' +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      if (relativePath === "audio.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-audio-en"),
          context.forRecipe("fmp4-audio-en"),
        );
      }
      return null;
    },
  },
  "fmp4-error-master-variable-definition": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath, context) {
      if (relativePath === "master.m3u8") {
        return {
          body:
            "#EXTM3U\n" +
            '#EXT-X-DEFINE:QUERYPARAM="token"\n' +
            "#EXT-X-STREAM-INF:BANDWIDTH=1900000\n" +
            "variant.m3u8\n",
          contentType: CONTENT_TYPE_M3U8,
        };
      }
      if (relativePath === "variant.m3u8") {
        return createMediaPlaylistResponse(
          await readGeneratedMediaPlaylist("fmp4-muxed-av"),
          context,
        );
      }
      return null;
    },
  },
  "fmp4-error-master-without-variant": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath !== "master.m3u8") {
        return null;
      }
      return {
        body:
          "#EXTM3U\n" +
          "#EXT-X-VERSION:7\n" +
          '#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio-main",NAME="English",URI="audio.m3u8"\n',
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
  "fmp4-error-master-other-parsing-error": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath !== "master.m3u8") {
        return null;
      }
      return {
        body:
          "#EXTM3U\n" +
          '#EXT-X-DEFINE:IMPORT="cdn"\n' +
          "#EXT-X-STREAM-INF:BANDWIDTH=1900000\n" +
          "variant.m3u8\n",
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
  "fmp4-error-top-level-missing-extm3u": {
    entryPath: "master.m3u8",
    recipeId: "fmp4-muxed-av",
    async getFile(relativePath) {
      if (relativePath !== "master.m3u8") {
        return null;
      }
      return {
        body:
          "#EXT-X-VERSION:7\n" +
          "#EXT-X-STREAM-INF:BANDWIDTH=1900000,RESOLUTION=960x540\n" +
          "variant.m3u8\n",
        contentType: CONTENT_TYPE_M3U8,
      };
    },
  },
};

/**
 * @param {string} scenarioId
 * @param {string} relativePath
 * @param {string} serverBaseUrl
 */
export async function getVodScenarioResponse(
  scenarioId,
  relativePath,
  serverBaseUrl,
) {
  const scenario = SCENARIOS[scenarioId];
  if (scenario === undefined) {
    return null;
  }
  await ensureVodRecipe(scenario.recipeId);
  const normalizedPath = normalizeScenarioRelativePath(relativePath);
  if (normalizedPath === null) {
    return null;
  }
  return await scenario.getFile(normalizedPath, {
    baseUrl: `${serverBaseUrl}/vod/generated/${scenario.recipeId}/`,
    scenarioId,
    forRecipe(recipeId) {
      return {
        baseUrl: `${serverBaseUrl}/vod/generated/${recipeId}/`,
      };
    },
  });
}

export async function ensureDefaultVodFixtures() {
  await Promise.all(
    Object.keys(RECIPES).map((recipeId) => ensureVodRecipe(recipeId)),
  );
}

/**
 * @param {string} relativePath
 */
export function getVodRecipeIdFromGeneratedPath(relativePath) {
  const normalizedPath = normalizeScenarioRelativePath(relativePath);
  if (normalizedPath === null) {
    return null;
  }
  const slashIndex = normalizedPath.indexOf("/");
  const recipeId =
    slashIndex === -1
      ? normalizedPath
      : normalizedPath.substring(0, slashIndex);
  if (RECIPES[recipeId] === undefined) {
    return null;
  }
  return recipeId;
}

/**
 * @param {string} relativePath
 */
export function getVodGeneratedRelativeFilePath(relativePath) {
  const normalizedPath = normalizeScenarioRelativePath(relativePath);
  if (normalizedPath === null) {
    return null;
  }
  const slashIndex = normalizedPath.indexOf("/");
  if (slashIndex === -1 || slashIndex === normalizedPath.length - 1) {
    return null;
  }
  return normalizedPath.substring(slashIndex + 1);
}

/**
 * @param {string} recipeId
 */
export function getVodRecipeOutputDir(recipeId) {
  const recipe = RECIPES[recipeId];
  return recipe === undefined ? null : path.join(GENERATED_VOD_ROOT, recipe.id);
}

/**
 * @param {string} recipeId
 */
export async function ensureVodRecipe(recipeId) {
  const recipe = RECIPES[recipeId];
  if (recipe === undefined) {
    throw new Error(`Unknown VoD recipe: ${recipeId}`);
  }

  const currentPromise = generationPromises.get(recipeId);
  if (currentPromise !== undefined) {
    return await currentPromise;
  }

  const generationPromise = ensureVodRecipeInner(recipe).finally(() => {
    generationPromises.delete(recipeId);
  });
  generationPromises.set(recipeId, generationPromise);
  return await generationPromise;
}

/**
 * @param {VodRecipe} recipe
 */
async function ensureVodRecipeInner(recipe) {
  const outputDir = path.join(GENERATED_VOD_ROOT, recipe.id);
  const expectedFingerprint = buildRecipeFingerprint(recipe);
  const metadataPath = path.join(outputDir, RECIPE_METADATA_FILE);
  const currentMetadata = await readRecipeMetadata(metadataPath);

  if (currentMetadata?.fingerprint === expectedFingerprint) {
    return {
      outputDir,
      playlistPath: path.join(outputDir, recipe.playlistName),
    };
  }

  await fs.promises.rm(outputDir, { recursive: true, force: true });
  await fs.promises.mkdir(outputDir, { recursive: true });
  await runFfmpeg(buildRecipeFfmpegArgs(recipe), recipe.id);

  await fs.promises.writeFile(
    metadataPath,
    JSON.stringify(
      {
        schemaVersion: RECIPE_SCHEMA_VERSION,
        recipeId: recipe.id,
        fingerprint: expectedFingerprint,
      },
      null,
      2,
    ),
  );

  return {
    outputDir,
    playlistPath: path.join(outputDir, recipe.playlistName),
  };
}

/**
 * @param {VodRecipe} recipe
 */
function buildRecipeFingerprint(recipe) {
  return createHash("sha1")
    .update(
      JSON.stringify({
        schemaVersion: RECIPE_SCHEMA_VERSION,
        recipe,
      }),
    )
    .digest("hex");
}

/**
 * @param {string} metadataPath
 * @returns {Promise<RecipeMetadata | null>}
 */
async function readRecipeMetadata(metadataPath) {
  try {
    const metadata = await fs.promises.readFile(metadataPath, "utf8");
    return JSON.parse(metadata);
  } catch (_error) {
    return null;
  }
}

/**
 * @param {VodRecipe} recipe
 */
function buildRecipeFfmpegArgs(recipe) {
  const gop = recipe.frameRate * recipe.segmentDurationSeconds;
  const outputPlaylistPath = recipe.playlistName;
  const segmentFilename = `seg-%03d.${recipe.segmentExtension}`;

  return [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    `testsrc2=size=${recipe.videoSize}:rate=${recipe.frameRate}`,
    "-f",
    "lavfi",
    "-i",
    `sine=frequency=${recipe.audioFrequency}:sample_rate=48000`,
    "-t",
    String(recipe.durationSeconds),
    ...(recipe.streams === "audio"
      ? []
      : [
          "-map",
          "0:v:0",
          "-c:v",
          "libx264",
          "-preset",
          "veryfast",
          "-pix_fmt",
          "yuv420p",
          "-b:v",
          recipe.videoBitrate,
          "-g",
          String(gop),
          "-keyint_min",
          String(gop),
          "-sc_threshold",
          "0",
        ]),
    ...(recipe.streams === "video"
      ? []
      : [
          "-map",
          "1:a:0",
          "-c:a",
          "aac",
          "-b:a",
          recipe.audioBitrate,
          "-ac",
          "2",
          "-ar",
          "48000",
        ]),
    "-f",
    "hls",
    "-hls_time",
    String(recipe.segmentDurationSeconds),
    "-hls_list_size",
    "0",
    "-hls_playlist_type",
    "vod",
    "-hls_flags",
    "independent_segments",
    ...(recipe.segmentType === "fmp4"
      ? [
          "-hls_segment_type",
          "fmp4",
          "-hls_fmp4_init_filename",
          recipe.initFileName,
        ]
      : []),
    "-hls_segment_filename",
    segmentFilename,
    outputPlaylistPath,
  ];
}

/**
 * @param {string[]} args
 * @param {string} recipeId
 * @returns {Promise<void>}
 */
function runFfmpeg(args, recipeId) {
  return new Promise((resolve, reject) => {
    const outputDir = getVodRecipeOutputDir(recipeId);
    if (outputDir === null) {
      reject(new Error(`Unknown VoD recipe: ${recipeId}`));
      return;
    }
    const proc = spawn("ffmpeg", args, {
      cwd: outputDir,
      stdio: ["ignore", "ignore", "pipe"],
    });
    let stderr = "";

    proc.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    proc.once("error", (error) => {
      reject(
        new Error(
          `Failed to start ffmpeg for VoD recipe "${recipeId}": ${error.message}`,
        ),
      );
    });

    proc.once("exit", (exitCode) => {
      if (exitCode === 0) {
        resolve();
        return;
      }
      reject(
        new Error(
          `ffmpeg failed while generating VoD recipe "${recipeId}" with exit code ${String(exitCode)}.\n${stderr}`,
        ),
      );
    });
  });
}

/**
 * @param {string} recipeId
 */
async function readGeneratedMediaPlaylist(recipeId) {
  const recipe = RECIPES[recipeId];
  const outputDir = getVodRecipeOutputDir(recipeId);
  if (recipe === undefined || outputDir === null) {
    throw new Error(`Unknown VoD recipe: ${recipeId}`);
  }
  return await fs.promises.readFile(
    path.join(outputDir, recipe.playlistName),
    "utf8",
  );
}

/**
 * @param {string} playlistText
 * @param {PlaylistContext} context
 */
function createMediaPlaylistResponse(playlistText, context) {
  return {
    body: rewriteMediaPlaylistUrls(playlistText, context.baseUrl),
    contentType: CONTENT_TYPE_M3U8,
  };
}

/**
 * @param {string} playlistText
 */
function createRelativeMediaPlaylistResponse(playlistText) {
  return {
    body: playlistText,
    contentType: CONTENT_TYPE_M3U8,
  };
}

/**
 * @param {string} location
 */
function createRedirectResponse(location) {
  return {
    status: 302,
    headers: {
      Location: location,
    },
    body: "",
  };
}

/**
 * @param {string} recipeId
 * @param {string | null} relativePath
 */
async function readGeneratedRecipeAssetResponse(recipeId, relativePath) {
  if (relativePath === null) {
    return null;
  }
  const outputDir = getVodRecipeOutputDir(recipeId);
  if (outputDir === null) {
    return null;
  }
  const filePath = path.join(outputDir, relativePath);
  if (!filePath.startsWith(outputDir + path.sep) && filePath !== outputDir) {
    return null;
  }
  try {
    const body = await fs.promises.readFile(filePath);
    return {
      body,
      contentType: getMimeTypeForFilePath(filePath),
    };
  } catch {
    return null;
  }
}

/**
 * @param {string} playlistText
 * @param {string} startTagLine
 */
function injectExtXStart(playlistText, startTagLine) {
  const lines = playlistText.split("\n");
  if (lines[0]?.trim() !== "#EXTM3U") {
    throw new Error("Unexpected playlist format: missing #EXTM3U header");
  }
  return [lines[0], startTagLine, ...lines.slice(1)].join("\n");
}

/**
 * @param {string} filePath
 */
function getMimeTypeForFilePath(filePath) {
  switch (path.extname(filePath).slice(1)) {
    case "m3u8":
      return CONTENT_TYPE_M3U8;
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

/**
 * @param {string} playlistText
 * @param {string} iso8601DateTime
 */
function injectProgramDateTime(playlistText, iso8601DateTime) {
  const lines = playlistText.split("\n");
  const extInfIndex = lines.findIndex((line) =>
    line.trim().startsWith("#EXTINF:"),
  );
  if (extInfIndex < 0) {
    throw new Error("Unexpected playlist format: missing #EXTINF tag");
  }
  return [
    ...lines.slice(0, extInfIndex),
    `#EXT-X-PROGRAM-DATE-TIME:${iso8601DateTime}`,
    ...lines.slice(extInfIndex),
  ].join("\n");
}

/**
 * @param {string} playlistText
 * @param {string} insertedLine
 */
function injectLineAfterExtM3u(playlistText, insertedLine) {
  const lines = playlistText.split("\n");
  if (lines[0]?.trim() !== "#EXTM3U") {
    throw new Error("Unexpected playlist format: missing #EXTM3U header");
  }
  return [lines[0], insertedLine, ...lines.slice(1)].join("\n");
}

/**
 * @param {string} playlistText
 * @param {(line: string) => boolean} predicate
 * @param {string} insertedLine
 */
function injectLineAfterFirstMatching(playlistText, predicate, insertedLine) {
  const lines = playlistText.split("\n");
  const index = lines.findIndex((line) => predicate(line.trim()));
  if (index < 0) {
    throw new Error("Unexpected playlist format: target line not found");
  }
  return [
    ...lines.slice(0, index + 1),
    insertedLine,
    ...lines.slice(index + 1),
  ].join("\n");
}

/**
 * @param {string} playlistText
 * @param {(line: string) => boolean} predicate
 * @param {string} insertedLine
 */
function injectLineBeforeFirstMatching(playlistText, predicate, insertedLine) {
  const lines = playlistText.split("\n");
  const index = lines.findIndex((line) => predicate(line.trim()));
  if (index < 0) {
    throw new Error("Unexpected playlist format: target line not found");
  }
  return [...lines.slice(0, index), insertedLine, ...lines.slice(index)].join(
    "\n",
  );
}

/**
 * @param {string} playlistText
 * @param {(line: string) => boolean} predicate
 * @param {string} replacementLine
 */
function replaceFirstLineMatching(playlistText, predicate, replacementLine) {
  const lines = playlistText.split("\n");
  const index = lines.findIndex((line) => predicate(line.trim()));
  if (index < 0) {
    throw new Error("Unexpected playlist format: target line not found");
  }
  lines[index] = replacementLine;
  return lines.join("\n");
}

/**
 * @param {string} playlistText
 * @param {(line: string) => boolean} predicate
 */
function removeFirstLineMatching(playlistText, predicate) {
  const lines = playlistText.split("\n");
  const index = lines.findIndex((line) => predicate(line.trim()));
  if (index < 0) {
    throw new Error("Unexpected playlist format: target line not found");
  }
  lines.splice(index, 1);
  return lines.join("\n");
}

/**
 * @param {string} playlistText
 * @param {string} baseUrl
 */
function rewriteMediaPlaylistUrls(playlistText, baseUrl) {
  return playlistText
    .split("\n")
    .map((line) => rewritePlaylistLine(line, baseUrl))
    .join("\n");
}

/**
 * @param {string} line
 * @param {string} baseUrl
 */
function rewritePlaylistLine(line, baseUrl) {
  const trimmedLine = line.trim();
  if (trimmedLine.length === 0) {
    return line;
  }
  if (!trimmedLine.startsWith("#")) {
    return toAbsoluteUrl(trimmedLine, baseUrl);
  }
  if (trimmedLine.startsWith("#EXT-X-MAP:")) {
    return trimmedLine.replace(/URI="([^"]+)"/u, (_fullMatch, uri) => {
      return `URI="${toAbsoluteUrl(uri, baseUrl)}"`;
    });
  }
  return line;
}

/**
 * @param {string} relativeUrl
 * @param {string} baseUrl
 */
function toAbsoluteUrl(relativeUrl, baseUrl) {
  return new URL(relativeUrl, baseUrl).href;
}

/**
 * @param {PlaylistContext} context
 */
async function createFmp4ByteRangePlaylist(context) {
  const recipeId = "fmp4-muxed-av";
  const playlistText = await readGeneratedMediaPlaylist(recipeId);
  const outputDir = getVodRecipeOutputDir(recipeId);
  if (outputDir === null) {
    throw new Error(`Unknown VoD recipe: ${recipeId}`);
  }

  const segmentFileName = "byterange-segments.m4s";
  const segmentOutputPath = path.join(outputDir, segmentFileName);
  const playlist = await buildByteRangeMediaPlaylist(
    playlistText,
    outputDir,
    context.baseUrl,
    segmentFileName,
  );
  await fs.promises.writeFile(segmentOutputPath, playlist.segmentData);

  return playlist.text;
}

/**
 * @param {string} playlistText
 * @param {string} outputDir
 * @param {string} baseUrl
 * @param {string} combinedSegmentFileName
 */
async function buildByteRangeMediaPlaylist(
  playlistText,
  outputDir,
  baseUrl,
  combinedSegmentFileName,
) {
  const lines = playlistText.split("\n");
  const playlistLines = [];
  let pendingDurationLine = null;
  const segmentUri = toAbsoluteUrl(combinedSegmentFileName, baseUrl);
  const segmentDataChunks = [];
  let currentOffset = 0;

  for (const line of lines) {
    const trimmedLine = line.trim();
    if (trimmedLine.startsWith("#EXTINF:")) {
      pendingDurationLine = trimmedLine;
      continue;
    }

    if (trimmedLine.length === 0) {
      continue;
    }

    if (pendingDurationLine !== null && !trimmedLine.startsWith("#")) {
      const segmentData = await fs.promises.readFile(
        path.join(outputDir, trimmedLine),
      );
      segmentDataChunks.push(segmentData);
      playlistLines.push(pendingDurationLine);
      playlistLines.push(
        `#EXT-X-BYTERANGE:${segmentData.byteLength}@${currentOffset}`,
      );
      playlistLines.push(segmentUri);
      currentOffset += segmentData.byteLength;
      pendingDurationLine = null;
      continue;
    }

    if (trimmedLine.startsWith("#EXT-X-MAP:")) {
      playlistLines.push(
        trimmedLine.replace(/URI="([^"]+)"/u, (_fullMatch, uri) => {
          return `URI="${toAbsoluteUrl(uri, baseUrl)}"`;
        }),
      );
      continue;
    }

    if (!trimmedLine.startsWith("#")) {
      playlistLines.push(toAbsoluteUrl(trimmedLine, baseUrl));
      continue;
    }

    playlistLines.push(trimmedLine);
  }

  if (pendingDurationLine !== null) {
    throw new Error(
      "Malformed generated playlist: dangling EXTINF without URI",
    );
  }

  return {
    text: playlistLines.join("\n"),
    segmentData: Buffer.concat(segmentDataChunks),
  };
}

/**
 * @param {string} relativePath
 */
function normalizeScenarioRelativePath(relativePath) {
  const normalizedPath = relativePath.replace(/^\/+/u, "");
  if (
    normalizedPath.length === 0 ||
    normalizedPath.includes("\0") ||
    normalizedPath.split("/").some((segment) => segment === "..")
  ) {
    return null;
  }
  return normalizedPath;
}

/**
 * @param {string} value
 * @param {string} prefix
 */
function stripPrefix(value, prefix) {
  return value.startsWith(prefix) ? value.substring(prefix.length) : null;
}
