import { exec, spawn } from "child_process";
import * as fs from "fs";
import * as fsProm from "fs/promises";
import * as path from "path";
import { fileURLToPath } from "url";
import { build as esbuild } from "esbuild";

/** @typedef {{text: string, virtualPath: string}} VirtualFile */
export const rootDirectory = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export const currentDirectory = path.join(
  rootDirectory,
  "tests",
  "performance",
);
const performanceSuitesDirectory = path.join(currentDirectory, "src", "suites");

/**
 * @param {{branchName: string, remoteGitUrl?: string, contentServerPort: number, performanceTestFiles: string[], innerIterations: number}} options
 * @returns {Promise<void>}
 */
export async function initializePerformanceTestsPages({
  branchName,
  remoteGitUrl,
  contentServerPort,
  performanceTestFiles,
  innerIterations,
}) {
  const performanceTestsEntry =
    createPerformanceTestsEntry(performanceTestFiles);
  await prepareLastWaspHlsTests({
    branchName,
    remoteGitUrl,
    contentServerPort,
    performanceTestsEntry,
    innerIterations,
  });
  await prepareCurrentWaspHlsTests({
    contentServerPort,
    performanceTestsEntry,
    innerIterations,
  });
}

/**
 * @param {{contentServerPort: number, performanceTestsEntry: VirtualFile, innerIterations: number}} options
 * @returns {Promise<void>}
 */
async function prepareCurrentWaspHlsTests({
  contentServerPort,
  performanceTestsEntry,
  innerIterations,
}) {
  await linkCurrentWaspHls();
  await createBundle({
    output: "current.js",
    contentServerPort,
    input: performanceTestsEntry,
    minify: true,
    innerIterations,
  });
}

/**
 * @param {{branchName: string, remoteGitUrl?: string, contentServerPort: number, performanceTestsEntry: VirtualFile, innerIterations: number}} options
 * @returns {Promise<void>}
 */
async function prepareLastWaspHlsTests({
  branchName,
  contentServerPort,
  remoteGitUrl,
  performanceTestsEntry,
  innerIterations,
}) {
  await linkWaspHlsBranch({ branchName, remoteGitUrl });
  await createBundle({
    contentServerPort,
    input: performanceTestsEntry,
    output: "previous.js",
    minify: true,
    innerIterations,
  });
  await fsProm.copyFile(
    path.join(currentDirectory, "previous.js"),
    path.join(currentDirectory, "control-current.js"),
  );
}

/**
 * Link the current Wasp HLS to the performance tests, so its performance can be
 * tested.
 * @returns {Promise<void>}
 */
async function linkCurrentWaspHls() {
  const rootDir = rootDirectory;
  const innerNodeModulesPath = path.join(currentDirectory, "node_modules");
  await fsProm.rm(innerNodeModulesPath, { force: true, recursive: true });
  await fsProm.mkdir(innerNodeModulesPath, { recursive: true });
  const waspHlsPath = path.join(innerNodeModulesPath, "wasp-hls");
  await fsProm.mkdir(path.join(rootDir, "build", "es6", "wasm"), {
    recursive: true,
  });
  await spawnProc("npm", ["run", "build", "--", "--release"], {
    parseError: (code) => new Error(`npm run build exited with code ${code}`),
  }).promise;
  await fsProm.symlink(rootDir, waspHlsPath);
}

/**
 * @param {{branchName: string, remoteGitUrl?: string}} options
 * @returns {Promise<void>}
 */
async function linkWaspHlsBranch({ branchName, remoteGitUrl }) {
  const innerNodeModulesPath = path.join(currentDirectory, "node_modules");
  await fsProm.rm(innerNodeModulesPath, { force: true, recursive: true });
  await fsProm.mkdir(innerNodeModulesPath, { recursive: true });
  const waspHlsPath = path.join(innerNodeModulesPath, "wasp-hls");
  let url =
    remoteGitUrl ??
    (await execCommandAndGetFirstOutput("git config --get remote.origin.url"));
  url = url.trim();
  await spawnProc(
    "git",
    ["clone", "--depth", "1", "-b", branchName, url, waspHlsPath],
    {
      parseError: (code) => new Error(`git clone exited with code ${code}`),
    },
  ).promise;
  await spawnProc("npm", ["install"], {
    cwd: waspHlsPath,
    parseError: (code) => new Error(`npm install failed with code ${code}`),
  }).promise;
  await fsProm.mkdir(path.join(waspHlsPath, "build", "es6", "wasm"), {
    recursive: true,
  });
  const binaryenDirectory = path.join(rootDirectory, "tmp", "binaryen", "bin");
  const env = { ...process.env };
  if (fs.existsSync(path.join(binaryenDirectory, "wasm-opt"))) {
    env.PATH = [binaryenDirectory, env.PATH]
      .filter(Boolean)
      .join(path.delimiter);
  }
  await spawnProc("npm", ["run", "build", "--", "--release"], {
    cwd: waspHlsPath,
    env,
    parseError: (code) => new Error(`npm run build exited with code ${code}`),
  }).promise;
}

