import { MediaType } from "../ts-common/generatedWasmEnums.ts";
import logger, { LoggerLevel } from "../ts-common/logger.ts";

export interface HiddenTransmuxProfilingConfig {
  transmuxProfiling?: boolean;
  transmuxProfilingSampleSize?: number;
  transmuxProfilingSlowThreshold?: number;
}

interface TransmuxProfilingConfig {
  enabled: boolean;
  sampleSize: number;
  slowThreshold: number;
}

interface TransmuxProfilingTotals {
  count: number;
  totalDurationMs: number;
  maxDurationMs: number;
  totalInputBytes: number;
  totalOutputBytes: number;
}

const DEFAULT_CONFIG: TransmuxProfilingConfig = {
  enabled: false,
  sampleSize: 20,
  slowThreshold: 12,
};

const currentConfig: TransmuxProfilingConfig = { ...DEFAULT_CONFIG };
let totals = createEmptyTotals();

function createEmptyTotals(): TransmuxProfilingTotals {
  return {
    count: 0,
    totalDurationMs: 0,
    maxDurationMs: 0,
    totalInputBytes: 0,
    totalOutputBytes: 0,
  };
}

function clampPositiveInteger(value: number, fallback: number): number {
  if (!Number.isFinite(value) || value < 1) {
    return fallback;
  }
  return Math.floor(value);
}

function clampNonNegativeNumber(value: number, fallback: number): number {
  if (!Number.isFinite(value) || value < 0) {
    return fallback;
  }
  return value;
}

function formatBytesPerMs(bytes: number, durationMs: number): string {
  if (durationMs <= 0) {
    return "n/a";
  }
  return (bytes / durationMs).toFixed(1);
}

function mediaTypeToString(mediaType: MediaType): string {
  switch (mediaType) {
    case MediaType.Audio:
      return "audio";
    case MediaType.Video:
      return "video";
    default:
      return "unknown";
  }
}

export function updateTransmuxProfilingConfig(
  config: HiddenTransmuxProfilingConfig,
): void {
  const previousEnabled = currentConfig.enabled;
  if (config.transmuxProfiling !== undefined) {
    currentConfig.enabled = config.transmuxProfiling;
  }
  if (config.transmuxProfilingSampleSize !== undefined) {
    currentConfig.sampleSize = clampPositiveInteger(
      config.transmuxProfilingSampleSize,
      DEFAULT_CONFIG.sampleSize,
    );
  }
  if (config.transmuxProfilingSlowThreshold !== undefined) {
    currentConfig.slowThreshold = clampNonNegativeNumber(
      config.transmuxProfilingSlowThreshold,
      DEFAULT_CONFIG.slowThreshold,
    );
  }

  if (currentConfig.enabled !== previousEnabled || !currentConfig.enabled) {
    resetTransmuxProfiling();
  }

  if (
    currentConfig.enabled &&
    !previousEnabled &&
    logger.hasLevel(LoggerLevel.Info)
  ) {
    logger.info(
      "[transmux-profile] enabled sampleSize=",
      currentConfig.sampleSize,
      "slowThresholdMs=",
      currentConfig.slowThreshold,
    );
  }
}

export function resetTransmuxProfiling(): void {
  totals = createEmptyTotals();
}

export function recordTransmuxProfile(args: {
  durationMs: number;
  inputBytes: number;
  outputBytes: number;
  mediaType: MediaType;
}): void {
  if (!currentConfig.enabled) {
    return;
  }

  totals.count += 1;
  totals.totalDurationMs += args.durationMs;
  totals.maxDurationMs = Math.max(totals.maxDurationMs, args.durationMs);
  totals.totalInputBytes += args.inputBytes;
  totals.totalOutputBytes += args.outputBytes;

  if (
    args.durationMs >= currentConfig.slowThreshold &&
    logger.hasLevel(LoggerLevel.Info)
  ) {
    logger.info(
      "[transmux-profile] slow-segment mediaType=",
      mediaTypeToString(args.mediaType),
      "durationMs=",
      args.durationMs.toFixed(2),
      "inputBytes=",
      args.inputBytes,
      "outputBytes=",
      args.outputBytes,
    );
  }

  if (
    totals.count % currentConfig.sampleSize === 0 &&
    logger.hasLevel(LoggerLevel.Info)
  ) {
    logger.info(
      "[transmux-profile] summary segments=",
      totals.count,
      "avgMs=",
      (totals.totalDurationMs / totals.count).toFixed(2),
      "maxMs=",
      totals.maxDurationMs.toFixed(2),
      "avgInputBytes=",
      Math.round(totals.totalInputBytes / totals.count),
      "avgOutputBytes=",
      Math.round(totals.totalOutputBytes / totals.count),
      "avgInputBytesPerMs=",
      formatBytesPerMs(totals.totalInputBytes, totals.totalDurationMs),
      "avgOutputBytesPerMs=",
      formatBytesPerMs(totals.totalOutputBytes, totals.totalDurationMs),
    );
  }
}
