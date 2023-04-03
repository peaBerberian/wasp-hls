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
 * The exact way may seem pretty ugly: We're here converting the whole
 * WebAssembly binary file into a `Uint8Array` construction, then creating a
 * local URL through the `Object.createObjectURL` Web API to make it point to
 * that Uint8Array with the right `"application/wasm"` Content-Type, and then
 * export the URL.
 *
 * This leads to a gigantic multi-megas file size, though it should compress
 * pretty well.
 *
 * Then, without knowing it, an application can just import that file and give
 * its default export to the `WaspHlsPlayer` as if it was the WebAssembly file's
 * URL (it basically still is).
 */

const fs = require("fs");
const path = require("path");

const originalWasmFilePath = path.join(__dirname, "../build/wasp_hls_bg.wasm");
const destinationPath = path.join(__dirname, "../wasm.js");

const codePrefix = "const blobURL = URL.createObjectURL(new Blob([";
const codeSuffix = `], { type: "application/wasm" }));
export default blobURL;`;

fs.readFile(originalWasmFilePath, { encoding: null }, function (err, data) {
  if (err) {
    console.error(`Error while reading "${originalWasmFilePath}":`, err);
  } else {
    const u8Arr = new Uint8Array(data);
    const jsDataStr = `new Uint8Array([${u8Arr.toString()}])`;
    const content = codePrefix + jsDataStr + codeSuffix;
    fs.writeFile(destinationPath, content, (err) => {
      if (err) {
        console.error(`Error while writing "${destinationPath}":`, err);
      }
      // file written successfully
    });
  }
});