/**
 * Build the performance tests.
 * @param {Object} options
 * @param {string} options.output - The output file
 * @param {number} options.contentServerPort - Port on which media content is
 * served.
 * @param {VirtualFile} options.input
 * @param {boolean} [options.minify] - If `true`, the output will be minified.
 * @param {number} options.innerIterations - Page visits per browser process.
 * @returns {Promise<void>}
 */
async function createBundle(options) {
  try {
    await esbuild({
      stdin: {
        contents: options.input.text,
        resolveDir: path.dirname(options.input.virtualPath),
        sourcefile: path.basename(options.input.virtualPath),
      },
      bundle: true,
      minify: !!options.minify,
      platform: "browser",
      target: "es2020",
      format: "iife",
      outfile: path.join(currentDirectory, options.output),
      define: {
        __TEST_CONTENT_SERVER__: JSON.stringify({
          URL: "127.0.0.1",
          PORT: String(options.contentServerPort),
        }),
        __PERFORMANCE_INNER_ITERATIONS__: JSON.stringify(
          options.innerIterations,
        ),
      },
      logLevel: "silent",
    });
  } catch (err) {
    throw new Error(`Performance build failed: ${err}`);
  }
}

/**
 * @param {string} command
 * @param {Array.<string>} args
 * @param {Object} [params]
 * @param {string|undefined} [params.cwd]
 * @param {(code: number | null) => Error} [params.parseError]
 * @param {NodeJS.ProcessEnv} [params.env]
 */
function spawnProc(command, args, { cwd, parseError, env } = {}) {
  let child;
  const prom = new Promise(
    /** @param {(value?: void) => void} res */ (res, rej) => {
      child = spawn(command, args, { cwd, env, stdio: "inherit" });
      child.on("close", (code) => {
        if (code !== 0 && typeof parseError === "function") {
          rej(parseError(code));
        }
        res();
      });
    },
  );
  return {
    promise: prom,
    child,
  };
}

/** @param {string} command
 * @returns {Promise<string>} */
function execCommandAndGetFirstOutput(command) {
  return new Promise((res, rej) => {
    exec(command, (error, stdout) => {
      if (error) {
        rej(error);
      } else {
        res(stdout);
      }
    });
  });
}

/**
 * @param {string[]} requestedPaths
 * @returns {string[]}
 */
export function getPerformanceTestFiles(requestedPaths) {
  const pathsToInspect =
    requestedPaths.length === 0 ? [performanceSuitesDirectory] : requestedPaths;
  const testFiles = [];

  for (const requestedPath of pathsToInspect) {
    const resolvedPath = path.resolve(requestedPath);
    const relativePath = path.relative(
      performanceSuitesDirectory,
      resolvedPath,
    );
    if (
      relativePath.startsWith(".." + path.sep) ||
      path.isAbsolute(relativePath)
    ) {
      throw new Error(
        `Performance test path is outside src/suites: ${requestedPath}`,
      );
    }
    let stats;
    try {
      stats = fs.statSync(resolvedPath);
    } catch {
      throw new Error(`Performance test path does not exist: ${requestedPath}`);
    }
    if (stats.isDirectory()) {
      for (const entry of fs.readdirSync(resolvedPath, {
        withFileTypes: true,
      })) {
        testFiles.push(
          ...getPerformanceTestFiles([path.join(resolvedPath, entry.name)]),
        );
      }
    } else if (stats.isFile() && path.extname(resolvedPath) === ".js") {
      testFiles.push(resolvedPath);
    }
  }

  const uniqueTestFiles = [...new Set(testFiles)].sort();
  if (uniqueTestFiles.length === 0) {
    throw new Error("No JavaScript performance test file found");
  }
  return uniqueTestFiles;
}

/**
 * @param {string[]} testFiles
 * @returns {VirtualFile}
 */
function createPerformanceTestsEntry(testFiles) {
  const imports = testFiles.map((testFile) => {
    const relativePath =
      "./" +
      path.relative(currentDirectory, testFile).split(path.sep).join("/");
    return `  import(${JSON.stringify(relativePath)})`;
  });
  const text = `import { error } from "./src/lib.js";

// Keep those imports dynamic so esbuild does not tree-shake the test suites.
Promise.all([
${imports.join(",\n")}
]).catch((err) => {
  error("Could not load performance tests:", err instanceof Error ? String(err) : "Unknown error");
});
`;
  return {
    text,
    virtualPath: path.join(currentDirectory, "performance_tests.js"),
  };
}
