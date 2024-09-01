/**
 * Takes the TimeRange object returned by MSE `buffered` API and transform it
 * into a Float64Array where every even indices are a new start range and odd
 * indices are the next end range (it basically a flatten TimeRange object).
 *
 * @param {TimeRange} timeRanges
 * @returns {Float64Array}
 */
export default function timeRangesToFloat64Array(
  timeRanges: TimeRanges,
): Float64Array {
  const buffered = new Float64Array(timeRanges.length * 2);
  for (let i = 0; i < timeRanges.length; i++) {
    const offset = i * 2;
    buffered[offset] = timeRanges.start(i);
    buffered[offset + 1] = timeRanges.end(i);
  }
  return buffered;
}
