import { expect, it } from "vitest";
import { createDispatcher } from "../utils/dispatcher_test_tools.js";
import {
  JsTimeRanges,
  MediaSourceReadyState,
} from "../../../build/es6/wasm/js/index.js";

// Scheduling can stop before the usual rebuffering threshold is exceeded.
it.each([
  { goal: 1, shouldResumePlayback: true },
  { goal: 2, shouldResumePlayback: true },
  { goal: 3, shouldResumePlayback: true },
])(
  "handles startup with buffer goal $goal",
  async ({ goal, shouldResumePlayback }) => {
    const ctx = await createDispatcher({ segmentDuration: 2 });
    ctx.dispatcher.set_buffer_goal(goal);
    ctx.dispatcher.load_content("https://example.com/master.m3u8");
    ctx.finish(
      "master.m3u8",
      '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\nlow-video.m3u8\n',
    );
    ctx.finish("low-video.m3u8", ctx.playlist("low-video"));
    ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
    ctx.finish("low-video-0.ts");
    ctx.dispatcher.on_source_buffer_update(
      1,
      new JsTimeRanges(new Float64Array([0, 2])),
    );
    ctx.observe(2);
    ctx.observe(2);

    expect(ctx.rebufferingEvents).toEqual(
      shouldResumePlayback ? ["start", "stop"] : ["start"],
    );
    expect(
      ctx.requests.some((request) => request.url.endsWith("low-video-1.ts")),
    ).toBe(goal >= 2);
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  },
);

function prepareStartup(ctx, name, bufferId) {
  ctx.dispatcher.load_content(`https://example.com/${name}.m3u8`);
  ctx.finish(
    `${name}.m3u8`,
    `#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,CODECS="avc1.42E01E"\n${name}-video.m3u8\n`,
  );
  ctx.finish(`${name}-video.m3u8`, ctx.playlist(`${name}-video`));
  ctx.dispatcher.on_media_source_state_change(MediaSourceReadyState.Open);
  ctx.finish(`${name}-video-0.ts`);
  ctx.dispatcher.on_source_buffer_update(
    bufferId,
    new JsTimeRanges(new Float64Array([0, 2])),
  );
}

it.each([
  { initialGoal: 10, goal: 1, shouldResumePlayback: true },
  { initialGoal: 1, goal: 10, shouldResumePlayback: false },
])(
  "updates the resume threshold when the goal changes from $initialGoal to $goal",
  async ({ initialGoal, goal, shouldResumePlayback }) => {
    const ctx = await createDispatcher({ segmentDuration: 4 });
    try {
      ctx.dispatcher.set_buffer_goal(initialGoal);
      prepareStartup(ctx, "first", 1);
      ctx.dispatcher.set_buffer_goal(goal);
      ctx.observe(2);
      ctx.observe(2);
      expect(ctx.rebufferingEvents).toEqual(
        shouldResumePlayback ? ["start", "stop"] : ["start"],
      );
    } finally {
      ctx.dispatcher.stop();
      ctx.dispatcher.free();
    }
  },
);

it("retains the buffer goal across content reloads", async () => {
  const ctx = await createDispatcher({ segmentDuration: 4 });
  try {
    ctx.dispatcher.set_buffer_goal(1);
    prepareStartup(ctx, "first", 1);
    ctx.observe(1);
    ctx.observe(1);
    expect(ctx.rebufferingEvents).toEqual(["start", "stop"]);
    prepareStartup(ctx, "second", 2);
    ctx.observe(1);
    ctx.observe(1);
    expect(ctx.rebufferingEvents).toEqual(["start", "stop", "start", "stop"]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});

it("requires playable data even with a zero buffer goal", async () => {
  const ctx = await createDispatcher({ segmentDuration: 4 });
  try {
    prepareStartup(ctx, "first", 1);
    ctx.dispatcher.set_buffer_goal(0);
    ctx.observe(0);
    ctx.observe(0);
    expect(ctx.rebufferingEvents).toEqual(["start"]);
  } finally {
    ctx.dispatcher.stop();
    ctx.dispatcher.free();
  }
});
