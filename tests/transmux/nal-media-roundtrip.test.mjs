import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";
import { build } from "esbuild";
import {
  referenceNals,
  randomChunks,
  randomGenerator,
} from "./helpers/nal-reference.mjs";

const cases = [
  {
    id: "baseline-short-gop",
    encoder: "libx264",
    profile: "baseline",
    pixels: "yuv420p",
    gop: 12,
    bframes: 0,
  },
  {
    id: "main-bframes-long-gop",
    encoder: "libx264",
    profile: "main",
    pixels: "yuv420p",
    gop: 48,
    bframes: 2,
  },
  {
    id: "rgb-444",
    encoder: "libx264rgb",
    profile: "high444",
    pixels: "rgb24",
    gop: 24,
    bframes: 0,
  },
];
function ffmpeg(args, options = {}) {
  return execFileSync(
    "ffmpeg",
    ["-hide_banner", "-loglevel", "error", "-threads", "1", ...args],
    { maxBuffer: 32 * 1024 * 1024, ...options },
  );
}
function packets(path, type) {
  return JSON.parse(
    execFileSync(
      "ffprobe",
      [
        "-v",
        "error",
        "-select_streams",
        type,
        "-show_packets",
        "-show_entries",
        "packet=pts_time,dts_time,duration_time,flags",
        "-of",
        "json",
        path,
      ],
      { encoding: "utf8" },
    ),
  ).packets;
}
function frames(path, type = "v:0") {
  return ffmpeg(
    [
      "-i",
      path,
      "-map",
      `0:${type}`,
      ...(type === "v:0"
        ? ["-an", "-pix_fmt", "rgb24"]
        : ["-vn", "-c:a", "pcm_s16le"]),
      "-f",
      "framemd5",
      "-hash",
      "sha256",
      "-",
    ],
    { encoding: "utf8" },
  )
    .split("\n")
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(",").at(-1).trim());
}

