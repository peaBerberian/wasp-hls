/**
 * ============= tasks/index =============
 *
 * Main task runner entrypoint used by npm scripts to build, check, clean, and
 * serve project artifacts.
 */

import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";
import {
  buildAll,
  buildDocs,
  buildDemoBundle,
  buildMain,
  buildWasm,
  buildWorker,
  generateWasmAbi,
} from "./build.mjs";
import {
  checkAll,
  checkCommon,
  checkDemo,
  checkMain,
  checkRust,
  checkScripts,
  checkTests,
  checkWorker,
} from "./check.mjs";
import { exec } from "../utils/exec.mjs";
import { cleanBuildDirectory } from "../utils/fs.mjs";
import launchStaticServer from "../launch_static_server.mjs";
import {
  testAll,
  testIntegration,
  testMemory,
  testPerformance,
  testRust as runRustTests,
  testTransmux,
} from "./test.mjs";
import { watchDemo } from "./watch.mjs";
import { reportSuccess } from "./report.mjs";
import { npmExecCommand } from "../utils/exec.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rawArgs = process.argv.slice(2);
const normalizedArgs =
  rawArgs.length === 1 && (rawArgs[0] === "--help" || rawArgs[0] === "-h")
    ? ["help"]
    : rawArgs;
const [command = "help", ...rest] = normalizedArgs;

run().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});

async function run() {
  switch (command) {
    case "build": {
      const { flags, scope } = parseArgs(rest, ["--release", "--watch"]);
      switch (scope ?? "all") {
        case "all":
          assertNoFlag(
            flags,
            "--watch",
            '"--watch" is only supported for "build demo".',
          );
          await buildAll(ROOT, {
            release: flags.has("--release"),
          });
          reportSuccess("Builds");
          return;
        case "demo":
          if (flags.has("--watch")) {
            await watchDemo(ROOT, { release: flags.has("--release") });
          } else {
            await buildDemoFull(ROOT, { release: flags.has("--release") });
          }
          reportSuccess("Demo build");
          return;
        case "wasm":
          assertNoFlag(
            flags,
            "--watch",
            '"--watch" is only supported for "build demo".',
          );
          await buildWasm(ROOT, { release: flags.has("--release") });
          reportSuccess("WASM build");
          return;
        case "worker":
          assertNoFlag(
            flags,
            "--watch",
            '"--watch" is only supported for "build demo".',
          );
          await buildWorker(ROOT, { release: flags.has("--release") });
          reportSuccess("ts-worker build");
          return;
        case "main":
          assertNoFlag(
            flags,
            "--watch",
            '"--watch" is only supported for "build demo".',
          );
          await buildMain(ROOT, { release: flags.has("--release") });
          reportSuccess("ts-main build");
          return;
        case "docs":
          assertNoFlags(flags, 'Flags are not supported for "build docs".');
          await buildDocs(ROOT);
          reportSuccess("documentation build");
          return;
        default:
          throw new Error(`Unknown build scope "${scope}".\n\n${helpText()}`);
      }
    }
    case "check": {
      const { scope } = parseArgs(rest, []);
      switch (scope ?? "all") {
        case "all":
          await checkAll(ROOT);
          reportSuccess("All checks");
          return;
        case "main":
          await generateWasmAbi(ROOT);
          await checkMain(ROOT);
          reportSuccess("ts-main checks");
          return;
        case "worker":
          await generateWasmAbi(ROOT);
          await checkWorker(ROOT);
          reportSuccess("ts-worker checks");
          return;
        case "common":
          await checkCommon(ROOT);
          reportSuccess("ts-common checks");
          return;
        case "demo":
          await checkDemo(ROOT);
          reportSuccess("demo checks");
          return;
        case "scripts":
          await checkScripts(ROOT);
          reportSuccess("scripts checks");
          return;
        case "tests":
          await checkTests(ROOT);
          reportSuccess("tests checks");
          return;
        case "rust":
          await checkRust(ROOT);
          reportSuccess("Rust checks");
          return;
        default:
          throw new Error(`Unknown check scope "${scope}".\n\n${helpText()}`);
      }
    }
    case "fmt": {
      const { flags } = parseArgs(rest, ["--check"]);
      if (flags.has("--check")) {
        await exec("cargo", ["fmt", "--check"], { cwd: ROOT });
        const prettier = npmExecCommand("prettier", [".", "--check"]);
        await exec(prettier.command, prettier.args, {
          cwd: ROOT,
        });
        reportSuccess("Code formatting checks");
      } else {
        await exec("cargo", ["fmt"], { cwd: ROOT });
        const prettier = npmExecCommand("prettier", [
          "--write",
          ".",
          "--loglevel",
          "warn",
        ]);
        await exec(prettier.command, prettier.args, {
          cwd: ROOT,
        });
        reportSuccess("Code formatting");
      }
      return;
    }
    case "test": {
      if (rest[0] === "performance") {
        await testPerformance(ROOT, rest.slice(1));
        reportSuccess("Performance tests");
        return;
      }
      const options = parseTestArgs(rest);
      switch (options.scope ?? "all") {
        case "all":
          ensureNoTestOptions(options, "all");
          await testAll(ROOT);
          reportSuccess("All tests");
          return;
        case "integration":
          await testIntegration(ROOT, {
            browser: options.browser,
            filters: options.filters,
            watch: options.watch,
          });
          reportSuccess("Integration tests");
          return;
        case "memory":
          if (options.browser != null && options.browser !== "chrome") {
            throw new Error(
              "Memory tests are only supported with `--browser chrome`.",
            );
          }
          assertNoWatchOption(options, "memory");
          await testMemory(ROOT, {
            browser: options.browser,
            filters: options.filters,
            watch: false,
          });
          reportSuccess("Memory tests");
          return;
        case "transmux":
          assertNoBrowserOption(options, "transmux");
          await testTransmux(ROOT, {
            filters: options.filters,
            watch: options.watch,
          });
          reportSuccess("Transmux tests");
          return;
        case "rust":
          assertNoBrowserOption(options, "rust");
          assertNoWatchOption(options, "rust");
          await runRustTests(ROOT, { filters: options.filters });
          reportSuccess("Rust unit tests");
          return;
        default:
          throw new Error(
            `Unknown test scope "${options.scope}".\n\n${helpText()}`,
          );
      }
    }
    case "generate":
      ensureNoArgs(rest);
      await generateWasmAbi(ROOT);
      reportSuccess("WASM ABI code generation");
      return;
    case "clean":
      ensureNoArgs(rest);
      cleanBuildDirectory(join(ROOT, "build"), { preserveDemoBundle: false });
      reportSuccess("Build directory cleanup");
      return;
    case "serve":
      ensureNoArgs(rest);
      await startStaticBuildServer(ROOT);
      return;
    case "start":
      ensureNoArgs(rest);
      {
        const server = await startStaticBuildServer(ROOT);
        try {
          await watchDemo(ROOT, { release: false });
        } finally {
          server.close();
        }
      }
      return;
    case "help":
      ensureNoArgs(rest);
      console.log(helpText());
      return;
    default:
      throw new Error(`Unknown command "${command}".\n\n${helpText()}`);
  }
}

