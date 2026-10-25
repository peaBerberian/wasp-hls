import test from "node:test";
import assert from "node:assert/strict";
import {
  parseOptions,
  samplePlan,
  scenarios as scenarioNames,
} from "./config.mjs";
import { manifestVariant, describeMediaPlaylist } from "./fixtures.mjs";
import { Observations } from "./src/observations.ts";
import { scenarios } from "./src/scenarios/index.ts";
import { loadContent } from "./src/scenarios/startup.ts";
import { checkMemoryBuffer } from "./src/memory.ts";
import { readProcessTreePss } from "./process-memory.mjs";

test("selection validates values, deduplicates and preserves equals in paths", () => {
  const options = parseOptions(
    ["--player=wasp", "--player", "wasp", "--report=/tmp/a=b.json"],
    "/tmp",
  );
  assert.deepEqual(options.players, ["wasp"]);
  assert.equal(options.report, "/tmp/a=b.json");
  for (const args of [
    ["--iterations=0"],
    ["--player=dashjs"],
    ["--manifest=direct"],
    ["--format"],
    ["--diagnostic=false"],
  ])
    assert.throws(() => parseOptions(args, "/tmp"));
});

test("sample blocks interleave controls, rotating player and condition order", () => {
  const options = parseOptions(
    [
      "--scenario=playback",
      "--scenario=pressure",
      "--iterations=2",
      "--format=mpegts",
      "--manifest=master",
    ],
    "/tmp",
  );
  const plan = [...samplePlan(options)];
  assert.equal(plan.length, 12);
  assert.deepEqual(
    plan.slice(0, 2).map((x) => [x.player, x.scenario]),
    [
      ["wasp", "playback"],
      ["wasp", "pressure"],
    ],
  );
  assert.deepEqual(
    plan.slice(6, 8).map((x) => [x.player, x.scenario]),
    [
      ["hlsjs", "pressure"],
      ["hlsjs", "playback"],
    ],
  );
  assert.equal(new Set(plan.map((x) => JSON.stringify(x))).size, plan.length);
});

test("default matrix includes known codecs and explicit probing cases without mixing cells", () => {
  const options = parseOptions(["--iterations=1"], "/tmp");
  assert.deepEqual(options.manifests, ["master", "no-codecs"]);
  assert.deepEqual(options.scenarios, ["startup", "reload"]);
  const plan = [...samplePlan(options)];
  assert.equal(plan.length, 24);
  for (const player of options.players)
    for (const format of options.formats)
      for (const scenario of options.scenarios)
        assert.deepEqual(
          plan
            .filter(
              (x) =>
                x.player === player &&
                x.format === format &&
                x.scenario === scenario,
            )
            .map((x) => x.manifest)
            .sort(),
          ["master", "no-codecs"],
        );
});

test("memory checkpoints reject different buffered workloads", () => {
  const snapshot = {
    phase: "steady",
    time: 0,
    bytes: 100,
    breakdown: [],
    currentTime: 16.2,
    buffered: [[0, 19.99]],
  };
  assert.doesNotThrow(() => checkMemoryBuffer(snapshot));
  for (const buffered of [
    [],
    [[0, 22]],
    [[2, 20]],
    [
      [0, 10],
      [12, 20],
    ],
  ])
    assert.throws(() => checkMemoryBuffer({ ...snapshot, buffered }));
});

test("PSS includes children launched on non-main threads, without double counting", async () => {
  const files = {
    "/proc/1/smaps_rollup": "Pss: 10 kB\n",
    "/proc/1/task/1/children": "",
    "/proc/1/task/7/children": "2 2 3",
    "/proc/2/smaps_rollup": "Pss: 20 kB\n",
    "/proc/2/task/2/children": "",
  };
  const fs = {
    async readFile(path) {
      if (!(path in files))
        throw Object.assign(new Error("gone"), { code: "ENOENT" });
      return files[path];
    },
    async readdir(path) {
      return path === "/proc/1/task" ? ["1", "7"] : ["2"];
    },
  };
  assert.equal(await readProcessTreePss(1, fs), 30 * 1024);
});

