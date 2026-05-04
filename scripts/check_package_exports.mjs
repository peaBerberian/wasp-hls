#!/usr/bin/env node
/**
 * # check_package_exports.mjs
 *
 * This file allows to validate the consumer-facing npm package shape exposed by
 * this repository.
 *
 * You can either run it directly as a script (run
 * `node check_package_exports.mjs -h` to see the different options) or by
 * requiring/importing it as a node module.
 * If doing the latter you will obtain a function returning a Promise resolving
 * once the package shape has been validated, and rejecting with an Error
 * otherwise.
 */

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "fs";
import { execFileSync } from "child_process";
import { build as esbuild } from "esbuild";
import { join, resolve } from "path";
import { tmpdir } from "os";
import { fileURLToPath, pathToFileURL } from "url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));

/**
 * Validate the npm package exports and embedded entrypoints from a packed
 * tarball, as a consumer would see them.
 * @returns {Promise<void>}
 */
export default async function checkPackageExports() {
  const rootPackageJson = JSON.parse(
    readFileSync(join(repoRoot, "package.json"), "utf8"),
  );
  const expectedTarballName = `${rootPackageJson.name
    .replace(/^@/, "")
    .replace(/\//g, "-")}-${rootPackageJson.version}.tgz`;
  const tempDir = mkdtempSync(join(tmpdir(), "wasp-hls-pack-check-"));
  const npmCacheDir = join(tempDir, "npm-cache");
  const packDir = join(tempDir, "pack");
  const unpackDir = join(tempDir, "unpacked");
  const consumerDir = join(tempDir, "consumer");
  let repoTarballPath = null;

  try {
    mkdirSync(npmCacheDir, { recursive: true });
    mkdirSync(packDir, { recursive: true });
    mkdirSync(unpackDir, { recursive: true });
    mkdirSync(consumerDir, { recursive: true });

    repoTarballPath = join(repoRoot, expectedTarballName);
    rmSync(repoTarballPath, { force: true });

    const npmPackOutput = execOutsideRepo(
      "npm",
      ["pack", "--silent"],
      repoRoot,
      npmCacheDir,
    ).trim();
    const filename = getPackedFilename(expectedTarballName, npmPackOutput);

    copyFileSync(repoTarballPath, join(packDir, filename));
    const packagedFiles = new Set(
      execOutsideRepo("tar", ["-tzf", repoTarballPath], repoRoot, npmCacheDir)
        .split("\n")
        .filter((entry) => entry.startsWith("package/"))
        .map((entry) => entry.slice("package/".length))
        .filter(Boolean),
    );
    const requiredFiles = [
      "build/es6/index.js",
      "build/es6/index.d.ts",
      "build/embedded/wasm.js",
      "build/embedded/wasm.d.ts",
      "build/embedded/worker.js",
      "build/embedded/worker.d.ts",
      "build/worker.js",
      "build/wasp_hls_bg.wasm",
    ];
    for (const requiredFile of requiredFiles) {
      if (!packagedFiles.has(requiredFile)) {
        throw new Error(`Packed tarball is missing "${requiredFile}".`);
      }
    }

    execOutsideRepo(
      "tar",
      ["-xzf", repoTarballPath, "-C", unpackDir],
      repoRoot,
      npmCacheDir,
    );
    const unpackedPackageDir = join(unpackDir, "package");
    const unpackedPackageJson = JSON.parse(
      readFileSync(join(unpackedPackageDir, "package.json"), "utf8"),
    );
    assertExports(unpackedPackageJson);

    writeFileSync(
      join(consumerDir, "package.json"),
      JSON.stringify(
        {
          name: "wasp-hls-package-check",
          private: true,
          type: "module",
        },
        null,
        2,
      ),
    );
    execOutsideRepo(
      "npm",
      ["install", join(packDir, filename)],
      consumerDir,
      npmCacheDir,
    );

    const entryFile = join(consumerDir, "entry.js");
    writeFileSync(
      entryFile,
      `import WaspHlsPlayer from "wasp-hls";
import EmbeddedWasm from "wasp-hls/wasm";
import EmbeddedWorker from "wasp-hls/worker";

void [WaspHlsPlayer, EmbeddedWasm, EmbeddedWorker];
`,
    );

    await esbuild({
      entryPoints: [entryFile],
      bundle: true,
      format: "esm",
      platform: "browser",
      outfile: join(consumerDir, "bundle.js"),
      logLevel: "silent",
    });
  } finally {
    if (repoTarballPath !== null) {
      rmSync(repoTarballPath, { force: true });
    }
    rmSync(tempDir, { recursive: true, force: true });
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
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

  if (args.length > 0) {
    console.error(`ERROR: unknown option: "${args[0]}"\n`);
    displayHelp();
    process.exit(1);
  }

  try {
    await checkPackageExports();
    console.log("Package exports validated from the packed tarball.");
  } catch (error) {
    console.error("Package export validation failed.");
    if (error instanceof Error) {
      console.error(error.message);
    } else {
      console.error(error);
    }
    process.exit(1);
  }
}

/**
 * @param {string} expectedTarballName
 * @param {string} npmPackOutput
 * @returns {string}
 */
function getPackedFilename(expectedTarballName, npmPackOutput) {
  const trimmedOutput = npmPackOutput.trim();
  if (trimmedOutput.length > 0 && trimmedOutput.endsWith(".tgz")) {
    return trimmedOutput;
  }
  return expectedTarballName;
}

/**
 * Execute a command and return its stdout, throwing with richer diagnostics if
 * it fails.
 * @param {string} command
 * @param {string[]} args
 * @param {string} cwd
 * @param {string} npmCacheDir
 * @returns {string}
 */
function execOutsideRepo(command, args, cwd, npmCacheDir) {
  try {
    return execFileSync(command, args, {
      cwd,
      encoding: "utf8",
      env: {
        ...process.env,
        npm_config_cache: npmCacheDir,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    if (error instanceof Error && "stdout" in error && "stderr" in error) {
      const stdout =
        typeof error.stdout === "string" && error.stdout.length > 0
          ? `\nstdout:\n${error.stdout}`
          : "";
      const stderr =
        typeof error.stderr === "string" && error.stderr.length > 0
          ? `\nstderr:\n${error.stderr}`
          : "";
      throw new Error(`${error.message}${stdout}${stderr}`);
    }
    throw error;
  }
}

/**
 * @param {Record<string, unknown>} packageJson
 */
function assertExports(packageJson) {
  const expectedExports = {
    ".": {
      types: "./build/es6/index.d.ts",
      default: "./build/es6/index.js",
    },
    "./wasm": {
      types: "./build/embedded/wasm.d.ts",
      default: "./build/embedded/wasm.js",
    },
    "./worker": {
      types: "./build/embedded/worker.d.ts",
      default: "./build/embedded/worker.js",
    },
  };

  if (JSON.stringify(packageJson.exports) !== JSON.stringify(expectedExports)) {
    throw new Error(
      "Unexpected package exports.\n" +
        `Expected: ${JSON.stringify(expectedExports)}\n` +
        `Received: ${JSON.stringify(packageJson.exports)}`,
    );
  }
}

/**
 * Display the CLI help.
 */
function displayHelp() {
  console.log(`Check the consumer-facing npm package exports.

Usage:
  node ./scripts/check_package_exports.mjs

What it does:
  1. Packs the current repository with npm.
  2. Verifies the tarball contains the expected public artifacts.
  3. Checks package.json exports for ".", "./wasm" and "./worker".
  4. Installs that tarball into a temporary consumer project.
  5. Bundles imports of "wasp-hls", "wasp-hls/wasm" and "wasp-hls/worker".

Options:
  -h, --help   Show this help message.
`);
}
