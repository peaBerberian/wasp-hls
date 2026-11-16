export function startPressure() {
  const host = document.createElement("div");
  host.style.cssText =
    "width:600px;height:240px;overflow:hidden;position:absolute;top:400px";
  const nodes = Array.from({ length: 400 }, () =>
    host.appendChild(document.createElement("div")),
  );
  document.body.appendChild(host);
  const payload = JSON.stringify(
    Array.from({ length: 500 }, (_, i) => ({
      id: i,
      text: `row-${i}`,
      value: i * 1.5,
    })),
  );
  let timer = 0;
  let batches = 0;
  let busyMs = 0;
  let checksum = 0;
  let stopped = false;
  function work() {
    const start = performance.now();
    for (const [i, node] of nodes.entries()) {
      node.textContent = `${batches}:${i}`;
      node.style.width = `${200 + ((i + batches) % 200)}px`;
    }
    checksum += nodes[batches % nodes.length].offsetWidth;
    for (let pass = 0; pass < 48; pass++) {
      const rows = JSON.parse(payload) as { value: number }[];
      checksum += JSON.stringify(rows).length;
    }
    busyMs += performance.now() - start;
    batches++;
    if (!stopped) timer = window.setTimeout(work, 16);
  }
  timer = window.setTimeout(work, 0);
  return () => {
    stopped = true;
    clearTimeout(timer);
    host.remove();
    return {
      batches,
      busyMs,
      checksum,
      jsonPassesPerBatch: 48,
      idleMs: 16,
      nodes: nodes.length,
    };
  };
}
