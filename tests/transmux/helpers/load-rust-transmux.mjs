import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

/** Use the real wrapper and compiled WASM; fail if parsing calls any host API. */
export async function loadTransmuxers(directory) {
  const outfile = join(directory, "transmuxers.mjs");
  await build({
    entryPoints: ["tests/transmux/helpers/rust-transmux-test-entry.ts"],
    bundle: true,
    format: "esm",
    platform: "node",
    outfile,
    logLevel: "silent",
  });
  const mod = await import(pathToFileURL(outfile).href);
  const module = new WebAssembly.Module(
    await readFile("tmp/transmux-test.wasm"),
  );
  const imports = {};
  for (const item of WebAssembly.Module.imports(module)) {
    assert.equal(item.kind, "function");
    (imports[item.module] ??= {})[item.name] = () => {
      throw new Error(`Unexpected host call: ${item.name}`);
    };
  }
  const wasm = new WebAssembly.Instance(module, imports).exports;
  mod.setWasmExports(wasm);
  return { ...mod, wasm };
}
