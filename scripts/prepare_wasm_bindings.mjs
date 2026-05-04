#!/usr/bin/env node
/**
 * # prepare_wasm_bindings.mjs
 *
 * This file allows to prepare the shipped JavaScript bindings from the current
 * `wasm-bindgen` output so they match this project's runtime expectations.
 *
 * In particular it is used to patch the generated wasm loading behavior and to
 * bundle/transpile the resulting runtime to an ES2017-compatible ESM file.
 *
 * You can either run it directly as a script (run
 * `node prepare_wasm_bindings.mjs -h` to see the different options) or by
 * requiring/importing it as a node module.
 * If doing the latter you will obtain a function returning a Promise
 * resolving once the wasm bindings have been prepared, and rejecting with an
 * Error otherwise.
 */

import { build } from "esbuild";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const defaultGeneratedDir = path.join(repoRoot, "tmp", "wasm-bindgen");
const defaultShippedDir = path.join(repoRoot, "src", "wasm");
const wasmUrlFallbackSnippet =
  "      module_or_path = new URL('wasp_hls_bg.wasm', import.meta.url);";
const explicitWasmPathErrorSnippet =
  '      throw new Error("wasp_hls init requires an explicit module or path.");';

/**
 * Prepare the shipped wasm bindings from the generated `wasm-bindgen` output.
 *
 * This patches the generated runtime's implicit wasm URL fallback and emits an
 * ES2017-compatible bundled runtime for the repository.
 * @param {object} [options]
 * @param {string} [options.generatedDir]
 * @param {string} [options.shippedDir]
 * @returns {Promise<void>}
 */
export default async function prepareWasmBindings({
  generatedDir = defaultGeneratedDir,
  shippedDir = defaultShippedDir,
} = {}) {
  const generatedJsPath = path.join(generatedDir, "wasp_hls.js");
  const patchedJsPath = path.join(generatedDir, "wasp_hls.patched.js");

  mkdirSync(shippedDir, { recursive: true });

  const rawBindings = readFileSync(generatedJsPath, "utf8");
  if (!rawBindings.includes(wasmUrlFallbackSnippet)) {
    throw new Error(
      "Could not find wasm-bindgen's implicit wasm URL fallback in " +
        `"${generatedJsPath}". The generated bindings shape may have changed.`,
    );
  }
  const patchedBindings = rawBindings.replace(
    wasmUrlFallbackSnippet,
    explicitWasmPathErrorSnippet,
  );
  writeFileSync(patchedJsPath, patchedBindings);

  await build({
    entryPoints: [patchedJsPath],
    outfile: path.join(shippedDir, "wasp_hls.js"),
    bundle: true,
    format: "esm",
    target: "es2017",
    logLevel: "warning",
  });

  cpSync(
    path.join(generatedDir, "wasp_hls_bg.wasm"),
    path.join(shippedDir, "wasp_hls_bg.wasm"),
  );
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  runFromCli(process.argv.slice(2));
}

/**
 * Run the script from the command line.
 * @param {string[]} args
 * @returns {Promise<void>}
 */
async function runFromCli(args) {
  if (args[0] === "-h" || args[0] === "--help") {
    displayHelp();
    process.exit(0);
  }

  const options = parseCliArgs(args);

  try {
    await prepareWasmBindings(options);
    console.log("Prepared shipped wasm bindings.");
  } catch (error) {
    console.error("Wasm bindings preparation failed.");
    if (error instanceof Error) {
      console.error(error.message);
    } else {
      console.error(error);
    }
    process.exit(1);
  }
}

/**
 * @param {string[]} args
 * @returns {{ generatedDir?: string; shippedDir?: string; }}
 */
function parseCliArgs(args) {
  const options = {};

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--generated-dir") {
      const value = args[++i];
      if (value === undefined) {
        throwCliError('missing value for "--generated-dir"');
      }
      options.generatedDir = path.resolve(repoRoot, value);
    } else if (arg === "--shipped-dir") {
      const value = args[++i];
      if (value === undefined) {
        throwCliError('missing value for "--shipped-dir"');
      }
      options.shippedDir = path.resolve(repoRoot, value);
    } else {
      throwCliError(`unknown option: "${arg}"`);
    }
  }

  return options;
}

/**
 * @param {string} message
 */
function throwCliError(message) {
  console.error(`ERROR: ${message}\n`);
  displayHelp();
  process.exit(1);
}

/**
 * Display the CLI help.
 */
function displayHelp() {
  console.log(`Prepare shipped JS/wasm bindings from wasm-bindgen output.

Usage:
  node ./scripts/prepare_wasm_bindings.mjs [options]

What it does:
  1. Reads the generated tmp/wasm-bindgen/wasp_hls.js file.
  2. Replaces wasm-bindgen's implicit wasm URL fallback with an explicit error.
  3. Re-bundles that runtime to an ES2017 ESM file in src/wasm/wasp_hls.js.
  4. Copies the corresponding wasm binary to src/wasm/wasp_hls_bg.wasm.

Options:
  --generated-dir <path>  Directory containing wasm-bindgen output.
  --shipped-dir <path>    Directory where prepared bindings should be written.
  -h, --help              Show this help message.
`);
}
