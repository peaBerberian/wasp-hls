/**
 * Common utility function allowing to determine if a variable is equal to
 * `null` or `undefined`.
 *
 * This is the same as doing `x == null`, excepted this call is much more
 * explicit about the intent.
 *
 * @param {*} x
 * @returns {boolean} - Returns `true` if `x` is equal to `null` or `undefined`.
 * Returns `false` in any other cases.
 */
export default function isNullOrUndefined(x: unknown): x is null | undefined {
  return x === null || x === undefined;
}