test("real H.264/AAC TS preserves NAL bytes, packet timing and decoded video", async (t) => {
  // Do not silently skip missing tools/encoders: this is correctness coverage.
  execFileSync("ffprobe", ["-version"], { stdio: "ignore" });
  const directory = await mkdtemp(join(tmpdir(), "wasp-nal-roundtrip-"));
  try {
    const outfile = join(directory, "transmuxer.mjs");
    await build({
      entryPoints: ["tests/transmux/helpers/nal-media-test-entry.ts"],
      bundle: true,
      format: "esm",
      platform: "node",
      outfile,
    });
    const { Finder, Transmuxer, videoPes } = await import(
      pathToFileURL(outfile).href
    );
    for (const item of cases)
      await t.test(item.id, async () => {
        const output = join(directory, item.id);
        await mkdir(output);
        ffmpeg([
          "-f",
          "lavfi",
          "-i",
          "testsrc2=size=320x180:rate=24",
          "-f",
          "lavfi",
          "-i",
          "sine=frequency=880:sample_rate=48000",
          "-t",
          "4",
          "-map",
          "0:v:0",
          "-map",
          "1:a:0",
          "-c:v",
          item.encoder,
          "-profile:v",
          item.profile,
          "-pix_fmt",
          item.pixels,
          "-preset",
          "fast",
          "-threads:v",
          "1",
          "-g",
          String(item.gop),
          "-keyint_min",
          String(item.gop),
          "-bf",
          String(item.bframes),
          "-sc_threshold",
          "0",
          "-c:a",
          "aac",
          "-b:a",
          "64k",
          "-f",
          "hls",
          "-hls_time",
          "2",
          "-hls_list_size",
          "0",
          "-hls_segment_filename",
          join(output, "segment-%d.ts"),
          join(output, "master.m3u8"),
        ]);
        const playlist = await readFile(join(output, "master.m3u8"), "utf8");
        const files = playlist
          .split(/\r?\n/)
          .filter((line) => line.endsWith(".ts"));
        assert.equal(files.length, 2);
        const transmuxer = new Transmuxer({ keepOriginalTimestamps: true });
        let timelineOffset;
        for (const [index, name] of files.entries()) {
          const inputPath = join(output, name);
          const bytes = new Uint8Array(await readFile(inputPath));
          const expected = referenceNals(
            ffmpeg([
              "-i",
              inputPath,
              "-map",
              "0:v:0",
              "-c:v",
              "copy",
              "-f",
              "h264",
              "-",
            ]),
          );
          const pes = videoPes(bytes);
          const chunkings = [
            pes,
            randomChunks(
              Buffer.concat(pes),
              randomGenerator(0x719 + index),
              251,
            ),
            [...Buffer.concat(pes)].map((byte) => Uint8Array.of(byte)),
          ];
          for (const chunks of chunkings) {
            const finder = new Finder();
            const actual = chunks.flatMap((chunk) => finder.findNext(chunk));
            const last = finder.flush();
            if (last) actual.push(last);
            assert.deepEqual(
              actual.map((unit) => Buffer.from(unit)),
              expected.map((unit) => Buffer.from(unit)),
              `${name}: extracted NAL payloads`,
            );
          }
          const converted = transmuxer.transmuxSegment(bytes);
          assert.ok(converted?.data.length);
          const mp4 = join(output, `segment-${index}.mp4`);
          await writeFile(mp4, converted.data);
          assert.deepEqual(
            frames(mp4),
            frames(inputPath),
            `${name}: decoded RGB frame hashes`,
          );
          const originalVideo = packets(inputPath, "v:0");
          const convertedVideo = packets(mp4, "v:0");
          const offset =
            Number(convertedVideo[0].dts_time) -
            Number(originalVideo[0].dts_time);
          timelineOffset ??= offset;
          assert.ok(
            Math.abs(offset - timelineOffset) < 2 / 90000,
            `${name}: segment timeline continuity`,
          );
          for (const type of ["v:0", "a:0"]) {
            const before = packets(inputPath, type),
              after = packets(mp4, type);
            assert.equal(
              after.length,
              before.length,
              `${name}: ${type} packet count`,
            );
            for (let n = 0; n < before.length; n++)
              for (const key of ["pts_time", "dts_time"]) {
                assert.ok(
                  Math.abs(
                    Number(after[n][key]) -
                      Number(before[n][key]) -
                      timelineOffset,
                  ) <
                    2 / 90000,
                  `${name}: ${type} packet ${n} ${key}`,
                );
              }
          }
        }
      });
    // These use the same encoder output, isolating the single-track/container paths.
    for (const [name, type, format] of [
      ["video-only-ts", "v:0", "mpegts"],
      ["audio-only-ts", "a:0", "mpegts"],
      ["raw-aac", "a:0", "adts"],
    ])
      await t.test(name, async () => {
        const input = join(directory, `${name}.bin`);
        ffmpeg([
          "-i",
          join(directory, "baseline-short-gop", "segment-0.ts"),
          "-map",
          `0:${type}`,
          "-c",
          "copy",
          "-f",
          format,
          input,
        ]);
        const transmuxer = new Transmuxer({
          keepOriginalTimestamps: true,
        });
        const inputBytes = new Uint8Array(await readFile(input));
        const converted = transmuxer.transmuxSegment(inputBytes);
        assert.ok(converted?.data.length);
        const output = join(directory, `${name}.mp4`);
        await writeFile(output, converted.data);
        assert.deepEqual(
          frames(output, type),
          frames(input, type),
          `${name}: decoded frame hashes`,
        );
        const before = packets(input, type),
          after = packets(output, type);
        assert.equal(after.length, before.length, `${name}: packet count`);
        const offset = Number(after[0].dts_time) - Number(before[0].dts_time);
        for (let index = 0; index < before.length; index++)
          for (const key of ["pts_time", "dts_time"])
            assert.ok(
              Math.abs(
                Number(after[index][key]) - Number(before[index][key]) - offset,
              ) <
                2 / 90000,
              `${name}: packet ${index} ${key}`,
            );
        if (format === "adts") {
          for (const reset of [false, true]) {
            const next = transmuxer.transmuxSegment(inputBytes, { reset });
            assert.ok(next?.data.length);
            const path = join(directory, `aac-repeat-${reset}.mp4`);
            await writeFile(path, next.data);
            assert.deepEqual(frames(path, type), frames(input, type));
            const streams = JSON.parse(
              execFileSync(
                "ffprobe",
                [
                  "-v",
                  "error",
                  "-show_entries",
                  "stream=id",
                  "-of",
                  "json",
                  path,
                ],
                { encoding: "utf8" },
              ),
            ).streams;
            assert.deepEqual(
              streams.map((stream) => stream.id),
              ["0x1"],
            );
          }
          transmuxer.transmuxSegment(
            new Uint8Array(
              await readFile(
                join(directory, "baseline-short-gop", "segment-0.ts"),
              ),
            ),
            { reset: true },
          );
          const switched = transmuxer.transmuxSegment(inputBytes, {
            reset: true,
          });
          assert.ok(switched?.data.length);
          const path = join(directory, "aac-after-ts-reset.mp4");
          await writeFile(path, switched.data);
          assert.deepEqual(frames(path, type), frames(input, type));
        }
      });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
