import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { after, before, test } from "node:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadTransmuxers } from "./helpers/load-rust-transmux.mjs";

let directory, mod, wasm;
const fixtures = new Map();

function ffmpeg(args) {
  return execFileSync("ffmpeg", ["-v", "error", "-y", ...args], {
    maxBuffer: 32 * 1024 * 1024,
  });
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
        "packet=pts,dts,duration,size,data_hash",
        "-show_data_hash",
        "sha256",
        "-of",
        "json",
        path,
      ],
      { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
    ),
  ).packets;
}

function frames(path) {
  return ffmpeg(["-i", path, "-map", "0:v:0", "-f", "framemd5", "-"])
    .toString()
    .split("\n")
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(",").at(-1).trim());
}

function audioPayloadHashes(path) {
  // Repackage both containers as ADTS with FFmpeg, without re-encoding. This
  // independently normalizes their transport headers before hashing AAC data.
  const bytes = ffmpeg([
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
  return createHash("sha256").update(bytes).digest("hex");
}

before(async () => {
  directory = await mkdtemp(join(tmpdir(), "wasp-rust-transmux-"));
  mod = await loadTransmuxers(directory);
  wasm = mod.wasm;
  for (const [name, profile, pixels, bframes, encoder] of [
    ["baseline", "baseline", "yuv420p", 0, "libx264"],
    ["high", "high", "yuv420p", 2, "libx264"],
    ["422", "high422", "yuv422p", 2, "libx264"],
    ["444", "high444", "rgb24", 2, "libx264rgb"],
  ]) {
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
      "6",
      "-c:v",
      encoder,
      "-profile:v",
      profile,
      "-pix_fmt",
      pixels,
      "-preset",
      "fast",
      "-threads:v",
      "1",
      "-g",
      "48",
      "-keyint_min",
      "48",
      "-bf",
      String(bframes),
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
      join(directory, `${name}-%d.ts`),
      join(directory, `${name}.m3u8`),
    ]);
    const segments = await Promise.all(
      [0, 1, 2].map(
        async (index) =>
          new Uint8Array(
            await readFile(join(directory, `${name}-${index}.ts`)),
          ),
      ),
    );
    fixtures.set(name, segments);
  }
});

after(async () => {
  if (directory) await rm(directory, { recursive: true, force: true });
});

for (const name of ["baseline", "high", "422", "444"]) {
  test(`Rust WASM ${name}: decoded pictures, AAC payloads and packet timing survive`, async () => {
    const muxer = new mod.RustTransmuxer({ keepOriginalTimestamps: true });
    try {
      for (const [index, bytes] of fixtures.get(name).entries()) {
        const output = muxer.transmuxSegment(bytes);
        assert.ok(output?.data.length);
        const source = join(directory, `${name}-${index}.ts`),
          dest = join(directory, `${name}-${index}.mp4`);
        await writeFile(dest, output.data);
        assert.deepEqual(frames(dest), frames(source));
        for (const type of ["v:0", "a:0"]) {
          const original = packets(source, type),
            converted = packets(dest, type);
          assert.equal(
            converted.length,
            original.length,
            `${type} packet count`,
          );
          for (let i = 0; i < original.length; i++) {
            const scale = type === "v:0" ? 90000 : 48000;
            for (const field of ["pts", "dts"])
              assert.ok(
                Math.abs(
                  converted[i][field] / scale - original[i][field] / 90000,
                ) <
                  2 / 90000,
                `${type} ${field} ${i}`,
              );
          }
        }
        assert.deepEqual(audioPayloadHashes(dest), audioPayloadHashes(source));
        assert.equal(mod.getIsoBmffCodecs(output.data).length, 2);
      }
    } finally {
      muxer.dispose();
    }
  });
}

test("Rust and TS agree on continuity hints, reset, skipped segments and back-seeks", () => {
  const [early, , late] = fixtures.get("high");
  const rust = new mod.RustTransmuxer(),
    ts = new mod.TsTransmuxer();
  try {
    for (const [data, seed, reset] of [
      [late, 360000, false],
      [early, 999999, false],
      [late, 765432, false],
      [early, 1800000, true],
      [late, 90000, false],
    ]) {
      const options = {
        reset,
        baseMediaDecodeTimeSeed: { value: seed, timescale: 90000 },
      };
      const a = rust.transmuxSegment(data, options),
        b = ts.transmuxSegment(data, options);
      assert.ok(a && b);
      assert.equal(a.timingInfo.timescale, b.timingInfo.timescale);
      assert.ok(Math.abs(a.timingInfo.start - b.timingInfo.start) <= 1);
      assert.ok(Math.abs(a.timingInfo.end - b.timingInfo.end) <= 1);
      const ai = mod.getIsobmfTimeInfo(a.data, mod.getInitTrackInfo(a.data));
      const bi = mod.getIsobmfTimeInfo(b.data, mod.getInitTrackInfo(b.data));
      assert.ok(
        Math.abs(ai.time / ai.timescale - bi.time / bi.timescale) < 2 / 90000,
      );
    }
  } finally {
    rust.dispose();
  }
});

