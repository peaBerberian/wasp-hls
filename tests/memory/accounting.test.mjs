import test from "node:test";
import assert from "node:assert/strict";
import { formatMeasurement } from "../utils/browser_memory.js";

test("page totals already include dedicated workers; telemetry is not added", () => {
  for (const worker of [
    { userAgentSpecificBytes: 800, wasmMemoryBytes: 600 },
    { jsHeapUsedBytes: 200, wasmMemoryBytes: 600 },
    {},
  ]) {
    const result = formatMeasurement({ bytes: 1800, breakdown: [] }, worker);
    assert.equal(result.totalMemoryBytes, 1800);
    assert.equal(result.workerWasmMemoryBytes, worker.wasmMemoryBytes ?? null);
  }
});
