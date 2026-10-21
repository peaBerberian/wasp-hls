import type { Adapter } from "../adapters.ts";
import type { Recorder } from "../observations.ts";

/** Shared observations and public commands, never player-specific readiness. */
export interface ScenarioContext {
  video: HTMLVideoElement;
  player: Adapter;
  url: string;
  initializeStartedAt: number;
  metrics: Record<string, number>;
  mark: Recorder;
  fail(error: unknown): void;
  healthy<T>(promise: Promise<T>): Promise<T>;
}
