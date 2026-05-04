import { build } from "esbuild";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const generatedDir = path.join(repoRoot, "tmp", "wasm-bindgen");
const shippedDir = path.join(repoRoot, "src", "wasm");
const generatedJsPath = path.join(generatedDir, "wasp_hls.js");
const patchedJsPath = path.join(generatedDir, "wasp_hls.patched.js");

mkdirSync(shippedDir, { recursive: true });

const rawBindings = readFileSync(generatedJsPath, "utf8");
const patchedBindings = rawBindings.replace(
  "      module_or_path = new URL('wasp_hls_bg.wasm', import.meta.url);",
  '      throw new Error("wasp_hls init requires an explicit module or path.");',
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
