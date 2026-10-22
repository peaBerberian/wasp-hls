// Opt-in kernel benchmark. Input copy, output copy and cache updates are timed.
// Native library output layouts are retained; no artificial concatenation copy
// is added to hls.js or Shaka. This does not measure browser playback or workers.
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { loadTransmuxers } from "./helpers/load-rust-transmux.mjs";
import { loadCompetitors } from "./helpers/competitor-transmuxers.mjs";
import {
  hash,
  probeContexts,
  validateAdapters,
} from "./helpers/transmux-benchmark-validation.mjs";

const options = {
  files: [],
  iterations: 100,
  warmup: 20,
  hlsjs: undefined,
  shaka: undefined,
  hlsjsDeps: undefined,
  report: undefined,
};
const flags = {
  "--hlsjs": "hlsjs",
  "--shaka": "shaka",
  "--hlsjs-deps": "hlsjsDeps",
  "--iterations": "iterations",
  "--warmup": "warmup",
  "--report": "report",
};
const args = process.argv.slice(2);
for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (!arg.startsWith("--")) {
    options.files.push(resolve(arg));
    continue;
  }
  assert.ok(arg in flags, `unknown option ${arg}`);
  const value = args[++i];
  assert.ok(value && !value.startsWith("--"), `missing value for ${arg}`);
  const key = flags[arg];
  options[key] = ["iterations", "warmup"].includes(key) ? Number(value) : value;
}
assert.ok(
  options.files.length,
  "Usage: node tests/transmux/benchmark-rust.mjs segment.ts [segment.ts ...] [--hlsjs checkout --hlsjs-deps node_modules --shaka compiled.js --report report.json]",
);
assert.ok(Number.isInteger(options.iterations) && options.iterations > 0);
assert.ok(Number.isInteger(options.warmup) && options.warmup >= 0);
const directory = await mkdtemp(join(tmpdir(), "wasp-transmux-benchmark-"));
try {
  const mod = await loadTransmuxers(directory);
  const competitors = await loadCompetitors(directory, options);
  const inputs = await Promise.all(
    options.files.map(async (file) => new Uint8Array(await readFile(file))),
  );
  const contexts = probeContexts(options.files);
  const factories = {
    rust: () => {
      const muxer = new mod.RustTransmuxer();
      return {
        run(input) {
          const output = muxer.transmuxSegment(input);
          assert.ok(output);
          return { muxed: [output.data] };
        },
        dispose: () => muxer.dispose(),
      };
    },
    ts: () => {
      const muxer = new mod.TsTransmuxer();
      return {
        run(input) {
          const output = muxer.transmuxSegment(input);
          assert.ok(output);
          return { muxed: [output.data] };
        },
        dispose: () => muxer.reset(),
      };
    },
    ...competitors.factories,
  };
  const validation = await validateAdapters(
    directory,
    inputs,
    contexts,
    factories,
  );
  const names = Object.keys(factories),
    times = Object.fromEntries(names.map((name) => [name, []]));
  const returnedBytes = Object.fromEntries(names.map((name) => [name, 0]));
  let checksum = 0;
  // Each pass starts a fresh session, outside timing. Within a pass, feed all
  // consecutive segments in order. Repeatedly treating a backwards timestamp
  // jump as contiguous makes hls.js perform repair work unrelated to parsing.
  for (let pass = 0; pass < options.warmup + options.iterations; pass++) {
    const order = names.map((_, index) => names[(index + pass) % names.length]);
    for (const name of order) {
      const muxer = factories[name]();
      try {
        for (let i = 0; i < inputs.length; i++) {
          const start = performance.now();
          const output = await muxer.run(inputs[i], contexts[i]);
          const duration = performance.now() - start;
          let size = 0;
          for (const parts of Object.values(output))
            for (const part of parts) {
              size += part.length;
              checksum =
                (checksum + part.length + (part[part.length - 1] ?? 0)) >>> 0;
            }
          assert.ok(size > 0, `${name}: no output`);
          if (pass >= options.warmup) {
            times[name].push(duration);
            returnedBytes[name] += size;
          }
        }
      } finally {
        await muxer.dispose();
      }
    }
  }
  const metrics = {};
  for (const [name, samples] of Object.entries(times)) {
    samples.sort((a, b) => a - b);
    metrics[name] = {
      medianMs: samples[Math.floor(samples.length / 2)],
      p95Ms: samples[Math.floor(samples.length * 0.95)],
      samples: samples.length,
      meanReturnedBytes: returnedBytes[name] / samples.length,
    };
  }
  const report = {
    schemaVersion: 2,
    date: new Date().toISOString(),
    node: process.version,
    cpu: cpus()[0]?.model,
    measurement:
      "clear H.264/AAC TS to native JS output buffers; no decoding, network, MSE or worker transport",
    lifecycle:
      "fresh session per playlist pass; construction/destruction excluded; per-segment init generation included",
    warmupPasses: options.warmup,
    measuredPasses: options.iterations,
    inputs: options.files.map((file, index) => ({
      file,
      bytes: inputs[index].length,
      sha256: hash(inputs[index]),
      ...contexts[index],
    })),
    wasmSha256: hash(await readFile("build/wasp_hls_bg.wasm")),
    competitors: competitors.metadata,
    validation,
    metrics,
    wasmMemoryBytes: mod.wasm.memory.buffer.byteLength,
    checksum,
  };
  const serialized = JSON.stringify(report, null, 2) + "\n";
  if (options.report) await writeFile(options.report, serialized);
  process.stdout.write(serialized);
} finally {
  await rm(directory, { recursive: true, force: true });
}
