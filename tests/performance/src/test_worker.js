import EmbeddedWorker from "wasp-hls/worker";
import { runTestWorkerBootstrap } from "../../utils/test_worker_bootstrap.js";

export function createPerformanceWorker(fetchRules = []) {
  const channelName = `wasp-hls-perf-${Math.random().toString(36).slice(2)}`;
  const channel = new BroadcastChannel(channelName);
  const events = [];
  const waiters = new Set();
  channel.onmessage = ({ data }) => {
    events.push(data);
    for (const waiter of waiters) {
      waiter(data);
    }
  };
  const config = {
    workerUrl: EmbeddedWorker,
    telemetryChannelName: channelName,
    fetchRules,
  };
  const blob = new Blob(
    [`(${runTestWorkerBootstrap.toString()})(${JSON.stringify(config)});\n`],
    { type: "application/javascript" },
  );
  const url = URL.createObjectURL(blob);

  return {
    url,
    waitFor(predicate, timeoutMs = 10_000) {
      const previous = events.find(predicate);
      if (previous !== undefined) {
        return Promise.resolve(previous);
      }
      return new Promise((resolve, reject) => {
        const timeoutId = setTimeout(() => {
          waiters.delete(onEvent);
          reject(new Error("Timed out waiting for worker telemetry"));
        }, timeoutMs);
        const onEvent = (event) => {
          if (predicate(event)) {
            clearTimeout(timeoutId);
            waiters.delete(onEvent);
            resolve(event);
          }
        };
        waiters.add(onEvent);
      });
    },
    waitForCount(predicate, count, timeoutMs = 10_000) {
      const matching = () => events.filter(predicate);
      if (matching().length >= count) {
        return Promise.resolve(matching());
      }
      return new Promise((resolve, reject) => {
        const timeoutId = setTimeout(() => {
          waiters.delete(onEvent);
          reject(new Error("Timed out waiting for worker telemetry count"));
        }, timeoutMs);
        const onEvent = () => {
          const matches = matching();
          if (matches.length >= count) {
            clearTimeout(timeoutId);
            waiters.delete(onEvent);
            resolve(matches);
          }
        };
        waiters.add(onEvent);
      });
    },
    dispose() {
      waiters.clear();
      channel.close();
      URL.revokeObjectURL(url);
    },
  };
}