test("CLI scenario names match implementations without separate pressure/large-playlist algorithms", () => {
  assert.deepEqual(Object.keys(scenarios).sort(), [...scenarioNames].sort());
  assert.equal(
    scenarios.playback.afterStartup,
    scenarios.pressure.afterStartup,
  );
  assert.equal(scenarios.startup.afterStartup, undefined);
  assert.equal(scenarios["large-playlist"].afterStartup, undefined);
});

test("codec omission changes only the declared codecs", () => {
  const master =
    '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.64001F,mp4a.40.2",RESOLUTION=960x540\nvideo.m3u8\n';
  assert.equal(manifestVariant(master, "master"), master);
  assert.equal(
    manifestVariant(master, "no-codecs"),
    master.replace(',CODECS="avc1.64001F,mp4a.40.2"', ""),
  );
  assert.throws(() => manifestVariant(master, "unknown"));
});

test("generated playlist metadata counts actual segments and repeated URLs", () => {
  const stats = describeMediaPlaylist(
    "#EXTM3U\n#EXTINF:2,\na.ts\n#EXT-X-DISCONTINUITY\n#EXTINF:1.5,\na.ts\n#EXT-X-ENDLIST\n",
  );
  assert.equal(stats.segments, 2);
  assert.equal(stats.uniqueUrls, 1);
  assert.equal(stats.discontinuities, 1);
  assert.equal(stats.duration, 3.5);
  assert.throws(() => describeMediaPlaylist("#EXTM3U\n#EXTINF:2,\na.ts"));
});

class FakeVideo extends EventTarget {
  frames = new Map();
  next = 0;
  requestVideoFrameCallback(callback) {
    this.frames.set(++this.next, callback);
    return this.next;
  }
  cancelVideoFrameCallback(id) {
    this.frames.delete(id);
  }
  display() {
    for (const [id, callback] of [...this.frames]) {
      this.frames.delete(id);
      callback(performance.now(), { mediaTime: 0, processingDuration: 0 });
    }
  }
}

test("reload cannot complete from an old frame or old canplay", async () => {
  const video = new FakeVideo();
  const marks = [];
  const scope = new Observations((name) => marks.push(name));
  const load = scope.load(video);
  video.dispatchEvent(new Event("canplay"));
  video.dispatchEvent(new Event("playing"));
  video.display();
  assert.deepEqual(marks, []);
  assert.equal(video.frames.size, 0);
  video.dispatchEvent(new Event("loadedmetadata"));
  assert.equal(video.frames.size, 1);
  video.dispatchEvent(new Event("canplay"));
  video.dispatchEvent(new Event("playing"));
  video.display();
  await Promise.all(Object.values(load));
  assert.deepEqual(marks, ["loadedmetadata", "canplay", "playing", "frame"]);
  scope.dispose();
});

test("disposing removes pending frame callbacks and event observers", () => {
  const video = new FakeVideo();
  const marks = [];
  const scope = new Observations((name) => marks.push(name));
  scope.frame(video);
  scope.event(video, "seeked");
  scope.dispose();
  video.display();
  video.dispatchEvent(new Event("seeked"));
  assert.equal(video.frames.size, 0);
  assert.deepEqual(marks, []);
});

test("shared load observes canplay, not the player load promise, before requesting play", async () => {
  const video = new FakeVideo();
  const calls = [];
  let finishLoad;
  video.play = async () => {
    calls.push("play");
    video.dispatchEvent(new Event("playing"));
    video.display();
    finishLoad();
  };
  const times = await loadContent({
    video,
    player: {
      load() {
        calls.push("load");
        queueMicrotask(() => {
          video.dispatchEvent(new Event("loadedmetadata"));
          calls.push("canplay");
          video.dispatchEvent(new Event("canplay"));
        });
        return new Promise((resolve) => {
          finishLoad = resolve;
        });
      },
    },
    url: "fixture.m3u8",
    mark() {},
    healthy: (promise) => promise,
    fail: (error) => assert.fail(String(error)),
  });
  assert.deepEqual(calls, ["load", "canplay", "play"]);
  assert.ok(
    times.start <= times.metadata &&
      times.metadata <= times.canPlay &&
      times.canPlay <= times.firstFrame,
  );
  assert.equal(video.frames.size, 0);
});
