enum SourceBufferOperation {
  Push,
  Remove,
}

/*
 * Action created by the QueuedSourceBuffer to push a chunk.
 * Will be converted into an `PushQueueItem` once in the queue
 */
interface PushAction {
  type : SourceBufferOperation.Push;
  value : BufferSource;
}

/**
 * Action created by the QueuedSourceBuffer to remove Segment(s).
 * Will be converted into an `RemoveQueueItem` once in the queue
 */
interface RemoveAction {
  type : SourceBufferOperation.Remove;
  value : {
    start : number;
    end : number;
  };
}

/** Actions understood by the QueuedSourceBuffer. */
type QSBAction =
  PushAction |
  RemoveAction;

/** Item waiting in the queue to push a new chunk to the SourceBuffer. */
interface PushQueueItem extends PushAction {
  resolve : () => void;
  reject : (err : Error) => void;
}

/** Item waiting in the queue to remove segment(s) from the SourceBuffer. */
interface RemoveQueueItem extends RemoveAction {
  resolve : () => void;
  reject : (err : Error) => void;
}

/** Action waiting in the queue. */
type QSBQueueItem =
  PushQueueItem |
  RemoveQueueItem;

/**
 * Allows to push and remove new Segments to a SourceBuffer in a FIFO queue (not
 * doing so can lead to browser Errors).
 *
 * To work correctly, only a single QueuedSourceBuffer per SourceBuffer should
 * be created.
 *
 * @class QueuedSourceBuffer
 */
export default class QueuedSourceBuffer {
  /** SourceBuffer implementation. */
  private readonly _sourceBuffer : SourceBuffer;

  /**
   * Queue of awaited buffer "operations".
   * The first element in this array will be the first performed.
   */
  private _queue : QSBQueueItem[];

  /**
   * Information about the current operation processed by the
   * QueuedSourceBuffer.
   * If equal to null, it means that no operation from the queue is currently
   * being processed.
   */
  private _pendingTask : QSBQueueItem | null;

  private _dispose : Array<() => void>;

  /**
   * @constructor
   * @param {SourceBuffer} sourceBuffer
   */
  constructor(sourceBuffer : SourceBuffer) {
    this._sourceBuffer = sourceBuffer;
    this._queue = [];
    this._pendingTask = null;

    // Some browsers (happened with firefox 66) sometimes "forget" to send us
    // `update` or `updateend` events.
    // In that case, we're completely unable to continue the queue here and
    // stay locked in a waiting state.
    // This interval is here to check at regular intervals if the underlying
    // SourceBuffer is currently updating.
    const intervalId = setInterval(() => {
      this._flush();
    }, 2000);

    const onError = this._onPendingTaskError.bind(this);


    const _onUpdateEnd = () => {
      this._flush();
    };

    sourceBuffer.addEventListener("error", onError);
    sourceBuffer.addEventListener("updateend", _onUpdateEnd);

    this._dispose = [() => {
      clearInterval(intervalId);
      sourceBuffer.removeEventListener("error", onError);
      sourceBuffer.removeEventListener("updateend", _onUpdateEnd);
    }];
  }

  /**
   * Push a chunk of the media segment given to the attached SourceBuffer, in a
   * FIFO queue.
   *
   * Depending on the type of data appended, this might need an associated
   * initialization segment.
   *
   * @param {BufferSource} data
   * @returns {Promise}
   */
  public push(data: BufferSource) : Promise<void> {
    console.debug("QSB: receiving order to push data to the SourceBuffer");
    return this._addToQueue({ type: SourceBufferOperation.Push, value: data });
  }

  /**
   * Remove buffered data (added to the same FIFO queue than `push`).
   * @param {number} start - start position, in seconds
   * @param {number} end - end position, in seconds
   * @returns {Promise}
   */
  public removeBuffer(start : number, end : number) : Promise<void> {
    console.debug(
      "QSB: receiving order to remove data from the SourceBuffer",
      start,
      end
    );
    return this._addToQueue({
      type: SourceBufferOperation.Remove,
      value: { start, end },
    });
  }

