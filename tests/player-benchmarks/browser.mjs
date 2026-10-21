import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** One process/profile per sample; logs must never terminate a measurement. */
export async function runBrowser(chrome, url, result) {
  const profile = await mkdtemp(join(tmpdir(), "wasp-player-benchmark-"));
  let child;
  let timer;
  try {
    child = spawn(
      chrome,
      [
        "--headless",
        "--no-first-run",
        "--no-default-browser-check",
        "--no-sandbox",
        "--disable-dev-shm-usage",
        "--autoplay-policy=no-user-gesture-required",
        ...(process.env.BENCH_CHROME_LOG ? ["--enable-logging=stderr"] : []),
        `--user-data-dir=${profile}`,
        url.href,
      ],
      { stdio: process.env.BENCH_CHROME_LOG ? "inherit" : "ignore" },
    );
    const exited = new Promise((_resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code, signal) =>
        reject(new Error(`Chrome exited before result: ${code ?? signal}`)),
      );
    });
    const timeout = new Promise((_resolve, reject) => {
      timer = setTimeout(
        () => reject(new Error("Page timed out after 45 seconds")),
        45000,
      );
    });
    return await Promise.race([result, exited, timeout]);
  } finally {
    clearTimeout(timer);
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, "exit");
      child.kill("SIGKILL");
      await exited;
    }
    await rm(profile, {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 100,
    });
  }
}
