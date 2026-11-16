export type Mark = { name: string; time: number; details?: unknown };
export type Recorder = (name: string, details?: unknown) => void;

/** Observers belong to one operation and are always cancelled on completion. */
export class Observations {
  private controller = new AbortController();
  private frames = new Map<number, HTMLVideoElement>();
  private record: Recorder;
  constructor(record: Recorder) {
    this.record = record;
  }

  event(video: HTMLVideoElement, name: string): Promise<number> {
    return new Promise((resolve) =>
      video.addEventListener(
        name,
        () => {
          this.record(name);
          resolve(performance.now());
        },
        { once: true, signal: this.controller.signal },
      ),
    );
  }

  frame(
    video: HTMLVideoElement,
    predicate: (metadata: VideoFrameCallbackMetadata) => boolean = () => true,
  ): Promise<number> {
    return new Promise((resolve) => {
      const request = () => {
        const id = video.requestVideoFrameCallback((_now, metadata) => {
          this.frames.delete(id);
          if (this.controller.signal.aborted) return;
          if (!predicate(metadata)) {
            request();
            return;
          }
          const timestamp = performance.now();
          this.record("frame", {
            mediaTime: metadata.mediaTime,
            processingDuration: metadata.processingDuration,
          });
          resolve(timestamp);
        });
        this.frames.set(id, video);
      };
      request();
    });
  }

  /** Register before load. Do not accept a frame queued by the previous source. */
  load(video: HTMLVideoElement) {
    // The listener runs synchronously during loadedmetadata dispatch, before a
    // subsequent canplay event. Frame registration is deliberately gated here.
    let newMetadata = false;
    const afterMetadata = (name: string) =>
      new Promise<number>((resolve) => {
        const listener = () => {
          if (!newMetadata) return;
          video.removeEventListener(name, listener);
          this.record(name);
          resolve(performance.now());
        };
        video.addEventListener(name, listener, {
          signal: this.controller.signal,
        });
      });
    const canPlay = afterMetadata("canplay");
    const playing = afterMetadata("playing");
    const metadata = this.event(video, "loadedmetadata");
    let resolveFrame: (time: number) => void;
    const firstFrame = new Promise<number>((resolve) => {
      resolveFrame = resolve;
    });
    video.addEventListener(
      "loadedmetadata",
      () => {
        newMetadata = true;
        void this.frame(video).then(resolveFrame);
      },
      { once: true, signal: this.controller.signal },
    );
    return {
      metadata,
      canPlay,
      firstFrame,
      playing,
    };
  }

  dispose() {
    this.controller.abort();
    for (const [id, video] of this.frames) video.cancelVideoFrameCallback(id);
    this.frames.clear();
  }
}
