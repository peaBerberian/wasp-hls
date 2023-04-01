import logger from "../ts-common/logger";
import { MainMessage } from "../ts-common/types";

/**
 * Post given `msg` message to the given `worker` with optional `Transferable`
 * values.
 *
 * One of the main advantage of this separte method is that it is completely
 * type-checked for all messages that may send the Main thread.
 * @param {Worker} worker
 * @param {Object} msg
 * @param {Array.<*>} transferables
 */
export default function postMessageToWorker(
  worker: Worker,
  msg: MainMessage,
  transferables?: Transferable[]
): void {
  logger.debug("--> sending to worker:", msg.type);
  // Written that way due to TypeScript shenanigans
  if (transferables === undefined) {
    worker.postMessage(msg);
  } else {
    worker.postMessage(msg, transferables);
  }
}
