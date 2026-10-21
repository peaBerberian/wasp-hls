/** Slow whole-buffer oracle: no streaming state or optimized scanning. */
export function referenceNals(input) {
  const bytes = Buffer.from(input);
  // Latin-1 maps each byte to one character. Regex matching is independent of
  // the production scanner's incremental cursor and pending-zero state.
  const delimiters = [...bytes.toString("latin1").matchAll(/\x00{2,}\x01/g)];
  const units = [];
  for (let i = 0; i < delimiters.length; i++) {
    const start = delimiters[i].index + delimiters[i][0].length;
    const end = delimiters[i + 1]?.index ?? bytes.length;
    if (end > start) units.push(bytes.subarray(start, end));
  }
  return units;
}

export function randomGenerator(seed) {
  return () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return seed >>> 0;
  };
}

export function randomChunks(bytes, random, maximum = 97) {
  const chunks = [];
  for (let offset = 0; offset < bytes.length;) {
    if (random() % 4 === 0) chunks.push(new Uint8Array());
    const end = Math.min(bytes.length, offset + 1 + (random() % maximum));
    chunks.push(bytes.subarray(offset, end));
    offset = end;
  }
  return chunks;
}
