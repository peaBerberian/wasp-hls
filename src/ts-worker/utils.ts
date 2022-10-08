export function formatErrMessage(err: unknown, defaultMsg: string) {
  return err instanceof Error ?
    err.name + ": " + err.message :
    defaultMsg;
}
