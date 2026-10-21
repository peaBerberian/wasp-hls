import assert from "node:assert/strict";
import test, { after } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";
import { randomGenerator } from "./helpers/nal-reference.mjs";

const directory = await mkdtemp(join(tmpdir(), "wasp-mp4-assembly-"));
after(() => rm(directory, { recursive: true, force: true }));
const outfile = join(directory, "assembly.mjs");
await build({
  entryPoints: ["src/ts-transmux/src/mp4-fragment.ts"],
  bundle: true,
  format: "esm",
  platform: "node",
  outfile,
});
const { createFragment, assembleSegment } = await import(
  pathToFileURL(outfile).href
);

function u32(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32BE(value);
  return bytes;
}

/** Independent, deliberately copy-heavy layout used before direct assembly. */
function referenceAssembly(init, fragments) {
  const boxes = fragments.map((fragment) => {
    const payload = Buffer.concat(
      fragment.payload.map((part) =>
        fragment.lengthPrefixed
          ? Buffer.concat([u32(part.length), part])
          : Buffer.from(part),
      ),
    );
    const mdat = Buffer.concat([
      u32(payload.length + 8),
      Buffer.from("mdat"),
      payload,
    ]);
    return Buffer.concat([fragment.moof, mdat]);
  });
  return Buffer.concat([init, ...boxes]);
}

test("direct output matches independent concatenation for seeded AVC/AAC fragment layouts", () => {
  const random = randomGenerator(0xabc017);
  for (let trial = 0; trial < 1000; trial++) {
    const init = Uint8Array.from(
      { length: random() % 80 },
      () => random() % 256,
    );
    const fragments = Array.from({ length: random() % 4 }, () =>
      createFragment(
        Uint8Array.from({ length: random() % 64 }, () => random() % 256),
        Array.from({ length: random() % 20 }, () =>
          Uint8Array.from({ length: random() % 257 }, () => random() % 256),
        ),
        Boolean(random() % 2),
      ),
    );
    assert.deepEqual(
      Buffer.from(assembleSegment(init, fragments)),
      referenceAssembly(init, fragments),
      `trial=${trial}`,
    );
  }
});

test("AVC lengths are big-endian, subarray offsets are respected, and fragments keep order", () => {
  const backing = Uint8Array.from([99, 11, 12, 13, 99]);
  const audio = createFragment(
    Uint8Array.of(21),
    [backing.subarray(1, 3)],
    false,
  );
  const video = createFragment(
    Uint8Array.of(31),
    [backing.subarray(2, 4), new Uint8Array(300).fill(42)],
    true,
  );
  const actual = assembleSegment(Uint8Array.of(1, 2), [audio, video]);
  assert.deepEqual(
    Buffer.from(actual),
    referenceAssembly(Uint8Array.of(1, 2), [audio, video]),
  );
  const view = new DataView(actual.buffer);
  const videoStart = 2 + audio.byteLength;
  assert.equal(view.getUint32(videoStart + 1), 8 + 4 + 2 + 4 + 300);
  assert.equal(view.getUint32(videoStart + 1 + 8), 2);
  assert.equal(view.getUint32(videoStart + 1 + 8 + 4 + 2), 300);
  assert.deepEqual([...backing], [99, 11, 12, 13, 99]);
});

test("payloads are copied only once, into the one final output allocation", () => {
  const parts = [new Uint8Array(1000).fill(7), new Uint8Array(2000).fill(9)];
  const fragment = createFragment(Uint8Array.of(1, 2), parts, true);
  const set = Uint8Array.prototype.set;
  const copies = [];
  let output;
  Uint8Array.prototype.set = function (source, offset) {
    copies.push({ target: this, source });
    return set.call(this, source, offset);
  };
  try {
    output = assembleSegment(Uint8Array.of(3, 4), [fragment]);
  } finally {
    Uint8Array.prototype.set = set;
  }
  assert.ok(copies.every((copy) => copy.target === output));
  for (const part of parts)
    assert.equal(copies.filter((copy) => copy.source === part).length, 1);
  const saved = Buffer.from(output);
  parts[0].fill(0);
  parts[1].fill(0);
  assert.deepEqual(
    Buffer.from(output),
    saved,
    "returned bytes own their storage",
  );
});
