import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";
import { build } from "esbuild";

function ffmpeg(args) {
  return execFileSync("ffmpeg", [
    "-hide_banner",
    "-loglevel",
    "error",
    ...args,
  ]);
}

function audioFrames(path) {
  return ffmpeg([
    "-i",
    path,
    "-map",
    "0:a:0",
    "-c:a",
    "pcm_s16le",
    "-f",
    "framemd5",
    "-hash",
    "sha256",
    "-",
  ])
    .toString()
    .split("\n")
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(",").at(-1).trim());
}

test("raw AAC keeps track ID 1 after another segment or a reset", async () => {
  const directory = await mkdtemp(join(tmpdir(), "wasp-aac-track-id-"));
  try {
    const outfile = join(directory, "transmuxer.mjs");
    await build({
      entryPoints: ["tests/transmux/helpers/transmux-test-entry.ts"],
      bundle: true,
      format: "esm",
      platform: "node",
      outfile,
    });
    const { default: Transmuxer } = await import(pathToFileURL(outfile).href);
    const input = join(directory, "input.aac");
    ffmpeg([
      "-f",
      "lavfi",
      "-i",
      "sine=frequency=880:sample_rate=48000",
      "-t",
      "2",
      "-c:a",
      "aac",
      "-b:a",
      "64k",
      "-f",
      "adts",
      input,
    ]);
    const ts = join(directory, "input.ts");
    ffmpeg(["-i", input, "-c:a", "copy", "-f", "mpegts", ts]);
    const inputBytes = new Uint8Array(await readFile(input));
    const tsBytes = new Uint8Array(await readFile(ts));
    const expectedFrames = audioFrames(input);
    const transmuxer = new Transmuxer({ keepOriginalTimestamps: true });
    for (const [name, options] of [
      ["first", {}],
      ["next", {}],
      ["reset", { reset: true }],
      ["after-ts", { reset: true }],
    ]) {
      if (name === "after-ts")
        transmuxer.transmuxSegment(tsBytes, { reset: true });
      const converted = transmuxer.transmuxSegment(inputBytes, options);
      assert.ok(converted?.data.length);
      const output = join(directory, `${name}.mp4`);
      await writeFile(output, converted.data);
      const streams = JSON.parse(
        execFileSync(
          "ffprobe",
          ["-v", "error", "-show_entries", "stream=id", "-of", "json", output],
          { encoding: "utf8" },
        ),
      ).streams;
      assert.deepEqual(
        streams.map((stream) => stream.id),
        ["0x1"],
        name,
      );
      assert.deepEqual(audioFrames(output), expectedFrames, name);
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
