/**
 * Creates an ID generator which generates a different string identifier each
 * time you call it.
 * @returns {Function}
 */
export default function idGenerator() : () => string {
  let prefix = "";
  let currId = -1;
  return function generateNewId() : string {
    currId++;
    if (currId >= Number.MAX_SAFE_INTEGER) {
      prefix += "0";
      currId = 0;
    }
    return prefix + String(currId);
  };
}

export function numberIdGenerator() : () => number {
  let currId = -1;
  return function generateNewNumberId() : number {
    currId++;
    if (currId >= Number.MAX_SAFE_INTEGER) {
      console.warn("Exceeding `numberIdGenerator` limit. Collisions may occur");
      currId = 0;
    }
    return currId;
  };
}
