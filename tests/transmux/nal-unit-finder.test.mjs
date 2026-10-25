import assert from "node:assert/strict";
import test, { after } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";
import {
  referenceNals,
  randomChunks,
  randomGenerator,
} from "./helpers/nal-reference.mjs";

const directory = await mkdtemp(join(tmpdir(), "wasp-nal-tests-"));
after(() => rm(directory, { recursive: true, force: true }));
const outfile = join(directory, "nal-finder.mjs");
await build({
  entryPoints: ["src/ts-transmux/src/H264NalUnitProducer.ts"],
  bundle: true,
  format: "esm",
  platform: "node",
  outfile,
});
const { NalUnitFinder: Finder } = await import(pathToFileURL(outfile).href);
const arrays = (units) => units.map((unit) => [...unit]);

function split(chunks, finder = new Finder()) {
  const units = chunks.flatMap((chunk) => finder.findNext(chunk));
  const tail = finder.flush();
  if (tail !== null) units.push(tail);
  return arrays(units);
}

test("known framing outputs across every partition of short streams", () => {
  const cases = [
    {
      bytes: [0, 0, 1, 0x65, 1, 0, 0, 0, 0, 1, 0x41, 0x17],
      units: [
        [0x65, 1],
        [0x41, 0x17],
      ],
    },
    {
      bytes: [0, 0, 0, 1, 0x65, 0, 0, 3, 1, 0, 0, 1, 0x41],
      units: [[0x65, 0, 0, 3, 1], [0x41]],
    },
    {
      bytes: [0, 0, 1, 0x65, 0, 0, 2, 0, 0, 1, 0x41],
      units: [[0x65, 0, 0, 2], [0x41]],
    },
    { bytes: [0, 0, 1, 0, 0, 1, 0x41], units: [[0x41]] },
    { bytes: [9, 0, 0, 1, 0x65, 0, 0], units: [[0x65, 0, 0]] },
  ];
  for (const { bytes, units } of cases) {
    assert.deepEqual(
      arrays(referenceNals(bytes)),
      units,
      "oracle known output",
    );
    for (let mask = 0; mask < 2 ** (bytes.length - 1); mask++) {
      const chunks = [new Uint8Array()];
      let start = 0;
      for (let end = 1; end <= bytes.length; end++) {
        if (end === bytes.length || mask & (1 << (end - 1))) {
          chunks.push(
            Uint8Array.from(bytes.slice(start, end)),
            new Uint8Array(),
          );
          start = end;
        }
      }
      assert.deepEqual(
        split(chunks),
        units,
        `bytes=${bytes}, partition=${mask}`,
      );
    }
  }
});

test("zero runs stay pending until the following byte confirms a delimiter", () => {
  for (let length = 2; length <= 64; length++)
    for (const following of [1, 2, 3, 4, 0xff]) {
      const bytes = Uint8Array.from([
        0,
        0,
        1,
        0x65,
        7,
        ...Array(length).fill(0),
        following,
        0x41,
        9,
      ]);
      const expected = arrays(referenceNals(bytes));
      for (let cut = 0; cut <= bytes.length; cut++)
        assert.deepEqual(
          split([bytes.subarray(0, cut), bytes.subarray(cut)]),
          expected,
          `zeros=${length}, next=${following}, cut=${cut}`,
        );
    }
});

