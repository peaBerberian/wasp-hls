/**
 * Worker bootstrap used by integration tests.
 *
 * This function is stringified and executed in a Worker blob. Keep it
 * self-contained:
 * - do not import anything here
 * - do not rely on variables from the surrounding module scope
 * - pass every value needed through the `config` argument
 *
 * Those constraints are the tradeoff for using a bundler-independent bootstrap
 * mechanism that is easy to understand: this function body is copied as source
 * code, then run in the Worker realm.
 *
 * @param {{
 *   workerUrl: string;
 *   telemetryChannelName: string | null;
 *   fetchRules: Array<{
 *     id?: string;
 *     match?: {
 *       urlIncludes?: string;
 *       urlEndsWith?: string;
 *       urlMatches?: string;
 *       hasRange?: boolean;
 *     };
 *     actions?: Array<{
 *       type: "passthrough" | "error" | "timeout" | "response";
 *       delayMs?: number;
 *       waitForRelease?: boolean;
 *       status?: number;
 *       body?: string;
 *       headers?: Record<string, string>;
 *       message?: string;
 *     }>;
 *   }>;
 * }} config
 */
export function runTestWorkerBootstrap(config) {
  const originalFetch = self.fetch.bind(self);
  const originalInstantiate = WebAssembly.instantiate.bind(WebAssembly);
  const originalInstantiateStreaming =
    typeof WebAssembly.instantiateStreaming === "function"
      ? WebAssembly.instantiateStreaming.bind(WebAssembly)
      : null;
  const channel =
    config.telemetryChannelName === null ||
    typeof BroadcastChannel !== "function"
      ? null
      : new BroadcastChannel(config.telemetryChannelName);
  const ruleHitCounts = new Array(config.fetchRules.length).fill(0);
  const originalSetTimeout = self.setTimeout.bind(self);
  const originalClearTimeout = self.clearTimeout.bind(self);
  /** @typedef {{ id: number; nativeId: number | null; duration: number | undefined; run: () => void }} FetchTimer */
  /** @type {Map<number, FetchTimer>} */
  const requestTimers = new Map();
  /** @type {Map<number, () => void>} */
  const fetchReleases = new Map();
  const patchedImports = new WeakSet();
  /** @type {FetchTimer | null} */
  let currentFetchTimer = null;
  let fetchRequestId = 0;
  let latestWasmMemory = null;

  function postTelemetry(event) {
    if (channel === null) {
      return;
    }
    try {
      channel.postMessage({
        ...event,
        timestampMs:
          typeof performance?.now === "function"
            ? performance.now()
            : Date.now(),
      });
    } catch (_) {
      // Best effort telemetry only.
    }
  }

  function createAbortError() {
    try {
      return new DOMException("Aborted", "AbortError");
    } catch (_) {
      const error = new Error("Aborted");
      error.name = "AbortError";
      return error;
    }
  }

  function getRangeHeader(headers) {
    if (headers == null) {
      return undefined;
    }
    if (typeof headers.get === "function") {
      return headers.get("Range") ?? headers.get("range") ?? undefined;
    }
    if (Array.isArray(headers)) {
      for (const [key, value] of headers) {
        if (String(key).toLowerCase() === "range") {
          return String(value);
        }
      }
      return undefined;
    }
    for (const key of Object.keys(headers)) {
      if (key.toLowerCase() === "range") {
        return String(headers[key]);
      }
    }
    return undefined;
  }

  function matchesRule(rule, url, init) {
    const match = rule.match ?? {};
    if (match.urlIncludes !== undefined && !url.includes(match.urlIncludes)) {
      return false;
    }
    if (match.urlEndsWith !== undefined && !url.endsWith(match.urlEndsWith)) {
      return false;
    }
    if (
      match.urlMatches !== undefined &&
      !new RegExp(match.urlMatches).test(url)
    ) {
      return false;
    }
    if (match.hasRange !== undefined) {
      const hasRange = getRangeHeader(init?.headers) !== undefined;
      if (hasRange !== match.hasRange) {
        return false;
      }
    }
    return true;
  }

  function pickAction(rule, ruleIndex) {
    const actions =
      Array.isArray(rule.actions) && rule.actions.length > 0
        ? rule.actions
        : [{ type: "passthrough" }];
    const hitCount = ruleHitCounts[ruleIndex];
    ruleHitCounts[ruleIndex] += 1;
    return {
      action: actions[Math.min(hitCount, actions.length - 1)],
      attempt: hitCount + 1,
    };
  }

  function delayWithAbort(delayMs, signal) {
    if (delayMs === undefined || delayMs <= 0) {
      return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        cleanup();
        resolve();
      }, delayMs);
      const onAbort = () => {
        cleanup();
        reject(createAbortError());
      };
      const cleanup = () => {
        clearTimeout(timeoutId);
        signal?.removeEventListener?.("abort", onAbort);
      };
      if (signal?.aborted === true) {
        cleanup();
        reject(createAbortError());
        return;
      }
      signal?.addEventListener?.("abort", onAbort);
    });
  }

  function maybeCaptureWasmMemory(result) {
    const instance =
      result != null && typeof result === "object" && "instance" in result
        ? result.instance
        : result;
    const memory = instance?.exports?.memory;
    if (memory instanceof WebAssembly.Memory) {
      latestWasmMemory = memory;
    }
  }

  // doFetch creates its timeout synchronously, just before calling fetch. Capture
  // that timer within the WASM fetch binding so a gated request can pause only
  // its own deadline. Other worker timers keep running normally.
  /** @param {WebAssembly.Imports | undefined} imports */
  function captureFetchTimers(imports) {
    if (
      !config.fetchRules.some((rule) =>
        rule.actions?.some((action) => action.waitForRelease),
      )
    ) {
      return;
    }
    const fetchBinding = imports?.wasp?.__js_func__fetch;
    if (imports === undefined || typeof fetchBinding !== "function") {
      throw new Error("Missing WASM fetch binding for gated requests");
    }
    if (patchedImports.has(imports)) {
      return;
    }
    patchedImports.add(imports);
    /** @param {...number} args */
    imports.wasp.__js_func__fetch = (...args) => {
      const previousSetTimeout = self.setTimeout;
      const previousFetchTimer = currentFetchTimer;
      currentFetchTimer = null;
      self.setTimeout = (callback, duration, ...timerArgs) => {
        if (typeof callback !== "function") {
          throw new Error("Expected a callback for the request timeout");
        }
        /** @type {FetchTimer} */
        const timer = {
          id: 0,
          nativeId: null,
          duration,
          run() {
            requestTimers.delete(timer.id);
            callback(...timerArgs);
          },
        };
        timer.id = timer.nativeId = originalSetTimeout(timer.run, duration);
        requestTimers.set(timer.id, timer);
        currentFetchTimer = timer;
        return timer.id;
      };
      try {
        return fetchBinding(...args);
      } finally {
        self.setTimeout = previousSetTimeout;
        currentFetchTimer = previousFetchTimer;
      }
    };
  }

  self.clearTimeout = (id) => {
    const timer = id === undefined ? undefined : requestTimers.get(id);
    if (timer !== undefined) {
      if (timer.nativeId !== null) {
        originalClearTimeout(timer.nativeId);
      }
      requestTimers.delete(timer.id);
    } else {
      originalClearTimeout(id);
    }
  };

  /**
   * @param {number} requestId
   * @param {AbortSignal | null | undefined} signal
   * @returns {Promise<void>}
   */
  function waitForFetchRelease(requestId, signal) {
    return new Promise((resolve, reject) => {
      const cleanup = () => {
        fetchReleases.delete(requestId);
        signal?.removeEventListener?.("abort", onAbort);
      };
      const onAbort = () => {
        cleanup();
        reject(createAbortError());
      };
      fetchReleases.set(requestId, () => {
        cleanup();
        resolve();
      });
      if (signal?.aborted === true) {
        onAbort();
      } else {
        signal?.addEventListener?.("abort", onAbort);
      }
    });
  }

  WebAssembly.instantiate = async function patchedInstantiate(source, imports) {
    captureFetchTimers(imports);
    const result = await originalInstantiate(source, imports);
    maybeCaptureWasmMemory(result);
    return result;
  };

  if (originalInstantiateStreaming !== null) {
    WebAssembly.instantiateStreaming =
      async function patchedInstantiateStreaming(source, imports) {
        captureFetchTimers(imports);
        const result = await originalInstantiateStreaming(source, imports);
        maybeCaptureWasmMemory(result);
        return result;
      };
  }

  async function createMemorySnapshot() {
    self.gc?.();
    const perfMemory = self.performance?.memory ?? null;
    let userAgentSpecificBytes = null;
    if (
      self.crossOriginIsolated &&
      typeof self.performance?.measureUserAgentSpecificMemory === "function"
    ) {
      try {
        const measurement =
          await self.performance.measureUserAgentSpecificMemory();
        userAgentSpecificBytes = measurement.bytes;
      } catch (_) {
        userAgentSpecificBytes = null;
      }
    }
    return {
      jsHeapUsedBytes: perfMemory?.usedJSHeapSize ?? null,
      jsHeapTotalBytes: perfMemory?.totalJSHeapSize ?? null,
      wasmMemoryBytes: latestWasmMemory?.buffer?.byteLength ?? null,
      userAgentSpecificBytes,
    };
  }

  channel?.addEventListener("message", (evt) => {
    const data = evt.data;
    if (data?.type === "release-fetch") {
      fetchReleases.get(data.requestId)?.();
      return;
    }
    if (data?.type !== "memory-snapshot-request") {
      return;
    }
    createMemorySnapshot().then((snapshot) => {
      postTelemetry({
        type: "memory-snapshot",
        requestId: data.requestId,
        ...snapshot,
      });
    });
  });

  self.fetch = async function patchedFetch(input, init) {
    const requestId = ++fetchRequestId;
    const url = typeof input === "string" ? input : input.url;
    let chosenRuleIndex = -1;
    let chosenRule;
    for (let i = 0; i < config.fetchRules.length; i++) {
      const rule = config.fetchRules[i];
      if (matchesRule(rule, url, init)) {
        chosenRuleIndex = i;
        chosenRule = rule;
        break;
      }
    }

    const { action, attempt } =
      chosenRuleIndex >= 0
        ? pickAction(chosenRule, chosenRuleIndex)
        : { action: { type: "passthrough" }, attempt: 1 };

    postTelemetry({
      type: "fetch-start",
      requestId,
      url,
      hasRange: getRangeHeader(init?.headers) !== undefined,
      ruleId: chosenRule?.id ?? null,
      ruleIndex: chosenRuleIndex >= 0 ? chosenRuleIndex : null,
      actionType: action.type,
      attempt,
    });

    try {
      if (action.waitForRelease) {
        const timer = currentFetchTimer;
        if (timer !== null) {
          if (timer.nativeId !== null) {
            originalClearTimeout(timer.nativeId);
          }
          timer.nativeId = null;
        }
        try {
          await waitForFetchRelease(requestId, init?.signal);
        } catch (error) {
          if (timer !== null) {
            self.clearTimeout(timer.id);
          }
          throw error;
        }
        if (timer !== null && requestTimers.has(timer.id)) {
          timer.nativeId = originalSetTimeout(timer.run, timer.duration);
        }
      }
      await delayWithAbort(action.delayMs, init?.signal);
      switch (action.type) {
        case "error":
          throw new TypeError(action.message ?? "Injected network error");
        case "timeout":
          return await new Promise((_, reject) => {
            const onAbort = () => {
              init?.signal?.removeEventListener?.("abort", onAbort);
              reject(createAbortError());
            };
            if (init?.signal?.aborted === true) {
              reject(createAbortError());
              return;
            }
            init?.signal?.addEventListener?.("abort", onAbort);
          });
        case "response": {
          const response = new Response(action.body ?? "", {
            status: action.status ?? 200,
            headers: action.headers,
          });
          Object.defineProperty(response, "url", { value: url });
          postTelemetry({
            type: "fetch-resolve",
            requestId,
            url,
            ruleId: chosenRule?.id ?? null,
            actionType: action.type,
            attempt,
            status: response.status,
            finalUrl: response.url || url,
            redirected: response.redirected,
          });
          return response;
        }
        case "passthrough":
        default: {
          const response = await originalFetch(input, init);
          postTelemetry({
            type: "fetch-resolve",
            requestId,
            url,
            ruleId: chosenRule?.id ?? null,
            actionType: action.type,
            attempt,
            status: response.status,
            finalUrl: response.url || url,
            redirected: response.redirected,
          });
          return response;
        }
      }
    } catch (error) {
      postTelemetry({
        type:
          error instanceof Error && error.name === "AbortError"
            ? "fetch-abort"
            : "fetch-reject",
        requestId,
        url,
        ruleId: chosenRule?.id ?? null,
        actionType: action.type,
        attempt,
        errorName: error instanceof Error ? error.name : undefined,
        message: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  };

  importScripts(config.workerUrl);
}
