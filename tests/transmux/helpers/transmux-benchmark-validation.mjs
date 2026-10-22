import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";

export const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

function ffmpeg(args, input) {
  return execFileSync("ffmpeg", ["-v", "error", "-y", ...args], {
    maxBuffer: 64 * 1024 * 1024,
    input,
  });
}

/** Probe before timing. Do not guess a segment's duration from its file size. */
export function probeContexts(files) {
  let origin;
  return files.map((file) => {
    const probe = JSON.parse(
      execFileSync(
        "ffprobe",
        [
          "-v",
          "error",
          "-show_packets",
          "-show_streams",
          "-show_entries",
          "stream=index,codec_name,codec_type,profile:packet=stream_index,dts_time,duration_time",
          "-of",
          "json",
          file,
        ],
        { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
      ),
    );
    const video = probe.streams.find((stream) => stream.codec_type === "video");
    const audio = probe.streams.find((stream) => stream.codec_type === "audio");
    assert.equal(video?.codec_name, "h264", "benchmark requires H.264 video");
    assert.equal(audio?.codec_name, "aac", "benchmark requires AAC audio");
    assert.equal(
      audio?.profile,
      "LC",
      "benchmark adapter declares AAC-LC to hls.js",
    );
    const packets = probe.packets.filter(
      (packet) => packet.stream_index === video.index,
    );
    assert.ok(
      packets.length >= 2,
      "benchmark needs at least two video pictures per segment",
    );
    const first = Number(packets[0].dts_time),
      last = packets.at(-1);
    const duration = Number(last.dts_time) + Number(last.duration_time) - first;
    assert.ok(Number.isFinite(first) && duration > 0);
    origin ??= first;
    return { start: first - origin, duration };
  });
}

function videoHashes(path) {
  return ffmpeg([
    "-i",
    path,
    "-map",
    "0:v:0",
    "-fps_mode",
    "passthrough",
    "-f",
    "framemd5",
    "-",
  ])
    .toString()
    .split("\n")
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(",").at(-1).trim());
}

function audioFrames(path) {
  // Normalize framing with an independent tool, without re-encoding AAC.
  const data = ffmpeg([
    "-i",
    path,
    "-map",
    "0:a:0",
    "-c:a",
    "copy",
    "-f",
    "adts",
    "-",
  ]);
  const frames = [];
  for (let at = 0; at < data.length;) {
    assert.equal(data[at], 255);
    const length =
      ((data[at + 3] & 3) << 11) | (data[at + 4] << 3) | (data[at + 5] >> 5);
    const header = data[at + 1] & 1 ? 7 : 9;
    assert.ok(length > header && at + length <= data.length);
    frames.push({
      bytes: data.subarray(at, at + length),
      hash: hash(data.subarray(at + header, at + length)),
    });
    at += length;
  }
  return frames;
}

/** Validate every adapter outside timing, including both Shaka demux passes. */
export async function validateAdapters(directory, inputs, contexts, factories) {
  const source = join(directory, "input.ts");
  await writeFile(source, Buffer.concat(inputs));
  const expectedVideo = videoHashes(source),
    expectedAudio = audioFrames(source);
  const results = {};
  for (const [name, create] of Object.entries(factories)) {
    const muxer = create(),
      collected = {};
    try {
      for (let i = 0; i < inputs.length; i++) {
        const output = await muxer.run(inputs[i], contexts[i]);
        for (const [type, parts] of Object.entries(output))
          (collected[type] ??= []).push(...parts);
      }
    } finally {
      await muxer.dispose();
    }
    const paths = {};
    for (const [type, parts] of Object.entries(collected)) {
      assert.ok(
        parts.some((part) => part.length),
        `${name}: no ${type} bytes`,
      );
      const path = join(directory, `${name}-${type}.mp4`);
      await writeFile(path, Buffer.concat(parts));
      paths[type] = path;
    }
    const actualVideo = videoHashes(paths.muxed ?? paths.video);
    assert.deepEqual(
      actualVideo,
      expectedVideo,
      `${name}: decoded video differs`,
    );
    const actualAudio = audioFrames(paths.muxed ?? paths.audio);
    const extra = actualAudio.length - expectedAudio.length;
    assert.ok(extra >= 0, `${name}: AAC frames were lost`);
    assert.deepEqual(
      actualAudio.slice(extra).map((frame) => frame.hash),
      expectedAudio.map((frame) => frame.hash),
      `${name}: encoded source AAC differs`,
    );
    if (extra) {
      // hls.js may prepend AAC silence to align its initial timeline. Permit
      // only that exact difference, and independently decode it to verify it.
      const pcm = ffmpeg(
        [
          "-f",
          "aac",
          "-i",
          "pipe:0",
          "-map",
          "0:a:0",
          "-c:a",
          "pcm_s16le",
          "-f",
          "s16le",
          "-",
        ],
        Buffer.concat(actualAudio.slice(0, extra).map((frame) => frame.bytes)),
      );
      assert.ok(
        pcm.length > 0 && pcm.every((byte) => byte === 0),
        `${name}: added AAC is not silence`,
      );
    }
    results[name] = {
      decodedVideoFrames: actualVideo.length,
      videoHashesSha256: hash(actualVideo.join("\n")),
      sourceAacFrames: expectedAudio.length,
      outputAacFrames: actualAudio.length,
      leadingSilentAacFrames: extra,
      sourceAacPayloadHashesSha256: hash(
        expectedAudio.map((frame) => frame.hash).join("\n"),
      ),
    };
  }
  return results;
}
