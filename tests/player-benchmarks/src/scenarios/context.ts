import type { Adapter } from "../adapters.ts";
import type { Recorder } from "../observations.ts";
import type { MemorySnapshot } from "../memory.ts";

/** Shared observations and public commands, never player-specific readiness. */
export interface ScenarioContext {
  video: HTMLVideoElement;
  player: Adapter;
  url: string;
  initializeStartedAt: number;
  metrics: Record<string, number>;
  memorySnapshots: MemorySnapshot[];
  mark: Recorder;
  fail(error: unknown): void;
  healthy<T>(promise: Promise<T>): Promise<T>;
}
