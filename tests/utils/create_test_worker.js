import { runTestWorkerBootstrap } from "./test_worker_bootstrap.js";
import EmbeddedWorker from "../../build/embedded/worker.js";

/**
 * @typedef {Object} TestWorkerFetchRule
 * @property {string} [id]
 * @property {{
 *   urlIncludes?: string;
 *   urlEndsWith?: string;
 *   urlMatches?: string;
 *   hasRange?: boolean;
 * }} [match]
 * @property {Array<TestWorkerFetchAction>} [actions]
 */

/**
 * @typedef {Object} TestWorkerFetchAction
 * @property {"passthrough"|"error"|"timeout"|"response"} type
 * @property {number} [delayMs]
 * @property {boolean} [waitForRelease] Hold the response and request timeout until releaseFetch.
 * @property {number} [status]
 * @property {string} [body]
 * @property {Record<string, string>} [headers]
 * @property {string} [message]
 */

/**
 * @typedef {Object} TestWorkerHandle
 * @property {string} url
 * @property {() => void} dispose
 * @property {WorkerTelemetryCollector|null} telemetry
 */

/**
 * @typedef {Object} WorkerTelemetryCollector
 * @property {() => Array<unknown>} getEvents
 * @property {(predicate: (event: unknown) => boolean, timeoutMs?: number) => Promise<unknown>} waitFor
 * @property {(predicate: (event: unknown) => boolean, count: number, timeoutMs?: number) => Promise<Array<unknown>>} waitForCount
 * @property {(timeoutMs?: number) => Promise<unknown>} requestMemorySnapshot
 * @property {(requestId: number) => void} releaseFetch
 * @property {() => void} close
 */

/**
 * Create a test-only Worker URL which patches `fetch` before loading the real
 * production Worker bundle.
 *
 * @param {{
 *   fetchRules?: Array<TestWorkerFetchRule>;
 *   collectTelemetry?: boolean;
 *   storeTelemetryHistory?: boolean;
 * }} [options]
 * @returns {TestWorkerHandle}
 */
export function createTestWorker({
  fetchRules = [],
  collectTelemetry = true,
  storeTelemetryHistory = true,
} = {}) {
  const telemetryChannelName = collectTelemetry
    ? `wasp-hls-test-worker-${Math.random().toString(36).slice(2)}`
    : null;
  const config = {
    workerUrl: EmbeddedWorker,
    telemetryChannelName,
    fetchRules,
  };
  const blob = new Blob(
    [`(${runTestWorkerBootstrap.toString()})(${JSON.stringify(config)});\n`],
    { type: "application/javascript" },
  );
  const url = URL.createObjectURL(blob);
  const telemetry =
    telemetryChannelName === null
      ? null
      : createTelemetryCollector(telemetryChannelName, {
          storeHistory: storeTelemetryHistory,
        });
  return {
    url,
    telemetry,
    dispose() {
      telemetry?.close();
      URL.revokeObjectURL(url);
    },
  };
}

/**
 * @param {string} channelName
 * @param {{ storeHistory?: boolean }} [options]
 * @returns {WorkerTelemetryCollector}
 */
function createTelemetryCollector(channelName, { storeHistory = true } = {}) {
  const channel = new BroadcastChannel(channelName);
  /** @type {Array<unknown>} */
  const events = [];
  /** @type {Set<(event: unknown) => void>} */
  const listeners = new Set();
  let requestId = 0;
  channel.onmessage = (evt) => {
    if (storeHistory) {
      events.push(evt.data);
    }
    for (const listener of listeners) {
      listener(evt.data);
    }
  };
  return {
    getEvents() {
      return events.slice();
    },
    releaseFetch(requestId) {
      channel.postMessage({ type: "release-fetch", requestId });
    },
    waitFor(predicate, timeoutMs = 20_000) {
      return new Promise((resolve, reject) => {
        const existingEvent = events.find(predicate);
        if (existingEvent !== undefined) {
          resolve(existingEvent);
          return;
        }
        const timeoutId = setTimeout(() => {
          listeners.delete(onEvent);
          cleanup();
          reject(new Error("Timed out waiting for worker telemetry"));
        }, timeoutMs);
        const onEvent = (event) => {
          if (predicate(event)) {
            listeners.delete(onEvent);
            cleanup();
            resolve(event);
          }
        };
        listeners.add(onEvent);
        function cleanup() {
          clearTimeout(timeoutId);
        }
      });
    },
    waitForCount(predicate, count, timeoutMs = 10_000) {
      return new Promise((resolve, reject) => {
        const getMatchingEvents = () => events.filter(predicate);
        const existingMatches = getMatchingEvents();
        if (existingMatches.length >= count) {
          resolve(existingMatches);
          return;
        }
        const timeoutId = setTimeout(() => {
          listeners.delete(onEvent);
          cleanup();
          reject(new Error("Timed out waiting for worker telemetry count"));
        }, timeoutMs);
        const onEvent = () => {
          const matches = getMatchingEvents();
          if (matches.length >= count) {
            listeners.delete(onEvent);
            cleanup();
            resolve(matches);
          }
        };
        listeners.add(onEvent);
        function cleanup() {
          clearTimeout(timeoutId);
        }
      });
    },
    requestMemorySnapshot(timeoutMs = 10_000) {
      const currentRequestId = ++requestId;
      const memorySnapshotPromise = this.waitFor(
        (event) =>
          event?.type === "memory-snapshot" &&
          event.requestId === currentRequestId,
        timeoutMs,
      );
      channel.postMessage({
        type: "memory-snapshot-request",
        requestId: currentRequestId,
      });
      return memorySnapshotPromise;
    },
    close() {
      channel.close();
    },
  };
}
