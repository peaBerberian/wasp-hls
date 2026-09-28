/**
 * Typecheck, lint, formatting, and Rust validation helpers for the repo.
 */

import { join } from "path";
import { exec, npmExecCommand } from "../utils/exec.mjs";
import { generateWasmAbi } from "./build.mjs";
import { reportStep } from "./report.mjs";

/** @param {string} root */
export async function checkAll(root) {
  await generateWasmAbi(root);
  await Promise.all([
    checkMain(root),
    checkWorker(root),
    checkCommon(root),
    checkDemo(root),
    checkScripts(root),
    checkTests(root),
    checkRust(root),
  ]);
}

/** @param {string} root */
export async function checkMain(root) {
  reportStep("CHECK", "typechecking ts-main...");
  const tsc = npmExecCommand("tsc", ["--project", "./src/ts-main", "--noEmit"]);
  await exec(tsc.command, tsc.args, {
    cwd: root,
  });
  reportStep("CHECK", "linting ts-main...");
  const eslint = npmExecCommand("eslint", ["."]);
  await exec(eslint.command, eslint.args, {
    cwd: join(root, "src", "ts-main"),
  });
}

/** @param {string} root */
export async function checkWorker(root) {
  reportStep("CHECK", "typechecking ts-worker...");
  const tsc = npmExecCommand("tsc", [
    "--project",
    "./src/ts-worker",
    "--noEmit",
  ]);
  await exec(tsc.command, tsc.args, {
    cwd: root,
  });
  reportStep("CHECK", "linting ts-worker...");
  const eslint = npmExecCommand("eslint", ["."]);
  await exec(eslint.command, eslint.args, {
    cwd: join(root, "src", "ts-worker"),
  });
  reportStep("CHECK", "linting ts-transmux...");
  await exec(eslint.command, eslint.args, {
    cwd: join(root, "src", "ts-transmux"),
  });
}

/** @param {string} root */
export async function checkCommon(root) {
  reportStep("CHECK", "linting ts-common...");
  const eslint = npmExecCommand("eslint", ["."]);
  await exec(eslint.command, eslint.args, {
    cwd: join(root, "src", "ts-common"),
  });
}

/** @param {string} root */
export async function checkDemo(root) {
  reportStep("CHECK", "typechecking demo...");
  const tsc = npmExecCommand("tsc", ["--project", "./demo", "--noEmit"]);
  await exec(tsc.command, tsc.args, {
    cwd: root,
  });
  reportStep("CHECK", "linting demo...");
  const eslint = npmExecCommand("eslint", ["src/**/*"]);
  await exec(eslint.command, eslint.args, {
    cwd: join(root, "demo"),
  });
}

/** @param {string} root */
export async function checkScripts(root) {
  reportStep("CHECK", "typechecking JSDoc-checked scripts...");
  const tsc = npmExecCommand("tsc", ["--project", "./scripts/tsconfig.json"]);
  await exec(tsc.command, tsc.args, {
    cwd: root,
  });
}

/** @param {string} root */
export async function checkTests(root) {
  reportStep("CHECK", "typechecking tests...");
  const tsc = npmExecCommand("tsc", ["--project", "./tests/tsconfig.json"]);
  await exec(tsc.command, tsc.args, { cwd: root });
}

/** @param {string} root */
export async function checkRust(root) {
  reportStep("CHECK", "run clippy on rust code...");
  await exec("cargo", ["clippy"], { cwd: root });
}
