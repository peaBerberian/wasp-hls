/**
 * Build helpers for wasm, worker, main-thread, demo, and package artifacts.
 */

import { copyFileSync, mkdirSync, readFileSync, rmSync } from "fs";
import { dirname, join } from "path";
import { exec, npmExecCommand } from "../utils/exec.mjs";
import { checkPackageExports } from "./check-package-exports.mjs";
import { reportStep } from "./report.mjs";

const NODE = process.execPath;

/**
 * @param {string} root
 * @param {{ release: boolean, skipGenerate?: boolean }} options
 */
export async function buildWasm(root, { release, skipGenerate = false }) {
  if (!skipGenerate) {
    await generateWasmAbi(root);
  }
  reportStep(
    "BUILD",
    "Building WebAssembly module" +
      (release ? " in release mode..." : " in debug mode..."),
  );
  await exec(
    "cargo",
    [
      "+nightly",
      "build",
      "-Zbuild-std=std,panic_abort",
      ...(release ? ["--release"] : []),
      "--target=wasm32-unknown-unknown",
    ],
    {
      cwd: root,
      env: {
        ...process.env,
        RUSTFLAGS: [process.env.RUSTFLAGS, "-Ctarget-cpu=mvp"]
          .filter(Boolean)
          .join(" "),
      },
    },
  );
  const mode = release ? "release" : "debug";
  const source = join(
    root,
    "target",
    "wasm32-unknown-unknown",
    mode,
    "wasp_hls.wasm",
  );
  const destinations = [
    join(root, "src", "wasm", "wasp_hls_bg.wasm"),
    join(root, "build", "wasp_hls_bg.wasm"),
  ];
  mkdirSync(join(root, "build"), { recursive: true });
  for (const destination of destinations) {
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(source, destination);
  }
  reportStep("BUILD", "Checking ABI correctness...");
  await exec(NODE, ["./scripts/check_wasm_abi.mjs"], { cwd: root });
  if (release) {
    reportStep("BUILD", "Stripping WASM debug symbols...");
    await exec(NODE, ["./scripts/wasm-strip.js", "build/wasp_hls_bg.wasm"], {
      cwd: root,
    });
  }
  reportStep("BUILD", "Validating WebAssembly MVP features...");
  await exec(
    NODE,
    ["./scripts/check_wasm_features.mjs", "build/wasp_hls_bg.wasm"],
    {
      cwd: root,
    },
  );
}

/**
 * @param {string} root
 * @param {{ release: boolean }} options
 */
export async function buildWorker(root, { release }) {
  const args = [
    join(root, "src/ts-worker/index.ts"),
    "--bundle",
    "--outfile=build/worker.js",
    `--tsconfig=${join(root, "src/ts-worker/tsconfig.json")}`,
  ];
  if (release) {
    args.push("--minify");
  }
  reportStep("BUILD", "building worker file...");
  const esbuild = npmExecCommand("esbuild", args);
  await exec(esbuild.command, esbuild.args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ release: boolean }} options
 */
export async function buildMain(root, { release }) {
  const args = [
    join(root, "src/ts-main/index.ts"),
    "--bundle",
    "--outfile=build/main.js",
    `--tsconfig=${join(root, "src/ts-main/tsconfig.json")}`,
  ];
  if (release) {
    args.push("--minify");
  }
  reportStep("BUILD", "building main file bundle...");
  const esbuild = npmExecCommand("esbuild", args);
  await exec(esbuild.command, esbuild.args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ release: boolean }} options
 */
export async function buildDemoBundle(root, { release }) {
  const args = [
    join(root, "demo/src/index.tsx"),
    "--bundle",
    "--outfile=build/demo.js",
    `--tsconfig=${join(root, "demo/tsconfig.json")}`,
  ];
  if (release) {
    args.push("--minify");
  }
  reportStep("BUILD", "building demo bundle...");
  const esbuild = npmExecCommand("esbuild", args);
  await exec(esbuild.command, esbuild.args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ release: boolean }} options
 */
export async function buildAll(root, { release }) {
  await generateWasmAbi(root);
  await buildWasm(root, { release, skipGenerate: true });
  await buildWorker(root, { release });
  reportStep("BUILD", "removing previous ES6 build artefacts...");
  rmSync(join(root, "build", "es6"), { force: true, recursive: true });
  rmSync(join(root, "build", "embedded"), { force: true, recursive: true });
  reportStep("BUILD", "generating ES6 build...");
  const tsc = npmExecCommand("tsc", [
    "-p",
    join(root, "src/ts-main/tsconfig.json"),
    "--rootDir",
    join(root, "src"),
    "--outDir",
    "./build/es6",
  ]);
  await exec(tsc.command, tsc.args, { cwd: root });
  copyFileSync(
    join(root, "src", "wasm", "wasp_hls_bg.wasm"),
    join(root, "build", "es6", "wasm", "wasp_hls_bg.wasm"),
  );
  reportStep("BUILD", "generating Embedded WASM JS file...");
  await exec(NODE, ["./scripts/generate_embedded_wasm.js"], { cwd: root });
  reportStep("BUILD", "generating Embedded Worker JS file...");
  await exec(NODE, ["./scripts/generate_embedded_worker.js"], { cwd: root });
  if (release) {
    reportStep("BUILD", "checking all exports...");
    await checkPackageExports(root);
  }
}

/**
 * @param {string} root
 */
export async function buildDocs(root) {
  reportStep("BUILD", "removing previous generated doc...");
  rmSync(join(root, "doc", "generated"), { force: true, recursive: true });
  reportStep("BUILD", "generating new doc...");
  const packageJson = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  );
  const version = packageJson.version;
  const readmeDoc = npmExecCommand("readme.doc", [
    "--input",
    "doc",
    "--output",
    "build/doc",
    "-p",
    version,
  ]);
  await exec(readmeDoc.command, readmeDoc.args, {
    cwd: root,
  });
}

/**
 * @param {string} root
 */
export async function generateWasmAbi(root) {
  reportStep("BUILD", "Generating ABI enums...");
  await exec(NODE, ["./scripts/generate_wasm_abi_enums.mjs"], { cwd: root });
  reportStep("BUILD", "Generating ABI bindings...");
  await exec(NODE, ["./scripts/generate_wasm_abi_bindings.mjs"], { cwd: root });
}
