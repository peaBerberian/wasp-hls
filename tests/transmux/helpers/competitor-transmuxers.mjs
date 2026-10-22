import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";
import { build } from "esbuild";

const require = createRequire(import.meta.url);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const noop = () => {};

/**
 * Use actual library pipelines. Hls.js returns separate init/moof/mdat buffers;
 * Shaka runs its video and audio pipelines separately, as its player does.
 * Keep those native output layouts: joining them would add copies their own
 * buffering paths do not require. Wasp's WASM-to-JS copy stays inside timing.
 */
export async function loadCompetitors(directory, { hlsjs, shaka, hlsjsDeps }) {
  globalThis.self ??= globalThis;
  const factories = {};
  const metadata = {};
  if (hlsjs) {
    const checkout = resolve(hlsjs);
    const entry = join(directory, "hlsjs-entry.mjs");
    await writeFile(
      entry,
      [
        `export {default as Transmuxer,TransmuxConfig,TransmuxState} from ${JSON.stringify(join(checkout, "src/demux/transmuxer.ts"))};`,
        `export {ChunkMetadata} from ${JSON.stringify(join(checkout, "src/types/transmuxer.ts"))};`,
      ].join("\n"),
    );
    const outfile = join(directory, "hlsjs.mjs");
    const define = Object.fromEntries(
      [
        "ALT_AUDIO",
        "CMCD",
        "CONTENT_STEERING",
        "EME_DRM",
        "IFRAMES",
        "INTERSTITIALS",
        "M2TS_ADVANCED_CODECS",
        "MEDIA_CAPABILITIES",
        "SUBTITLES",
        "VARIABLE_SUBSTITUTION",
      ].map((name) => [`__USE_${name}__`, "true"]),
    );
    await build({
      entryPoints: [entry],
      bundle: true,
      format: "esm",
      platform: "node",
      minify: true,
      outfile,
      nodePaths: hlsjsDeps ? [resolve(hlsjsDeps)] : [],
      define: {
        ...define,
        __VERSION__: JSON.stringify("local-source"),
        __IN_WORKER__: "false",
        __HLS_WORKER_BUNDLE__: "false",
      },
      logLevel: "silent",
    });
    const lib = await import(pathToFileURL(outfile).href);
    metadata.hlsjs = {
      build: "local source, esbuild minified",
      checkout,
      revision: execFileSync("git", ["-C", checkout, "rev-parse", "HEAD"], {
        encoding: "utf8",
      }).trim(),
      bundleSha256: sha256(await readFile(outfile)),
    };
    factories.hlsjs = () => {
      const observer = new EventEmitter();
      observer.on("hlsError", (_event, data) => {
        throw new Error(`hls.js: ${data.reason ?? data.details}`);
      });
      const logger = Object.fromEntries(
        ["trace", "debug", "log", "warn", "info", "error"].map((name) => [
          name,
          noop,
        ]),
      );
      const muxer = new lib.Transmuxer(
        observer,
        { mpeg: false, mp3: false, ac3: false },
        {
          // Defaults used by the clear H.264/AAC path; avoid importing unrelated
          // player controllers just to read their shared configuration object.
          progressive: false,
          forceKeyFrameOnDiscontinuity: true,
          handleMpegTsVideoIntegrityErrors: "process",
          stretchShortVideoTrack: false,
          maxAudioFramesDrift: 1,
          maxBufferHole: 0.1,
          enableSoftwareAES: true,
        },
        "",
        "main",
        logger,
      );
      muxer.configure(
        new lib.TransmuxConfig("mp4a.40.2", "avc1.4d4028", undefined, 3600),
      );
      let index = 0;
      return {
        run(input, context) {
          // Input files are standalone Uint8Arrays, not Node Buffer slab views.
          assert.equal(input.byteOffset, 0);
          assert.equal(input.byteLength, input.buffer.byteLength);
          const meta = new lib.ChunkMetadata(
            0,
            index,
            0,
            input.length,
            -1,
            false,
            context.duration,
          );
          const state = new lib.TransmuxState(
            index === 0,
            index > 0,
            true,
            false,
            context.start,
            false,
          );
          const pushed = muxer.push(input.buffer, null, meta, state);
          assert.ok(
            !(pushed instanceof Promise),
            "clear TS push must be synchronous",
          );
          const flushed = muxer.flush(meta);
          assert.ok(
            !(flushed instanceof Promise),
            "clear TS flush must be synchronous",
          );
          index++;
          const output = { audio: [], video: [] };
          for (const { remuxResult: result } of [pushed, ...flushed]) {
            for (const type of ["audio", "video"]) {
              const init = result.initSegment?.tracks?.[type]?.initSegment;
              if (init) output[type].push(init);
              const track = result[type];
              if (track?.data1?.length) output[type].push(track.data1);
              if (track?.data2?.length) output[type].push(track.data2);
            }
          }
          return output;
        },
        dispose: () => muxer.destroy(),
      };
    };
  }
  if (shaka) {
    const artifact = resolve(shaka);
    const lib = require(artifact);
    metadata.shaka = {
      build: "provided production bundle",
      artifact,
      version: lib.Player?.version,
      artifactSha256: sha256(await readFile(artifact)),
    };
    factories.shaka = () => {
      const muxers = {
        audio: new lib.transmuxer.TsTransmuxer("video/mp2t"),
        video: new lib.transmuxer.TsTransmuxer("video/mp2t"),
      };
      const streams = {
        audio: {
          id: 2,
          codecs: "mp4a.40.2",
          language: "und",
          encrypted: false,
          drmInfos: [],
        },
        video: {
          id: 1,
          codecs: "avc1.4d4028",
          language: "und",
          encrypted: false,
          drmInfos: [],
        },
      };
      return {
        async run(input, context) {
          // Use the actual exported reference class: Closure may rename its
          // internal properties, so a plain-object mock would change behavior.
          const ref = new lib.media.SegmentReference(
            context.start,
            context.start + context.duration,
            () => ["benchmark.ts"],
            0,
            null,
            null,
            0,
            0,
            Infinity,
          );
          const output = { audio: [], video: [] };
          for (const type of ["audio", "video"]) {
            const result = await muxers[type].transmux(
              input,
              streams[type],
              ref,
              3600,
              type,
            );
            if (result.init?.length) output[type].push(result.init);
            if (result.data?.length) output[type].push(result.data);
          }
          return output;
        },
        dispose: () => {
          muxers.audio.destroy();
          muxers.video.destroy();
        },
      };
    };
  }
  return { factories, metadata };
}