/**
 * @param {string} root
 * @param {{ release: boolean }} options
 */
async function buildDemoFull(root, { release }) {
  await generateWasmAbi(root);
  cleanBuildDirectory(join(root, "build"), { preserveDemoBundle: true });
  await buildWasm(root, { release, skipGenerate: true });
  await buildWorker(root, { release });
  await buildDemoBundle(root, { release });
}

/**
 * @param {string} root
 */
async function startStaticBuildServer(root) {
  const server = launchStaticServer(join(ROOT, "build"), {
    verbose: true,
    noCache: command === "start",
    httpPort: 8000,
    httpsPort: 8443,
    certificatePath: join(root, "localhost.crt"),
    keyPath: join(root, "localhost.key"),
  });

  try {
    await server.listeningPromise;
  } catch (error) {
    server.close();
    throw error;
  }

  return server;
}

/**
 * @param {string[]} args
 * @param {string[]} allowedFlags
 * @returns {{ flags: Set<string>; scope: string | undefined }}
 */
function parseArgs(args, allowedFlags) {
  const allowed = new Set(allowedFlags);
  const flags = new Set();
  const positionals = [];
  for (const arg of args) {
    if (arg.startsWith("--")) {
      if (!allowed.has(arg)) {
        throw new Error(`Unsupported flag "${arg}".\n\n${helpText()}`);
      }
      flags.add(arg);
    } else {
      positionals.push(arg);
    }
  }
  if (positionals.length > 1) {
    throw new Error(`Too many arguments.\n\n${helpText()}`);
  }
  return { flags, scope: positionals[0] };
}

