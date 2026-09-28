#!/usr/bin/env node

import * as fs from "fs";
import * as path from "path";
import { execFileSync } from "child_process";
import { pathToFileURL } from "url";
import {
  getPerformanceTestFiles,
  initializePerformanceTestsPages,
  rootDirectory,
} from "./prepare.mjs";
import { runPerformanceTests } from "./runner.mjs";
import { compareRuns } from "./statistics.mjs";
import {
  formatHtmlReport,
  logComparison,
  logRetryComparison,
} from "./report.mjs";

import { ensureDefaultVodFixtures } from "../contents/vod_fixtures.mjs";

const DEFAULT_CONTENT_SERVER_PORT = 3000;
const DEFAULT_TEST_PAGE_PORT = 8080;
const DEFAULT_RESULT_SERVER_PORT = 6789;

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main()
    .then((code) => process.exit(code))
    .catch((error) => {
      console.error("Error:", error);
      process.exit(1);
    });
}

async function main() {
  const args = process.argv.slice(2);

  let resultServerPort = DEFAULT_RESULT_SERVER_PORT;
  let contentServerPort = DEFAULT_CONTENT_SERVER_PORT;
  let testPagePort = DEFAULT_TEST_PAGE_PORT;

  /** @type {string | undefined} */
  let browser;
  /** @type {string | undefined} */
  let branchName;
  /** @type {string | undefined} */
  let remote;
  /** @type {string | undefined} */
  let reportFile;
  /** @type {string[]} */
  const filters = [];
  /**
   * @param {string|undefined} input
   * @param {string} flagName
   * @returns {number}
   */
  const parsePort = (input, flagName) => {
    if (input === undefined || input.startsWith("--")) {
      /* eslint-disable-next-line no-console */
      console.error(`ERROR: no port provided to ${flagName} flag\n`);
      displayHelp();
      process.exit(1);
    }
    const port = +input;
    if (isNaN(port)) {
      /* eslint-disable-next-line no-console */
      console.error(
        `ERROR: Invalid port configured for flag ${flagName}. Should be a number, received "` +
          input +
          '"\n',
      );
      displayHelp();
      process.exit(1);
    }
    return port;
  };
  for (let argOffset = 0; argOffset < args.length; argOffset++) {
    const currentArg = args[argOffset];
    switch (currentArg) {
      case "-h":
      case "--help":
        displayHelp();
        process.exit(0);
        break;

      case "--result-port":
        argOffset++;
        resultServerPort = parsePort(args[argOffset], currentArg);
        break;

      case "--page-port":
        argOffset++;
        testPagePort = parsePort(args[argOffset], currentArg);
        break;

      case "--content-port":
        argOffset++;
        contentServerPort = parsePort(args[argOffset], currentArg);
        break;

      case "--branch":
        argOffset++;
        branchName = args[argOffset];
        if (branchName === undefined || branchName.startsWith("--")) {
          // eslint-disable-next-line no-console
          console.error("ERROR: no branch name provided\n");
          displayHelp();
          process.exit(1);
        }
        break;

      case "--remote-git-url":
        argOffset++;
        remote = args[argOffset];
        if (remote === undefined || remote.startsWith("--")) {
          // eslint-disable-next-line no-console
          console.error("ERROR: no remote URL provided\n");
          displayHelp();
          process.exit(1);
        }
        break;

      case "--report":
        {
          argOffset++;
          reportFile = args[argOffset];
          if (reportFile === undefined || reportFile.startsWith("--")) {
            // eslint-disable-next-line no-console
            console.error("ERROR: no file path provided\n");
            displayHelp();
            process.exit(1);
          }
        }
        break;

      case "--browser":
        argOffset++;
        if (!["chrome", "firefox"].includes(args[argOffset])) {
          /* eslint-disable-next-line no-console */
          console.error(
            `ERROR: Invalid browser configured: should be either "chrome" or "firefox", received: ` +
              args[argOffset] +
              '"\n',
          );
          displayHelp();
          process.exit(1);
        }
        browser = args[argOffset];
        break;

      case "--filter":
        if (
          args[argOffset + 1] == null ||
          args[argOffset + 1].startsWith("--")
        ) {
          console.error("Missing value for --filter");
          process.exit(1);
        }
        filters.push(args[++argOffset]);
        break;

      default:
        // eslint-disable-next-line no-console
        console.error("ERROR: Unrecognized flag:", currentArg);
        displayHelp();
        process.exit(1);
    }
  }

  const controlIterations = Number(
    process.env.WASP_HLS_PERF_CONTROL_ITERATIONS ?? 40,
  );
  const treatmentIterations = Number(
    process.env.WASP_HLS_PERF_TREATMENT_ITERATIONS ?? 40,
  );
  const innerIterations = Number(
    process.env.WASP_HLS_PERF_INNER_ITERATIONS ?? 4,
  );
  let performanceTestFiles;
  try {
    if (
      !Number.isInteger(innerIterations) ||
      innerIterations < 2 ||
      !Number.isInteger(controlIterations) ||
      controlIterations < 1 ||
      !Number.isInteger(treatmentIterations) ||
      treatmentIterations < 1
    ) {
      throw new Error(
        "Performance tests need positive integer process counts and at least two inner iterations.",
      );
    }
    performanceTestFiles = getPerformanceTestFiles([]).filter(
      (file) =>
        filters.length === 0 ||
        filters.some((filter) =>
          path.relative(rootDirectory, file).includes(filter),
        ),
    );
    if (performanceTestFiles.length === 0)
      throw new Error("No performance suites matched the requested filters.");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error("ERROR:", err instanceof Error ? err.message : String(err));
    process.exit(1);
  }

  await initializePerformanceTestsPages({
    branchName: branchName ?? "main",
    remoteGitUrl: remote,
    contentServerPort,
    performanceTestFiles,
    innerIterations,
  });
  // Generate all scenario assets before starting performance measurements.
  await ensureDefaultVodFixtures();
  const { firstRun, secondRun, success } = await runComparison({
    browser,
    contentServerPort,
    resultServerPort,
    testPagePort,
    controlIterations,
    treatmentIterations,
  });
  await writeReport(reportFile, {
    success,
    baseBranch: branchName,
    firstRun,
    secondRun,
  });
  return success ? 0 : 1;
}

