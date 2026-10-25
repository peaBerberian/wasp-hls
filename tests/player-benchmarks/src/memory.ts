/** Application memory includes dedicated workers; never add their totals again. */
export interface MemorySnapshot {
  phase: string;
  time: number;
  bytes: number;
  breakdown: unknown[];
  buffered: number[][];
  currentTime: number;
}

export async function measureMemory(
  phase: string,
  video: HTMLVideoElement,
): Promise<MemorySnapshot> {
  const api = performance as Performance & {
    measureUserAgentSpecificMemory?: () => Promise<{
      bytes: number;
      breakdown: unknown[];
    }>;
  };
  const gc = (globalThis as typeof globalThis & { gc?: () => void }).gc;
  if (!crossOriginIsolated || !api.measureUserAgentSpecificMemory || !gc)
    throw new Error(
      "Memory scenarios require isolation, the memory API and exposed GC",
    );
  gc();
  const measurement = await api.measureUserAgentSpecificMemory();
  return {
    phase,
    time: performance.timeOrigin + performance.now(),
    bytes: measurement.bytes,
    breakdown: measurement.breakdown,
    currentTime: video.currentTime,
    buffered: Array.from({ length: video.buffered.length }, (_, i) => [
      video.buffered.start(i),
      video.buffered.end(i),
    ]),
  };
}

/** Compare the same retained media, allowing only audio/frame timestamp rounding. */
export function checkMemoryBuffer(snapshot: MemorySnapshot) {
  const { buffered, currentTime } = snapshot;
  if (buffered.length !== 1)
    throw new Error("Memory comparison requires one buffered range");
  const [[start, end]] = buffered;
  if (
    Math.abs(start) > 0.15 ||
    Math.abs(end - 20) > 0.15 ||
    currentTime < 16 ||
    currentTime > 17
  )
    throw new Error(`Unmatched memory buffer: ${JSON.stringify(snapshot)}`);
}
