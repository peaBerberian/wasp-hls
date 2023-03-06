import logger from "../ts-common/logger";
import { WorkerMessage } from "../ts-common/types";

export default function postMessageToMain(
  msg: WorkerMessage,
  transferables?: Transferable[]
) {
  logger.debug("<-- sending to main:", msg.type);
  // Written that way due to TypeScript shenanigans
  if (transferables === undefined) {
    postMessage(msg);
  } else {
    postMessage(msg, transferables);
  }
}