test("seeded valid escaped payloads agree with the independent whole-buffer oracle", () => {
  const random = randomGenerator(0x1571a1);
  for (let trial = 0; trial < 2000; trial++) {
    const input = [];
    const expected = [];
    for (let n = 0, count = 1 + (random() % 8); n < count; n++) {
      input.push(...Array(2 + (random() % 6)).fill(0), 1);
      const payload = [n % 2 ? 0x41 : 0x65];
      let zeros = 0;
      for (let i = 0, length = random() % 250; i < length; i++) {
        const byte = random() % 3 === 0 ? random() % 4 : random() % 256;
        if (zeros >= 2 && byte <= 3) {
          payload.push(3);
          zeros = 0;
        }
        payload.push(byte);
        zeros = byte === 0 ? zeros + 1 : 0;
      }
      payload.push(0x80); // A nonzero RBSP end makes generated NAL boundaries unambiguous.
      input.push(...payload);
      expected.push(payload);
    }
    const bytes = Uint8Array.from(input);
    assert.deepEqual(
      arrays(referenceNals(bytes)),
      expected,
      `oracle seed trial=${trial}`,
    );
    assert.deepEqual(
      split(randomChunks(bytes, random)),
      expected,
      `chunk seed trial=${trial}`,
    );
    assert.deepEqual(split([bytes]), expected);
  }
});

test("empty, incomplete and unframed input does not invent NAL units", () => {
  for (const bytes of [
    [],
    [0],
    [0, 0],
    [0, 0, 1],
    [0, 0, 0, 1],
    [8, 9, 10, 11],
    [0, 0, 1, 0, 0, 1],
  ]) {
    const finder = new Finder();
    assert.deepEqual(split([Uint8Array.from(bytes)], finder), []);
    assert.equal(finder.flush(), null);
    assert.deepEqual(finder.findNext(new Uint8Array()), []);
  }
});

test("seeded zero-heavy and truncated byte streams agree with the reference", () => {
  const random = randomGenerator(0xb0a7da7a);
  const alphabet = [0, 0, 0, 1, 2, 3, 0x41, 0x65, 0xff];
  for (let trial = 0; trial < 5000; trial++) {
    const bytes = Uint8Array.from(
      { length: random() % 300 },
      () => alphabet[random() % alphabet.length],
    );
    assert.deepEqual(
      split(randomChunks(bytes, random, 23)),
      arrays(referenceNals(bytes)),
      `arbitrary seed trial=${trial}`,
    );
  }
});

test("an unfinished NAL resumes scanning after its already scanned payload", () => {
  const finder = new Finder();
  const input = new Uint8Array(100003).fill(0x65);
  input.set([0, 0, 1]);
  finder.findNext(input);
  const indexOf = Uint8Array.prototype.indexOf;
  const starts = [];
  Uint8Array.prototype.indexOf = function (value, start) {
    starts.push(start);
    return indexOf.call(this, value, start);
  };
  try {
    finder.findNext(Uint8Array.of(0x41));
  } finally {
    Uint8Array.prototype.indexOf = indexOf;
  }
  assert.deepEqual(starts, [100000]);
});

test("flush preserves incomplete payload bytes and resets all streaming state", () => {
  const finder = new Finder();
  for (const suffix of [[], [0], [0, 0], [0, 0, 0], [0, 0, 3]]) {
    assert.deepEqual(
      split([Uint8Array.from([0, 0, 1, 0x65, ...suffix])], finder),
      [[0x65, ...suffix]],
    );
    assert.equal(finder.flush(), null);
  }
});

test("reset discards pending bytes, including partially consumed delimiters", () => {
  const finder = new Finder();
  const abandoned = [0, 0, 1, 0x65, 1, 0, 0, 0, 0, 1];
  for (let cut = 0; cut <= abandoned.length; cut++) {
    finder.findNext(Uint8Array.from(abandoned.slice(0, cut)));
    finder.reset();
    finder.reset();
    assert.deepEqual(split([Uint8Array.from([0, 0, 1, 0x41, 9])], finder), [
      [0x41, 9],
    ]);
  }
});

test("emitted views remain unchanged by later feeds, reset and flush", () => {
  const finder = new Finder();
  const [first] = finder.findNext(
    Uint8Array.from([0, 0, 1, 0x65, 8, 0, 0, 1, 0x41]),
  );
  assert.deepEqual([...first], [0x65, 8]);
  finder.findNext(Uint8Array.from([9, 0, 0, 1, 0x65, 2]));
  finder.flush();
  finder.reset();
  assert.deepEqual([...first], [0x65, 8]);
});