test("video without AUD uses slice boundaries", async () => {
  const input = fixtures.get("high")[0].slice();
  let replaced = 0;
  for (let i = 2; i < input.length - 2; i++) {
    if (
      input[i - 2] === 0 &&
      input[i - 1] === 0 &&
      input[i] === 1 &&
      input[i + 1] === 9
    ) {
      input[i + 1] = 12;
      input[i + 2] = 0x80;
      replaced++;
    }
  }
  assert.ok(replaced > 40);
  const muxer = new mod.RustTransmuxer({ keepOriginalTimestamps: true });
  try {
    const output = muxer.transmuxSegment(input);
    assert.ok(output);
    const dest = join(directory, "no-aud.mp4");
    await writeFile(dest, output.data);
    assert.deepEqual(frames(dest), frames(join(directory, "high-0.ts")));
  } finally {
    muxer.dispose();
  }
});

test("single-track TS, packed ADTS, CRC and ID3 timestamp", async () => {
  for (const [name, map, format] of [
    ["audio", "a:0", "mpegts"],
    ["video", "v:0", "mpegts"],
    ["aac", "a:0", "adts"],
  ]) {
    const path = join(directory, `${name}.bin`);
    ffmpeg([
      "-i",
      join(directory, "high-0.ts"),
      "-map",
      `0:${map}`,
      "-c",
      "copy",
      "-f",
      format,
      path,
    ]);
    const bytes = new Uint8Array(await readFile(path));
    const muxer = new mod.RustTransmuxer();
    try {
      const output = muxer.transmuxSegment(bytes);
      assert.ok(output);
      assert.equal(mod.getIsoBmffCodecs(output.data).length, 1);
      const dest = join(directory, `${name}-only.mp4`);
      await writeFile(dest, output.data);
      if (name === "video") assert.deepEqual(frames(dest), frames(path));
      else assert.deepEqual(audioPayloadHashes(dest), audioPayloadHashes(path));
    } finally {
      muxer.dispose();
    }
  }
  // A complete two-byte AAC payload with an ADTS CRC. Its bytes need not decode
  // for this framing test; the real AAC round-trip above covers decoding input.
  const crc = Uint8Array.of(255, 240, 76, 128, 1, 127, 252, 0, 0, 33, 16);
  const timestamp = 8589934590;
  const owner = Buffer.from("com.apple.streaming.transportStreamTimestamp\0");
  const value = Buffer.alloc(8);
  value.writeBigUInt64BE(BigInt(timestamp));
  const frame = Buffer.concat([
    Buffer.from("PRIV"),
    Buffer.alloc(6),
    owner,
    value,
  ]);
  frame.writeUInt32BE(owner.length + 8, 4);
  const tag = Buffer.alloc(10);
  tag.write("ID3");
  tag[3] = 3;
  tag[9] = frame.length;
  const muxer = new mod.RustTransmuxer({ keepOriginalTimestamps: true });
  try {
    const output = muxer.transmuxSegment(Buffer.concat([tag, frame, crc]));
    assert.ok(output);
    assert.equal(
      output.timingInfo.start,
      Math.floor((timestamp * 48000) / 90000),
    );
  } finally {
    muxer.dispose();
  }
});

test("failure is recoverable, output is owned and explicit cleanup reaches a WASM memory plateau", () => {
  const bytes = fixtures.get("high")[0];
  const muxer = new mod.RustTransmuxer();
  try {
    const first = muxer.transmuxSegment(bytes).data;
    const saved = first.slice();
    assert.throws(
      () => muxer.transmuxSegment(bytes.subarray(0, bytes.length - 1)),
      /truncated/,
    );
    assert.throws(
      () =>
        muxer.transmuxSegment(bytes, {
          baseMediaDecodeTimeSeed: { value: 0, timescale: 0 },
        }),
      /seed/,
    );
    wasm.memory.grow(1);
    for (let i = 0; i < 10; i++) assert.ok(muxer.transmuxSegment(bytes));
    const warmed = wasm.memory.buffer.byteLength;
    for (let i = 0; i < 100; i++) assert.ok(muxer.transmuxSegment(bytes));
    assert.equal(
      wasm.memory.buffer.byteLength,
      warmed,
      "repeated segments must reuse freed linear memory",
    );
    assert.deepEqual(
      first,
      saved,
      "later calls must not overwrite returned bytes",
    );
  } finally {
    muxer.dispose();
    muxer.dispose();
  }
  assert.throws(() => muxer.transmuxSegment(bytes), /disposed/);
  for (let i = 0; i < 10; i++) {
    const m = new mod.RustTransmuxer();
    m.transmuxSegment(bytes);
    m.dispose();
  }
  const warmed = wasm.memory.buffer.byteLength;
  for (let i = 0; i < 100; i++) {
    const m = new mod.RustTransmuxer();
    m.transmuxSegment(bytes);
    m.dispose();
  }
  assert.equal(
    wasm.memory.buffer.byteLength,
    warmed,
    "instance churn must not leak Rust handles or GOP caches",
  );
});

