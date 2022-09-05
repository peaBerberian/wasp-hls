import { MainMessage } from "../ts-common/types";

export default function postMessageToWorker(
  worker: Worker,
  msg: MainMessage,
  transferables?: Transferable[]
) {
  console.debug("--> sending to worker:", msg.type);
  // Written that way due to TypeScript shenanigans
  if (transferables === undefined) {
    worker.postMessage(msg);
  } else {
    worker.postMessage(msg, transferables);
  }
}