/** @param {Parameters<typeof runPerformanceTests>[0]} options */
async function runComparison(options) {
  const firstRun = await runPerformanceTests(options);
  logComparison(firstRun);
  /** @type {import("./statistics.mjs").Comparison | null} */
  let secondRun = null;
  if (firstRun.worse.length > 0) {
    console.warn("\nRetrying one time just to check if unlucky...");
    secondRun = await runPerformanceTests(options);
    logRetryComparison(firstRun, secondRun);
  }
  return {
    firstRun,
    secondRun,
    success: compareRuns(firstRun, secondRun).success,
  };
}

/** @param {string | undefined} reportFile
 * @param {Parameters<typeof formatHtmlReport>[0]} report */
async function writeReport(reportFile, report) {
  if (reportFile === undefined) return;
  try {
    const commitSha = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: rootDirectory,
      encoding: "utf8",
    }).trim();
    fs.writeFileSync(reportFile, formatHtmlReport({ ...report, commitSha }));
  } catch (error) {
    console.error(`WARNING: Cannot write report: ${String(error)}`);
  }
}

/**
 * Display through `console.log` an helping message relative to how to run this
 * script.
 */
function displayHelp() {
  /* eslint-disable-next-line no-console */
  console.log(
    `Usage: npm run test -- performance [options]
Available options:
  --filter <value>                  Select suites by path substring (repeatable).
  -h, --help                        Display this help message
  --branch <branch>                 Specify the branch name the performance results should be compared to.
                                    Defaults to the "main" branch.,
  --remote-git-url <URL>            Specify the remote git URL where the current repository can be cloned from.
                                    Defaults to the current remote URL.
  --browser <BROWSER>               The browser to run the tests on. Can be "chrome" or "firefox".
                                    "chrome" by default.
  --result-port <NUMBER>            Configure the port used to send/receive test results.
                                    ${DEFAULT_RESULT_SERVER_PORT} by default.
  --page-port <NUMBER>              Configure the port used to serve the test page.
                                    ${DEFAULT_TEST_PAGE_PORT} by default.
  --content-port <NUMBER>           Configure the port used to serve test contents.
                                    ${DEFAULT_CONTENT_SERVER_PORT} by default.
  --report <path>                   Optional path to HTML file where a report will be written in once done.

All suites run when no filter is provided. Watch mode is unsupported.`,
  );
}