test("33-bit timestamp wrap works forwards and across a back-seek", () => {
  function shifted(input, delta) {
    const result = input.slice(),
      period = 2 ** 33;
    for (let at = 0; at < result.length; at += 188) {
      if ((result[at + 1] & 0x40) === 0 || (result[at + 3] & 0x10) === 0)
        continue;
      let payload = at + 4;
      if (result[at + 3] & 0x20) payload += 1 + result[at + 4];
      if (
        payload + 19 > at + 188 ||
        result[payload] !== 0 ||
        result[payload + 1] !== 0 ||
        result[payload + 2] !== 1
      )
        continue;
      const flags = result[payload + 7] >> 6;
      for (const offset of flags === 3 ? [9, 14] : flags === 2 ? [9] : []) {
        const i = payload + offset;
        const value =
          (result[i] & 14) * 2 ** 29 +
          result[i + 1] * 2 ** 22 +
          (result[i + 2] & 254) * 2 ** 14 +
          result[i + 3] * 128 +
          (result[i + 4] >> 1);
        const time = (((value + delta) % period) + period) % period;
        result[i] = (result[i] & 0xf0) | (Math.floor(time / 2 ** 29) & 14) | 1;
        result[i + 1] = Math.floor(time / 2 ** 22) & 255;
        result[i + 2] = (Math.floor(time / 2 ** 14) & 254) | 1;
        result[i + 3] = Math.floor(time / 128) & 255;
        result[i + 4] = ((time % 128) << 1) | 1;
      }
    }
    return result;
  }
  const [early, , late] = fixtures
    .get("high")
    .map((input) => shifted(input, 2 ** 33 - 200000));
  const rust = new mod.RustTransmuxer(),
    ts = new mod.TsTransmuxer();
  try {
    for (const bytes of [early, late, early, late]) {
      const actual = rust.transmuxSegment(bytes),
        expected = ts.transmuxSegment(bytes);
      assert.deepEqual(actual.timingInfo, expected.timingInfo);
    }
  } finally {
    rust.dispose();
  }
});

test("PSI sections and PES headers may be split at any transport boundary", () => {
  function packet(pid, start, counter, payload) {
    const out = Buffer.alloc(188, 255);
    out[0] = 0x47;
    out[1] = (pid >> 8) | (start ? 0x40 : 0);
    out[2] = pid & 255;
    out[3] = 0x30 | counter;
    out[4] = 183 - payload.length;
    if (out[4] > 0) out[5] = 0;
    out.set(payload, 188 - payload.length);
    return out;
  }
  const pat = Buffer.from([
    0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe0, 100, 0, 0, 0, 0,
  ]);
  const pmt = Buffer.from([
    2, 0xb0, 18, 0, 1, 0xc1, 0, 0, 0xe0, 101, 0xf0, 0, 0x0f, 0xe0, 101, 0xf0, 0,
    0, 0, 0, 0,
  ]);
  const pes = Buffer.from([
    0, 0, 1, 0xc0, 0, 17, 0x80, 0x80, 5, 0x21, 0, 1, 0, 1, 255, 241, 76, 128, 1,
    63, 252, 33, 16,
  ]);
  for (let split = 1; split < 19; split++) {
    const input = Buffer.concat([
      packet(0, true, 0, Buffer.concat([Buffer.from([0]), pat.subarray(0, 2)])),
      packet(0, false, 1, pat.subarray(2)),
      packet(100, true, 0, Buffer.concat([Buffer.from([0]), pmt])),
      packet(101, true, 0, pes.subarray(0, split)),
      packet(101, false, 1, pes.subarray(split)),
    ]);
    const muxer = new mod.RustTransmuxer();
    try {
      const output = muxer.transmuxSegment(input);
      assert.deepEqual(output.timingInfo, {
        start: 0,
        end: 1024,
        timescale: 48000,
      });
    } finally {
      muxer.dispose();
    }
  }
});

