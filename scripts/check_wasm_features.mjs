/**
 * ============= check_wasm_features =============
 *
 * Validate the instructions in the final WebAssembly artifact against the
 * "WebAssembly MVP", which is the first published version of WebAssembly.
 *
 * This is the set base target for the project for now.
 */

import { existsSync } from "node:fs";
import { join } from "node:path";
import { exec } from "./utils/exec.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: node ./scripts/check_wasm_features.mjs

Checks that build/wasp_hls_bg.wasm contains only the features from the initial
version of WebAssembly, to maximize compatibility.`);
  process.exit(0);
}

const root = join(import.meta.dirname, "..");
const wasmFile = process.argv[2] ?? "build/wasp_hls_bg.wasm";
const localWasmOpt = join(root, "tmp", "binaryen", "bin", "wasm-opt");
const wasmOpt = existsSync(localWasmOpt) ? localWasmOpt : "wasm-opt";

if (!existsSync(join(root, wasmFile))) {
  throw new Error(`WebAssembly file not found: ${wasmFile}`);
}

// Remove target-feature declarations before validation so they cannot silently
// enable instructions outside the MVP allowlist.
await exec(
  wasmOpt,
  [
    wasmFile,
    "--strip-target-features",
    "--mvp-features",
    "-o",
    process.platform === "win32" ? "NUL" : "/dev/null",
  ],
  { cwd: root },
);
console.log(`Validated ${wasmFile}: WebAssembly MVP`);
