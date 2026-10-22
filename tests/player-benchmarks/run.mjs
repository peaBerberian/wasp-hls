#!/usr/bin/env node
import { createServer } from "node:http";
import { mkdir, writeFile, readFile } from "node:fs/promises";
import { cpus } from "node:os";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { randomUUID, createHash } from "node:crypto";
import { build } from "esbuild";
import { runBrowser } from "./browser.mjs";
import { prepareFixtures } from "./fixtures.mjs";
import { help, parseOptions, samplePlan } from "./config.mjs";
import getChromeCmd from "../../scripts/get_chrome_cmd.mjs";

const directory = dirname(fileURLToPath(import.meta.url));
const root = resolve(directory, "../..");
if (process.argv.includes("--help")) {
  console.log(help);
  process.exit(0);
}
const options = parseOptions(process.argv.slice(2), directory);
const chrome =
  process.env.WASP_HLS_CHROME_BINARY ??
  process.env.CHROME_PATH ??
  (await getChromeCmd());
if (!chrome) throw new Error("Chrome is unavailable");
const bundle = await build({
  entryPoints: [join(directory, "src/runner.ts")],
  bundle: true,
  write: false,
  minify: true,
  format: "esm",
  target: "es2022",
});
const fixtures = await prepareFixtures(
  directory,
  options.formats,
  options.scenarios.some((name) => name.includes("large-playlist")),
);
const samples = [];
const hlsWorker = await readFile(
  join(directory, "node_modules/hls.js/dist/hls.worker.js"),
);
let pending;
const server = createServer(async (request, response) => {
  try {
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    const path = new URL(request.url, "http://localhost").pathname;
    if (path === "/hls.worker.js") {
      if (pending) pending.hlsWorkerRequests++;
      response.setHeader("Content-Type", "text/javascript");
      response.setHeader("Cache-Control", "no-store");
      response.end(hlsWorker);
      return;
    }
    if (await fixtures.serve(path, response)) return;
    if (path === "/result" && request.method === "POST") {
      let body = "";
      for await (const chunk of request) {
        body += chunk;
        if (body.length > 10e6) throw new Error("Result exceeds limit");
      }
      const message = JSON.parse(body);
      if (!pending || message.token !== pending.token) {
        response.writeHead(409).end();
        return;
      }
      response.end("ok");
      pending.resolve(message);
      return;
    }
    if (path === "/runner.js") {
      response.setHeader("Content-Type", "text/javascript");
      response.end(bundle.outputFiles[0].contents);
      return;
    }
    if (path === "/") {
      response.setHeader("Content-Type", "text/html");
      response.end(
        '<!doctype html><meta charset="utf-8"><title>Player benchmark</title><video muted playsinline width="640" height="360"></video><script type="module" src="/runner.js"></script>',
      );
      return;
    }
    response.writeHead(404).end();
  } catch (error) {
    response.writeHead(500).end(String(error));
  }
});
const metadata = {
  schemaVersion: 3,
  fixtures: fixtures.metadata,
  date: new Date().toISOString(),
  chrome: execFileSync(chrome, ["--version"], { encoding: "utf8" }).trim(),
  node: process.version,
  platform: process.platform,
  cpu: cpus()[0]?.model,
  buildHashes: {},
  releaseBuildRequired: true,
  competitorArtifacts: {
    hlsWorkerSha256: createHash("sha256").update(hlsWorker).digest("hex"),
  },
  options,
  versions: {},
};
for (const artifact of [
  "build/es6/ts-main/index.js",
  "build/embedded/worker.js",
  "build/embedded/wasm.js",
]) {
  metadata.buildHashes[artifact] = createHash("sha256")
    .update(await readFile(join(root, artifact)))
    .digest("hex");
}
for (const name of ["hls.js", "shaka-player"])
  metadata.versions[name] = JSON.parse(
    await readFile(
      join(directory, "node_modules", name, "package.json"),
      "utf8",
    ),
  ).version;
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});
const pagePort = server.address().port;
try {
  for (const { scenario, format, player, iteration, manifest } of samplePlan(
    options,
  )) {
    const token = randomUUID();
    const url = new URL(`http://127.0.0.1:${pagePort}/`);
    url.search = new URLSearchParams({
      player,
      scenario,
      token,
      content: new URL(
        fixtures.entry(
          format,
          manifest,
          scenario.includes("large-playlist"),
          scenario.startsWith("memory"),
        ),
        url,
      ).href,
      ...(options.diagnostic ? { diagnostic: "1" } : {}),
    }).toString();
    try {
      const message = new Promise((resolve) => {
        pending = { token, resolve, hlsWorkerRequests: 0 };
      });
      const result = await runBrowser(chrome, url, message, {
        memory: scenario.startsWith("memory"),
      });
      if (result.error) {
        samples.push({
          player,
          format,
          scenario,
          iteration,
          manifest,
          error: result.error,
          marks: result.marks,
          diagnostics: result.diagnostics,
        });
        throw new Error(
          `${player}/${format}/${scenario}: ${result.error}; marks=${JSON.stringify(result.marks)}`,
        );
      }
      if (player === "hlsjs" && pending.hlsWorkerRequests === 0)
        throw new Error(
          "hls.js did not request its configured transmux worker",
        );
      const sample = {
        hlsWorkerRequests: pending.hlsWorkerRequests,
        format,
        iteration,
        manifest,
        ...result.result,
        ...(result.processMemory
          ? { processMemory: result.processMemory }
          : {}),
      };
      if (scenario.startsWith("memory")) {
        const initialized = sample.memorySnapshots.find(
          (x) => x.phase === "initialized",
        );
        const loaded = sample.processMemory.samples.filter(
          (x) => x.bytes !== undefined && x.time >= initialized.time,
        );
        if (loaded.length)
          sample.metrics.sampledLoadedProcessPeakPssBytes = Math.max(
            ...loaded.map((x) => x.bytes),
          );
        // Forced-GC checkpoints interrupt startup; never present their timing
        // as an ordinary performance sample.
        for (const key of Object.keys(sample.metrics))
          if (!key.endsWith("Bytes")) delete sample.metrics[key];
      }
      samples.push(sample);
      console.log(
        `${player.padEnd(6)} ${format.padEnd(6)} ${manifest.padEnd(9)} ${scenario.padEnd(8)} ${iteration}/${options.iterations} ${scenario.startsWith("memory") ? `steady: ${sample.metrics.steadyApplicationBytes} bytes` : `first frame: ${sample.metrics.initializeToFirstFrameMs.toFixed(1)} ms`}`,
      );
    } catch (error) {
      if (
        !samples.some(
          (sample) =>
            sample.player === player &&
            sample.format === format &&
            sample.scenario === scenario &&
            sample.manifest === manifest &&
            sample.iteration === iteration,
        )
      )
        samples.push({
          player,
          format,
          scenario,
          iteration,
          manifest,
          error: String(error),
        });
      throw error;
    } finally {
      pending = undefined;
    }
  }
} finally {
  try {
    await mkdir(dirname(options.report), { recursive: true });
    await writeFile(
      options.report,
      JSON.stringify({ metadata, samples }, null, 2),
    );
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}
const rows = [];
for (const scenario of options.scenarios)
  for (const format of options.formats)
    for (const manifest of options.manifests)
      for (const player of options.players) {
        const group = samples.filter(
          (sample) =>
            sample.player === player &&
            sample.format === format &&
            sample.manifest === manifest &&
            sample.scenario === scenario,
        );
        const row = { player, format, manifest, scenario, n: group.length };
        if (!scenario.startsWith("memory"))
          row.firstFrameMinMs = Number(
            Math.min(
              ...group.map((sample) => sample.metrics.initializeToFirstFrameMs),
            ).toFixed(2),
          );
        if (!scenario.startsWith("memory"))
          row.firstFrameMaxMs = Number(
            Math.max(
              ...group.map((sample) => sample.metrics.initializeToFirstFrameMs),
            ).toFixed(2),
          );
        for (const key of Object.keys(group[0]?.metrics ?? {})) {
          const values = group
            .map((sample) => sample.metrics[key])
            .sort((a, b) => a - b);
          const midpoint = Math.floor(values.length / 2);
          row[key] = Number(
            (values.length % 2
              ? values[midpoint]
              : (values[midpoint - 1] + values[midpoint]) / 2
            ).toFixed(2),
          );
        }
        rows.push(row);
      }
console.table(rows);
console.log(`Raw results: ${options.report}`);