test("duplicate transport packets are ignored and missing payload fails explicitly", () => {
  const input = fixtures.get("high")[0];
  let at = 0;
  while (
    at < input.length &&
    (((input[at + 1] & 31) << 8) | input[at + 2]) !== 256
  )
    at += 188;
  assert.ok(at < input.length);
  const duplicate = Buffer.concat([
    input.subarray(0, at + 188),
    input.subarray(at, at + 188),
    input.subarray(at + 188),
  ]);
  const missing = Buffer.concat([
    input.subarray(0, at + 188),
    input.subarray(at + 376),
  ]);
  const muxer = new mod.RustTransmuxer();
  try {
    const expected = muxer.transmuxSegment(input);
    assert.deepEqual(
      muxer.transmuxSegment(duplicate).timingInfo,
      expected.timingInfo,
    );
    assert.throws(
      () => muxer.transmuxSegment(missing),
      /missing transport packet/,
    );
    assert.deepEqual(
      muxer.transmuxSegment(input).timingInfo,
      expected.timingInfo,
    );
  } finally {
    muxer.dispose();
  }
});

test("mutated input returns errors or media, without trapping the WASM instance", () => {
  const source = fixtures.get("high")[0];
  let random = 0x713af;
  function next() {
    random ^= random << 13;
    random ^= random >>> 17;
    random ^= random << 5;
    return random >>> 0;
  }
  const muxer = new mod.RustTransmuxer();
  try {
    for (let round = 0; round < 1000; round++) {
      const bytes = source.slice();
      for (let i = 0; i < 8; i++) bytes[next() % bytes.length] = next() & 255;
      try {
        muxer.transmuxSegment(bytes, { reset: true });
      } catch (error) {
        assert.match(error.message, /^Rust transmux:/);
      }
    }
    assert.ok(muxer.transmuxSegment(source, { reset: true }));
  } finally {
    muxer.dispose();
  }
});

test("GOP history bridges multiple segments which start with predictive pictures", async () => {
  ffmpeg([
    "-f",
    "lavfi",
    "-i",
    "testsrc2=size=160x90:rate=24",
    "-t",
    "4",
    "-c:v",
    "libx264",
    "-preset",
    "fast",
    "-threads:v",
    "1",
    "-g",
    "96",
    "-keyint_min",
    "96",
    "-sc_threshold",
    "0",
    "-f",
    "hls",
    "-hls_time",
    "1",
    "-hls_flags",
    "split_by_time",
    "-hls_list_size",
    "0",
    "-hls_segment_filename",
    join(directory, "gop-%d.ts"),
    join(directory, "gop.m3u8"),
  ]);
  const muxer = new mod.RustTransmuxer();
  try {
    for (let index = 0; index < 4; index++) {
      const input = new Uint8Array(
        await readFile(join(directory, `gop-${index}.ts`)),
      );
      const result = muxer.transmuxSegment(input);
      assert.ok(result);
      // Each fragment must include the first IDR and the intervening history.
      assert.equal(result.timingInfo.start, 0);
      const dest = join(directory, `gop-${index}.mp4`);
      await writeFile(dest, result.data);
      assert.ok(frames(dest).length >= 24 * (index + 1) - 4);
    }
  } finally {
    muxer.dispose();
  }
});

test("ID3-less 44.1 kHz calls keep a fractional clock instead of accumulating rounding errors", () => {
  // One synthetic frame, two payload bytes, sample-rate index 4 (44.1 kHz).
  const bytes = Uint8Array.of(255, 241, 80, 128, 1, 63, 252, 33, 16);
  const muxer = new mod.RustTransmuxer();
  try {
    for (let index = 0; index < 2000; index++) {
      const result = muxer.transmuxSegment(bytes);
      assert.equal(result.timingInfo.timescale, 44100);
      assert.ok(Math.abs(result.timingInfo.start - index * 1024) <= 1);
    }
  } finally {
    muxer.dispose();
  }
});

test("input and packed-frame limits fail before large WASM scratch allocations", () => {
  const muxer = new mod.RustTransmuxer();
  try {
    const before = wasm.memory.buffer.byteLength;
    assert.throws(
      () => muxer.transmuxSegment(new Uint8Array(64 * 1024 * 1024 + 1)),
      /input limit/,
    );
    assert.equal(
      wasm.memory.buffer.byteLength,
      before,
      "input cap must be checked before the boundary copy",
    );
    const frame = Uint8Array.of(255, 241, 76, 128, 1, 63, 252, 33, 16);
    const input = new Uint8Array(frame.length * 131073);
    for (let at = 0; at < input.length; at += frame.length)
      input.set(frame, at);
    assert.throws(
      () => muxer.transmuxSegment(input),
      /exceeds transmux limits/,
    );
    assert.ok(muxer.transmuxSegment(fixtures.get("high")[0]));
  } finally {
    muxer.dispose();
  }
});