  /**
   * Returns the currently buffered data, in a TimeRanges object.
   * @returns {TimeRanges}
   */
  public getBufferedRanges() : TimeRanges {
    return this._sourceBuffer.buffered;
  }

  /**
   * Dispose of the resources used by this QueuedSourceBuffer.
   *
   * /!\ You won't be able to use the QueuedSourceBuffer after calling this
   * function.
   * @private
   */
  public dispose() : void {
    this._dispose.forEach(disposeFn => disposeFn());

    if (this._pendingTask !== null) {
      this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled"));
      this._pendingTask = null;
    }

    while (this._queue.length > 0) {
      const nextElement = this._queue.shift();
      if (nextElement !== undefined) {
        nextElement.reject(new Error("QueuedSourceBuffer Cancelled"));
      }
    }

    // try {
    //   this._sourceBuffer.abort();
    // } catch (e) {
    //   console.warn(
    //     `QSB: Failed to abort a SourceBuffer:`,
    //     e instanceof Error ? e : "Unknown error"
    //   );
    // }
  }

  /**
   * Called when an error arised that made the current task fail.
   * @private
   * @param {*} err
   */
  private _onPendingTaskError(err : unknown) : void {
    const error = err instanceof Error ?
      err :
      new Error(
        "An unknown error occured when doing operations " +
        "on the SourceBuffer"
      );

    if (this._pendingTask != null) {
      this._pendingTask.reject(error);
    }
  }

  /**
   * Add your operation to the queue. and begin the queue if not already
   * started.
   * @private
   * @param {Object} operation
   * @returns {Promise}
   */
  private _addToQueue(operation : QSBAction) : Promise<void> {
    return new Promise((resolve, reject) => {
      const shouldRestartQueue =
        this._queue.length === 0 &&
        this._pendingTask === null;
      const queueItem = { resolve, reject, ...operation };
      this._queue.push(queueItem);
      if (shouldRestartQueue) {
        this._flush();
      }
    });
  }

  /**
   * Perform next task if one.
   * @private
   */
  private _flush() : void {
    if (this._sourceBuffer.updating) {
      return; // still processing `this._pendingTask`
    }

    if (this._pendingTask !== null) {
      const task = this._pendingTask;
      const { resolve } = task;
      this._pendingTask = null;
      resolve();
      return this._flush(); // Go to next item in queue
    } else { // if this._pendingTask is null, go to next item in queue
      const nextItem = this._queue.shift();
      if (nextItem === undefined) {
        return; // we have nothing left to do
      } else {
        this._pendingTask = nextItem;
      }
    }

    try {
      switch (this._pendingTask.type) {
        case SourceBufferOperation.Push:
          const segmentData = this._pendingTask.value;
          if (segmentData === undefined) {
            this._flush();
            return;
          }
          console.debug("QSB: pushing data");
          this._sourceBuffer.appendBuffer(segmentData);
          break;

        case SourceBufferOperation.Remove:
          const { start, end } = this._pendingTask.value;
          console.debug(
            "QSB: removing data from SourceBuffer",
            start,
            end
          );
          this._sourceBuffer.remove(start, end);
          break;

        default:
          assertUnreachable(this._pendingTask);
      }
    } catch (e) {
      this._onPendingTaskError(e);
    }
  }
}

/**
 * TypeScript hack to make sure a code path is never taken.
 *
 * This can for example be used to ensure that a switch statement handle all
 * possible cases by adding a default clause calling assertUnreachable with
 * an argument (it doesn't matter which one).
 *
 * @example
 * function parseBinary(str : "0" | "1") : number {
 *   switch (str) {
 *     case "0:
 *       return 0;
 *     case "1":
 *       return 1;
 *     default:
 *       // branch never taken. If it can be, TypeScript will yell at us because
 *       // its argument (here, `str`) is not of the right type.
 *       assertUnreachable(str);
 *   }
 * }
 * @param {*} _
 * @throws AssertionError - Throw an AssertionError when called. If we're
 * sufficiently strict with how we use TypeScript, this should never happen.
 */
function assertUnreachable(_: never): never {
  throw new Error("Unreachable path taken");
}

