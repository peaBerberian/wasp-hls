import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sampleProcessMemory } from "./process-memory.mjs";

/** One process/profile per sample; logs must never terminate a measurement. */
export async function runBrowser(chrome, url, result, { memory = false } = {}) {
  const profile = await mkdtemp(join(tmpdir(), "wasp-player-benchmark-"));
  let child;
  let timer;
  let monitor;
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
        ...(memory ? ["--js-flags=--expose-gc"] : []),
        ...(process.env.BENCH_CHROME_LOG ? ["--enable-logging=stderr"] : []),
        `--user-data-dir=${profile}`,
        url.href,
      ],
      { stdio: process.env.BENCH_CHROME_LOG ? "inherit" : "ignore" },
    );
    if (memory) monitor = sampleProcessMemory(child.pid);
    const exited = new Promise((_resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code, signal) =>
        reject(new Error(`Chrome exited before result: ${code ?? signal}`)),
      );
    });
    const timeout = new Promise((_resolve, reject) => {
      timer = setTimeout(
        () => reject(new Error("Page timed out")),
        memory ? 300000 : 45000,
      );
    });
    const message = await Promise.race([result, exited, timeout]);
    if (monitor) {
      await monitor.stop();
      message.processMemory = {
        metric: "process-tree-pss",
        available: process.platform === "linux",
        intervalMs: 250,
        samples: monitor.samples,
      };
    }
    return message;
  } finally {
    clearTimeout(timer);
    await monitor?.stop();
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
