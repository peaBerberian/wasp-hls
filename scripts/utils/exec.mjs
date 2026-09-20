/**
 * Process execution helpers shared by build/check/watch scripts.
 */

import { spawn } from "child_process";

/**
 * @param {string} commandName
 * @param {string[]} args
 * @param {{ cwd?: string, stdio?: "inherit" | "pipe" | "ignore", env?: NodeJS.ProcessEnv }} [options]
 */
export function exec(commandName, args, options = {}) {
  return /** @type {Promise<void>} */ (
    new Promise((resolve, reject) => {
      const child = spawn(commandName, args, {
        cwd: options.cwd,
        stdio: options.stdio ?? "inherit",
        env: options.env,
      });
      child.on("error", reject);
      child.on("exit", (code, signal) => {
        if (signal != null) {
          reject(new Error(`${commandName} exited with signal ${signal}.`));
        } else if (code !== 0) {
          reject(new Error(`${commandName} exited with code ${code}.`));
        } else {
          resolve();
        }
      });
    })
  );
}

/**
 * Build a command tuple for `npm exec -- <tool> ...args` without relying on
 * PATH shims. When the current process was launched by npm, reuse npm's own JS
 * entrypoint through the current runtime.
 *
 * @param {string} toolName
 * @param {string[]} args
 * @returns {{ command: string, args: string[] }}
 */
export function npmExecCommand(toolName, args = []) {
  if (process.env.npm_execpath) {
    return {
      command: process.execPath,
      args: [process.env.npm_execpath, "exec", "--", toolName, ...args],
    };
  }

  if (process.platform === "win32") {
    return {
      command: "cmd.exe",
      // `/d` disables AutoRun hooks, `/s` preserves quoting for the command
      // string passed to `cmd`, and `/c` runs it then exits.
      args: ["/d", "/s", "/c", "npm", "exec", "--", toolName, ...args],
    };
  }

  return {
    command: "npm",
    args: ["exec", "--", toolName, ...args],
  };
}
