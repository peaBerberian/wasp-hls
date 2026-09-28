/**
 * Unified test helpers for Node, browser integration, and Rust unit tests.
 */

import { readdir } from "fs/promises";
import { join, relative } from "path";
import { exec } from "../utils/exec.mjs";
import { reportStep } from "./report.mjs";

const TESTS_DIRECTORY = "tests";
const TRANSMUX_DIRECTORY = join(TESTS_DIRECTORY, "transmux");
const NODE_TEST_FILE_PATTERN = /\.test\.(?:mjs|js)$/;
const NODE = process.execPath;

/**
 * @param {string} root
 * @param {string[]} args
 */
export async function testPerformance(root, args) {
  reportStep("TEST", "running browser performance comparison...");
  await exec(NODE, ["tests/performance/run.mjs", ...args], {
    cwd: root,
  });
}

/** @param {string} root */
export async function testAll(root) {
  await testRust(root, { filters: [] });
  await testTransmux(root, { filters: [], watch: false });
  await testBrowserSuite(root, {
    browser: undefined,
    filters: [],
    suite: "integration",
    watch: false,
  });
}

/**
 * @param {string} root
 * @param {{ filters: string[] }} options
 */
export async function testRust(root, { filters }) {
  reportStep("TEST", "running Rust unit tests...");
  const args = ["test"];
  if (filters.length > 0) {
    args.push("--", ...filters);
  }
  await exec("cargo", args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ filters: string[], watch: boolean }} options
 */
export async function testTransmux(root, { filters, watch }) {
  const allTests = await collectNodeTestFiles(
    root,
    join(root, TRANSMUX_DIRECTORY),
  );
  const selectedTests = selectTests(allTests, filters);

  if (selectedTests.length === 0) {
    throw new Error("No transmux test files matched the requested filters.");
  }

  reportStep(
    "TEST",
    watch ? "watching transmux tests..." : "running transmux tests...",
  );

  const args = ["--test"];
  if (watch) {
    args.push("--watch");
  }
  args.push(...selectedTests);
  await exec(NODE, args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ browser?: string, filters: string[], watch: boolean }} options
 */
export async function testIntegration(root, { browser, filters, watch }) {
  await testBrowserSuite(root, {
    browser,
    filters,
    suite: "integration",
    watch,
  });
}

/**
 * @param {string} root
 * @param {{ browser?: string, filters: string[], watch: boolean }} options
 */
export async function testMemory(root, { browser, filters, watch }) {
  if (watch) {
    throw new Error("Memory tests do not support watch mode.");
  }
  const memoryTestFile = "tests/memory/index.test.js";
  if (
    filters.length > 0 &&
    !filters.some((filter) => memoryTestFile.includes(filter))
  ) {
    throw new Error("No memory test files matched the requested filters.");
  }

  reportStep("TEST", "running browser memory tests...");
  const args = ["tests/testing-lib/simple-test-runner.mjs"];
  if (browser != null) {
    args.push("--browser", browser);
  }
  args.push(memoryTestFile);
  await exec(NODE, args, { cwd: root });
}

/**
 * @param {string} root
 * @param {{ browser?: string, filters: string[], suite: "integration" | "memory", watch: boolean }} options
 */
async function testBrowserSuite(root, { browser, filters, suite, watch }) {
  reportStep(
    "TEST",
    watch
      ? `watching browser ${suite} tests...`
      : `running browser ${suite} tests...`,
  );

  const args = ["./scripts/run_integration_tests.mjs"];
  if (suite === "memory") {
    args.push("--memory");
  }
  if (browser != null) {
    args.push("--browser", browser);
  }
  if (watch) {
    args.push("--watch");
  }
  for (const filter of filters) {
    args.push("--filter", filter);
  }
  await exec(NODE, args, { cwd: root });
}

/**
 * @param {string} root
 * @param {string} testsDirectory
 * @returns {Promise<string[]>}
 */
async function collectNodeTestFiles(root, testsDirectory) {
  /** @type {string[]} */
  const discoveredFiles = [];
  await walkTestsDirectory(root, testsDirectory, discoveredFiles);
  return discoveredFiles.sort();
}

/**
 * @param {string[]} files
 * @param {string[]} filters
 * @returns {string[]}
 */
function selectTests(files, filters) {
  if (filters.length === 0) {
    return files;
  }
  return files.filter((file) =>
    filters.some((filter) => file.includes(filter)),
  );
}

/**
 * @param {string} root
 * @param {string} directory
 * @param {string[]} output
 * @returns {Promise<void>}
 */
async function walkTestsDirectory(root, directory, output) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = join(directory, entry.name);
    if (entry.isDirectory()) {
      await walkTestsDirectory(root, absolutePath, output);
      continue;
    }

    if (!NODE_TEST_FILE_PATTERN.test(entry.name)) {
      continue;
    }
    output.push(relative(root, absolutePath));
  }
}
