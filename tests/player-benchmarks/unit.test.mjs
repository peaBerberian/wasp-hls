import test from "node:test";
import assert from "node:assert/strict";
import { parseOptions, samplePlan } from "./config.mjs";
import { manifestVariant, describeMediaPlaylist } from "./fixtures.mjs";
import { Observations } from "./src/observations.ts";

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
