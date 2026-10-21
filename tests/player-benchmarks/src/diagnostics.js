/**
 * Session diagnostics only. Wrapping workers changes their startup cost; never
 * mix these samples with ordinary performance measurements.
 */
export function installDiagnostics() {
  const events = [];
  const channelName = `diagnostic-${Math.random()}`;
  const channel = new BroadcastChannel(channelName);
  channel.onmessage = ({ data }) => events.push(data);
  instrument((event) => events.push(event));
  const OriginalWorker = window.Worker;
  window.Worker = class extends OriginalWorker {
    constructor(url, options) {
      const script = `(${workerBootstrap.toString()})(${JSON.stringify(String(url))}, ${JSON.stringify(channelName)}, ${instrument.toString()});`;
      super(
        URL.createObjectURL(new Blob([script], { type: "text/javascript" })),
        options,
      );
    }
  };
  return events;
}

function workerBootstrap(url, channelName, instrumentWorker) {
  const channel = new BroadcastChannel(channelName);
  globalThis.__waspDiagnosticChannel = channel;
  instrumentWorker((event) => channel.postMessage(event));
  const request = new XMLHttpRequest();
  request.open("GET", url, false);
  request.send();
  // Instrument the built Wasp method without editing production code. A
  // failed match is reported explicitly rather than implying zero CPU time.
  let source = request.responseText;
  const pattern = /transmuxSegment\(([^)]*)\)\s*\{/;
  if (source.includes("transmuxSegment")) {
    if (!pattern.test(source))
      throw new Error("Wasp transmux diagnostic pattern did not match");
    source = source.replace(
      pattern,
      `transmuxSegment(...args) {
        const start = performance.timeOrigin + performance.now();
        try { return this.__diagnosticTransmux(...args); }
        finally { globalThis.__waspDiagnosticChannel.postMessage({ name: "transmux", realm: "worker", time: start, duration: performance.timeOrigin + performance.now() - start, stack: new Error().stack }); }
      }
      __diagnosticTransmux($1) {`,
    );
  }
  (0, eval)(source);
}

function instrument(send) {
  const emit = (name, details = {}) =>
    send({
      name,
      time: performance.timeOrigin + performance.now(),
      realm: typeof document === "undefined" ? "worker" : "main",
      ...details,
    });
  if (typeof MediaSource !== "undefined") {
    const add = MediaSource.prototype.addSourceBuffer;
    MediaSource.prototype.addSourceBuffer = function (mime) {
      emit("sourcebuffer", { mime });
      return add.call(this, mime);
    };
  }
  if (typeof SourceBuffer !== "undefined") {
    const changeType = SourceBuffer.prototype.changeType;
    SourceBuffer.prototype.changeType = function (mime) {
      emit("changeType", { mime });
      return changeType.call(this, mime);
    };
    const append = SourceBuffer.prototype.appendBuffer;
    SourceBuffer.prototype.appendBuffer = function (data) {
      emit("append", { bytes: data.byteLength });
      this.addEventListener("updateend", () => emit("updateend"), {
        once: true,
      });
      return append.call(this, data);
    };
  }
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async function (...args) {
    emit("fetch", { url: String(args[0]) });
    const response = await originalFetch.apply(this, args);
    emit("response", { url: response.url });
    const arrayBuffer = response.arrayBuffer.bind(response);
    response.arrayBuffer = async () => {
      const result = await arrayBuffer();
      emit("body", { url: response.url, bytes: result.byteLength });
      return result;
    };
    return response;
  };
}