/**
 * @param {string[]} args
 */
function ensureNoArgs(args) {
  if (args.length !== 0) {
    throw new Error(`Unexpected arguments.\n\n${helpText()}`);
  }
}

/**
 * @param {string[]} args
 * @returns {{ browser: string | undefined; filters: string[]; scope: string | undefined; watch: boolean }}
 */
function parseTestArgs(args) {
  let scope;
  let watch = false;
  let browser;
  const filters = [];

  for (let index = 0; index < args.length; index++) {
    const arg = args[index];
    switch (arg) {
      case "--watch":
        watch = true;
        break;
      case "--browser":
        index++;
        browser = args[index];
        if (browser == null || browser.startsWith("--")) {
          throw new Error(`Missing value for "${arg}".\n\n${helpText()}`);
        }
        break;
      case "--filter":
        index++;
        {
          const filter = args[index];
          if (filter == null || filter.startsWith("--")) {
            throw new Error(`Missing value for "${arg}".\n\n${helpText()}`);
          }
          filters.push(filter);
        }
        break;
      default:
        if (arg.startsWith("--")) {
          throw new Error(`Unsupported flag "${arg}".\n\n${helpText()}`);
        }
        if (scope != null) {
          throw new Error(`Too many test arguments.\n\n${helpText()}`);
        }
        scope = arg;
        break;
    }
  }

  return {
    browser,
    filters,
    scope,
    watch,
  };
}

/**
 * @param {Set<string>} flags
 * @param {string} flag
 * @param {string} message
 */
function assertNoFlag(flags, flag, message) {
  if (flags.has(flag)) throw new Error(message);
}

/**
 * @param {Set<string>} flags
 * @param {string} message
 */
function assertNoFlags(flags, message) {
  if (flags.size !== 0) throw new Error(message);
}

/**
 * @param {{ watch: boolean; browser: string | undefined; filters: string[] }} options
 * @param {string} scope
 */
function ensureNoTestOptions(options, scope) {
  if (options.watch || options.browser != null || options.filters.length > 0) {
    throw new Error(
      `Extra options are not supported for "test ${scope}". Select a specific scope instead.\n\n${helpText()}`,
    );
  }
}

/**
 * @param {{ browser: string | undefined }} options
 * @param {string} scope
 */
function assertNoBrowserOption(options, scope) {
  if (options.browser != null) {
    throw new Error(
      `"--browser" is only supported for "test integration", not "test ${scope}".`,
    );
  }
}

/**
 * @param {{ watch: boolean }} options
 * @param {string} scope
 */
function assertNoWatchOption(options, scope) {
  if (options.watch) {
    throw new Error(`"--watch" is not supported for "test ${scope}".`);
  }
}

function helpText() {
  return `Usage: node scripts/tasks/index.mjs <command> [scope] [--flags]

Commands
  build [scope] [--release] [--watch]
    all         Build every targets of the player (doesn't include doc and dmeo) (default)
    demo        Build the demo and its runtime dependencies
    wasm        Build only the wasm target
    worker      Build only the worker bundle
    main        Build only the main-thread bundle
    docs        Build the documentation site

  check [scope]
    all         Typecheck and lint the whole TypeScript and Rust codebase (default)
    main        Check main-thread TypeScript
    worker      Check worker TypeScript
    common      Check common TypeScript
    demo        Check demo TypeScript
    scripts     Check JSDoc-checked Node scripts with TypeScript
    tests       Check tests with TypeScript
    rust        Run cargo clippy

  test [scope] [--filter <value>] [--watch] [--browser <name>]
    all         Run every test suite: Rust, transmux, and integration (default)
    rust        Run Rust unit tests
    transmux    Run Node-based transmux tests
    integration Run browser integration tests
    memory      Run browser memory-leak tests on Chrome
    performance Compare browser performance (excluded from the default run)
               --branch, --report, --remote-git-url and port options are performance-only
               --filter is repeatable for scoped test runs
               --watch is supported for integration and transmux
               --browser is supported for integration, memory and performance

  fmt [--check]   Format Rust and JS/TS/Markdown
  generate        Regenerate wasm ABI enums
  clean           Remove build artifacts
  serve           Serve the existing build directory
  start           Serve build/ and watch-rebuild the demo
  help            Print this message`;
}
