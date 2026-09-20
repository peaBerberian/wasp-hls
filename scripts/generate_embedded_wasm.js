/**
 * ============= generate_embedded_wasm =============
 *
 * == What is this?
 *
 * This file allows to generate a [huge] JavaScript file which embeds the
 * WaspHlsPlayer's WebAssembly file.
 *
 *
 * == Why?
 *
 * The Web API to instantiate a new WebAssembly module relies on having a
 * separate WebAssembly file which is loaded through an URL.
 *
 * This is still the recommended way of loading WaspHlsPlayer's WebAssembly
 * file, yet for quick tests and development having to store and serve a whole
 * separate file may be cumbersome to web developpers not used to handle such
 * kind of considerations.
 *
 * Hence, to facilitate developments, this script astuciously succeed to
 * allow WebAssembly loading without having to store the file separately.
 *
 * == How?
 *
 * We encode the WebAssembly file as base64, decode it into a `Uint8Array` at
 * module evaluation, and create a local Blob URL with the right
 * `"application/wasm"` Content-Type.
 *
 * This leads to a gigantic multi-megas file size, though it should compress
 * pretty well.
 *
 * Then, without knowing it, an application can just import that file and give
 * its default export to the `WaspHlsPlayer` as if it was the WebAssembly file's
 * URL (it basically still is).
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "path";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: node ./scripts/generate_embedded_wasm.js

Generates build/embedded/wasm.js and build/embedded/wasm.d.ts from
build/wasp_hls_bg.wasm.`);
  process.exit(0);
}

const originalWasmFilePath = path.join(
  import.meta.dirname,
  "../build/wasp_hls_bg.wasm",
);
const destinationDirPath = path.join(import.meta.dirname, "../build/embedded");
const destinationJsPath = path.join(destinationDirPath, "wasm.js");
const destinationDeclPath = path.join(destinationDirPath, "wasm.d.ts");
const declarationFile = `declare const EmbeddedWasm: string;
export default EmbeddedWasm;`;

const wasmData = await readFile(originalWasmFilePath);
const base64 = JSON.stringify(wasmData.toString("base64"));
const content = `const binary = atob(${base64});
const bytes = new Uint8Array(binary.length);
for (let i = 0; i < binary.length; i++) {
  bytes[i] = binary.charCodeAt(i);
}
const blobURL = URL.createObjectURL(new Blob([bytes], { type: "application/wasm" }));
export default blobURL;`;

await mkdir(destinationDirPath, { recursive: true });
await Promise.all([
  writeFile(destinationJsPath, content),
  writeFile(destinationDeclPath, declarationFile),
]);
