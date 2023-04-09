const MAX_UINT32 = Math.pow(2, 32);

function getUint64(uint8: Uint8Array): number | bigint {
  const dv = new DataView(uint8.buffer, uint8.byteOffset, uint8.byteLength);
  if (typeof dv.getBigUint64 === "function") {
    const value = dv.getBigUint64(0);
    if (value < Number.MAX_SAFE_INTEGER) {
      return Number(value);
    }
    return value;
  }
  return dv.getUint32(0) * MAX_UINT32 + dv.getUint32(4);
}

export { getUint64, MAX_UINT32 };
