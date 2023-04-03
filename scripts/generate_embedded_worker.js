/**
 * ============= generate_embedded-worker =============
 *
 * == What is this?
 *
 * This file allows to generate a JavaScript file which embeds the
 * WaspHlsPlayer's Worker file.
 *
 *
 * == Why?
 *
 * The Web API to create a WebWorker relies on having a separate JavaScript
 * file containing the Worker's code, which is loaded through an URL.
 *
 * This is still the recommended way of loading WaspHlsPlayer's Worker
 * file, yet for quick tests and development having to store and serve a whole
 * separate file may be cumbersome to web developpers not used to handle such
 * kind of considerations.
 *
 * Hence, to facilitate developments, this script astuciously succeed to
 * allow Worker loading without having to store the file separately.
 *
 * == How?
 *
 * We're here embedding the whole Worker's code into an IFEE, with the Worker's
 * own code embedded in it. The Worker code is not in JavaScript's string form
 * but as its real, parseable, JavaScript form - to allow the application's
 * build and minify steps to still be able to transform it.
 * Yet, to both prevent being polluted by the application's realm and to allow
 * URL creation, the whole IFEE is stringified. We do so at runtime to still
 * keep the advantage of having the real parseable JavaScript code initially.
 *
 * Then, that stringified IFEE is transformed into a local URL through the
 * `Object.createObjectURL` Web API to make it point to it with the right
 * `"application/javascript"` Content-Type, and then export the URL.
 *
 * Then, without knowing it, an application can just import that file and give
 * its default export to the `WaspHlsPlayer` as if it was the Worker file's
 * URL (it basically still is).
 */
const fs = require("fs");
const path = require("path");

const originalWorkerFilePath = path.join(__dirname, "../build/worker.js");
const destinationPath = path.join(__dirname, "../worker.js");

const codePrefix = `const blobURL = URL.createObjectURL(new Blob([ "(",
function(){`;
const codeSuffix = `}.toString(),
")()" ], { type: "application/javascript" }));
export default blobURL;`;

fs.readFile(
  originalWorkerFilePath,
  { encoding: "utf-8" },
  function (err, data) {
    if (err) {
      console.error(`Error while reading "${originalWorkerFilePath}":`, err);
    } else {
      const content = codePrefix + data + codeSuffix;
      fs.writeFile(destinationPath, content, (err) => {
        if (err) {
          console.error(`Error while writing "${destinationPath}":`, err);
        }
        // file written successfully
      });
    }
  }
);
