import { readFile, readdir } from "node:fs/promises";

/** Children can be launched by any Chrome thread, not only its main thread. */
export async function readProcessTreePss(pid, fs = { readFile, readdir }) {
  const seen = new Set();
  async function visit(id) {
    if (seen.has(id)) return 0;
    seen.add(id);
    try {
      const [smaps, threads] = await Promise.all([
        fs.readFile(`/proc/${id}/smaps_rollup`, "utf8"),
        fs.readdir(`/proc/${id}/task`),
      ]);
      const match = /^Pss:\s+(\d+) kB$/m.exec(smaps);
      if (!match) throw new Error("Missing process PSS");
      const children = await Promise.all(
        threads.map(async (thread) => {
          try {
            return await fs.readFile(
              `/proc/${id}/task/${thread}/children`,
              "utf8",
            );
          } catch (error) {
            if (error.code === "ENOENT" || error.code === "ESRCH") return "";
            throw error;
          }
        }),
      );
      const descendants = await Promise.all(
        children
          .join(" ")
          .trim()
          .split(/\s+/)
          .filter(Boolean)
          .map(Number)
          .map(visit),
      );
      return (
        Number(match[1]) * 1024 +
        descendants.reduce((sum, bytes) => sum + bytes, 0)
      );
    } catch (error) {
      if (error.code === "ENOENT" || error.code === "ESRCH") return 0;
      throw error;
    }
  }
  return visit(pid);
}

/** Linux process-tree PSS: includes native/media allocations, not just JS. */
export function sampleProcessMemory(pid) {
  const samples = [];
  if (process.platform !== "linux") return { samples, stop: async () => {} };
  let reading = false;
  let pending = Promise.resolve();
  const timer = setInterval(() => {
    if (reading) return;
    reading = true;
    pending = readProcessTreePss(pid)
      .then(
        (bytes) => samples.push({ time: Date.now(), bytes }),
        (error) => samples.push({ time: Date.now(), error: String(error) }),
      )
      .finally(() => {
        reading = false;
      });
  }, 250);
  return {
    samples,
    async stop() {
      clearInterval(timer);
      await pending;
    },
  };
}
