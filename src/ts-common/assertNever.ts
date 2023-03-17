export default function assertNever(_arg: never): void {
  throw new Error("This function should never be called");
}
