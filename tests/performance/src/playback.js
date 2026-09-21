/** Wait for an observable playback condition, with a bounded failure. */
export function waitUntil(predicate, description, timeoutMs = 15_000) {
  return new Promise((resolve, reject) => {
    const start = performance.now();
    const timer = setInterval(() => {
      try {
        if (predicate()) {
          clearInterval(timer);
          resolve();
        } else if (performance.now() - start >= timeoutMs) {
          throw new Error(`Timed out waiting for ${description}`);
        }
      } catch (error) {
        clearInterval(timer);
        reject(error);
      }
    }, 10);
  });
}

/** Resolve after the compositor receives a frame satisfying the predicate. */
export function waitForFrame(video, predicate = () => true) {
  if (typeof video.requestVideoFrameCallback !== "function") {
    throw new Error("First-frame benchmarks require requestVideoFrameCallback");
  }
  let callbackId;
  let timeoutId;
  const promise = new Promise((resolve, reject) => {
    const onFrame = (_now, metadata) => {
      if (predicate(metadata)) {
        clearTimeout(timeoutId);
        resolve(metadata);
      } else {
        callbackId = video.requestVideoFrameCallback(onFrame);
      }
    };
    timeoutId = setTimeout(() => {
      video.cancelVideoFrameCallback(callbackId);
      reject(new Error("Timed out waiting for a displayed video frame"));
    }, 15_000);
    callbackId = video.requestVideoFrameCallback(onFrame);
  });
  // Observe immediately, even when the caller awaits initialize/play first.
  promise.catch(() => {});
  return promise;
}

export function waitForLoaded(player) {
  return waitUntil(() => {
    if (player.getPlayerState() === "Error") {
      throw new Error("Player entered the Error state");
    }
    return player.getPlayerState() === "Loaded";
  }, "the Loaded state").catch((error) => {
    const video = document.querySelector("video");
    throw new Error(
      `${error.message}: ${JSON.stringify({
        state: player.getPlayerState(),
        error: player.getError(),
        readyState: video.readyState,
        currentTime: video.currentTime,
        duration: video.duration,
        buffered: Array.from({ length: video.buffered.length }, (_, i) => [
          video.buffered.start(i),
          video.buffered.end(i),
        ]),
      })}`,
    );
  });
}

/** Preserve EXTINF durations and resolve fixture URIs against their playlist. */
export function parseMediaPlaylist(text, url) {
  const initUri = text.match(/#EXT-X-MAP:URI="([^"]+)"/)?.[1];
  const segments = [];
  let duration;
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith("#EXTINF:")) {
      duration = Number(line.slice(8).split(",")[0]);
    } else if (line.length > 0 && !line.startsWith("#")) {
      if (!Number.isFinite(duration) || duration <= 0) {
        throw new Error("Fixture segment has no valid EXTINF duration");
      }
      segments.push({ url: new URL(line, url).href, duration });
      duration = undefined;
    }
  }
  if (segments.length < 6) {
    throw new Error("Playback benchmarks need at least six fixture segments");
  }
  return {
    initUrl: initUri === undefined ? undefined : new URL(initUri, url).href,
    segments,
    targetDuration: Math.ceil(Math.max(...segments.map((s) => s.duration))),
  };
}

export function isBuffered(video, position) {
  for (let i = 0; i < video.buffered.length; i++) {
    if (
      video.buffered.start(i) <= position &&
      video.buffered.end(i) > position
    ) {
      return true;
    }
  }
  return false;
}
