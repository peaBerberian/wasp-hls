import { describe, expect, it } from "vitest";
import {
  eventListener,
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createPlayerHarness,
  createTopLevelFetchRule,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Audio track changes requiring a variant change", function () {
  it.each(["upgrade", "downgrade", "unchanged", "locked"])(
    "requests only the final audio rendition during a compatible track change (%s)",
    { timeout: TEST_TIMEOUT_MS },
    async (mode) => {
      const baseUrl = new URL(".", getVodScenarioUrl("fmp4-alt-audio"));
      const currentAudioUrl = `${baseUrl}audio-fr.m3u8?rendition=current`;
      const alternativeAudioUrl = `${baseUrl}audio-fr.m3u8?rendition=alternative`;
      const alternativeVideoUrl = `${baseUrl}video.m3u8?rendition=alternative`;
      const currentBandwidth = mode === "downgrade" ? 4_000_000_000 : 1900000;
      const alternativeBandwidth =
        mode === "unchanged"
          ? 4_000_000_000
          : mode === "downgrade"
            ? 2000000
            : 4000000;
      const variantChanges = mode === "upgrade" || mode === "downgrade";
      // Partial scores keep this scheduling test on throughput selection.
      const currentScore = mode === "downgrade" ? ",SCORE=1" : "";
      const alternativeScore = mode === "downgrade" ? "" : ",SCORE=1";
      const ctx = await createPlayerHarness({
        initialBandwidth: 100_000_000,
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              body: `#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="current",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="${baseUrl}audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="current",NAME="French",LANGUAGE="fr",URI="${currentAudioUrl}"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="alternative",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="${alternativeAudioUrl}"
#EXT-X-STREAM-INF:BANDWIDTH=${currentBandwidth}${currentScore},AUDIO="current"
${baseUrl}video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=${alternativeBandwidth}${alternativeScore},AUDIO="alternative"
${alternativeVideoUrl}
`,
            },
          ]),
          {
            id: "intermediate-audio",
            match: { urlEndsWith: "audio-fr.m3u8?rendition=current" },
            actions: variantChanges
              ? [{ type: "response", status: 200, body: "invalid playlist" }]
              : [{ type: "passthrough" }],
          },
          {
            id: "bounded-download-rate",
            match: { urlIncludes: "/vod/generated/" },
            actions: [{ type: "passthrough", delayMs: 20 }],
          },
        ],
      });

      try {
        // Only the current variant provides English; French adds another ABR candidate.
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"), {
          initialAudioTrack: { language: "en" },
        });
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const initialVariant = ctx.player.getCurrentVariant();
        expect(initialVariant.bandwidth).toBe(currentBandwidth);
        if (mode === "locked") {
          const locked = waitForPlayerEvent(ctx.player, "variantLockUpdate");
          ctx.player.lockVariant(initialVariant.id);
          await locked;
        }

        const frenchTrack = ctx.player
          .getAudioTrackList()
          .find((track) => track.language === "fr");
        const events = [];
        ctx.player.addEventListener("audioTrackUpdate", () => {
          events.push("audioTrackUpdate");
          expect(ctx.player.getCurrentAudioTrack()?.id).toBe(frenchTrack.id);
          expect(ctx.player.getCurrentVariant()?.id).toBe(initialVariant.id);
        });
        ctx.player.addEventListener("variantUpdate", () => {
          events.push("variantUpdate");
          expect(ctx.player.getCurrentAudioTrack()?.id).toBe(frenchTrack.id);
        });
        const warnings = eventListener(ctx.player, "warning");
        const lockUpdates = eventListener(ctx.player, "variantLockUpdate");
        const updated = waitForPlayerEvent(
          ctx.player,
          variantChanges ? "variantUpdate" : "audioTrackUpdate",
        );
        ctx.player.setAudioTrack(frenchTrack.id);
        await updated;

        const finalAudioUrl = variantChanges
          ? alternativeAudioUrl
          : currentAudioUrl;
        const unusedAudioUrl = variantChanges
          ? currentAudioUrl
          : alternativeAudioUrl;
        await ctx.workerHandle.telemetry.waitFor(
          (event) =>
            event.type === "fetch-start" && event.url === finalAudioUrl,
        );
        expect(
          ctx.workerHandle.telemetry
            .getEvents()
            .filter(
              (event) =>
                event.type === "fetch-start" && event.url === unusedAudioUrl,
            ),
        ).toHaveLength(0);

        const initialTime = ctx.videoElement.currentTime;
        const progressed = new Promise((resolve) => {
          ctx.videoElement.addEventListener(
            "timeupdate",
            function onTimeUpdate() {
              if (ctx.videoElement.currentTime > initialTime + 0.25) {
                ctx.videoElement.removeEventListener(
                  "timeupdate",
                  onTimeUpdate,
                );
                resolve();
              }
            },
          );
        });
        await ctx.player.resume();
        await progressed;

        expect(events).toEqual(
          variantChanges
            ? ["audioTrackUpdate", "variantUpdate"]
            : ["audioTrackUpdate"],
        );
        expect(lockUpdates.getCurrentCount()).toBe(0);
        expect(warnings.getCurrentCount()).toBe(0);
        expect(ctx.player.getLockedVariant()).toEqual(
          mode === "locked" ? initialVariant : null,
        );
        expect(ctx.player.getCurrentVariant().bandwidth).toBe(
          variantChanges ? alternativeBandwidth : currentBandwidth,
        );
        expect(ctx.player.getCurrentAudioTrack().id).toBe(frenchTrack.id);
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        expect(ctx.getLastPlayerError()).toBeNull();
        const requests = ctx.workerHandle.telemetry
          .getEvents()
          .filter((event) => event.type === "fetch-start");
        expect(
          requests.filter((event) => event.url === finalAudioUrl),
        ).toHaveLength(1);
        expect(
          requests.filter((event) => event.url === unusedAudioUrl),
        ).toHaveLength(0);
        expect(
          requests.filter((event) => event.url === alternativeVideoUrl),
        ).toHaveLength(variantChanges ? 1 : 0);
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "rejects tracks without a variant asynchronously and preserves playback and locking",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const baseUrl = new URL(".", getVodScenarioUrl("fmp4-alt-audio"));
      const ctx = await createPlayerHarness({
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              body: `#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="${baseUrl}audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="unused",NAME="French",LANGUAGE="fr",URI="unused.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1900000,AUDIO="en"
${baseUrl}video.m3u8
`,
            },
          ]),
        ],
      });

      try {
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"), {
          initialAudioTrack: { language: "en" },
        });
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const initialVariant = ctx.player.getCurrentVariant();
        const initialTrack = ctx.player.getCurrentAudioTrack();
        const locked = waitForPlayerEvent(ctx.player, "variantLockUpdate");
        ctx.player.lockVariant(initialVariant.id);
        await locked;

        const frenchTrack = ctx.player
          .getAudioTrackList()
          .find((track) => track.language === "fr");
        const warnings = eventListener(ctx.player, "warning");
        const trackUpdates = eventListener(ctx.player, "audioTrackUpdate");
        const variantUpdates = eventListener(ctx.player, "variantUpdate");
        const lockUpdates = eventListener(ctx.player, "variantLockUpdate");

        // The following warning-producing track request acts as a worker-ordering barrier for
        // this repeated lock request.
        ctx.player.lockVariant(initialVariant.id);
        for (const trackId of [frenchTrack.id, frenchTrack.id, 0xfffffffe]) {
          const previousWarningCount = warnings.getCurrentCount();
          const warningPromise = warnings.awaitNext();
          expect(ctx.player.setAudioTrack(trackId)).toBeUndefined();
          expect(warnings.getCurrentCount()).toBe(previousWarningCount);
          const warning = await warningPromise;
          expect(warning.name).toBe("WaspOtherError");
          expect(warning.code).toBe("Unknown");
          expect(ctx.player.getCurrentAudioTrack()).toEqual(initialTrack);
          expect(ctx.player.getCurrentVariant()).toEqual(initialVariant);
          expect(ctx.player.getLockedVariant()).toEqual(initialVariant);
        }

        const initialTime = ctx.videoElement.currentTime;
        const progressed = new Promise((resolve) => {
          ctx.videoElement.addEventListener(
            "timeupdate",
            function onTimeUpdate() {
              if (ctx.videoElement.currentTime > initialTime + 0.25) {
                ctx.videoElement.removeEventListener(
                  "timeupdate",
                  onTimeUpdate,
                );
                resolve();
              }
            },
          );
        });
        await ctx.player.resume();
        await progressed;

        expect(warnings.getCurrentCount()).toBe(3);
        expect(trackUpdates.getCurrentCount()).toBe(0);
        expect(variantUpdates.getCurrentCount()).toBe(0);
        expect(lockUpdates.getCurrentCount()).toBe(0);
        expect(ctx.player.getPlayerState()).toBe("Loaded");
        expect(ctx.player.getError()).toBeNull();
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "announces the track, variant, then automatic unlocking in that order",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const baseUrl = new URL(".", getVodScenarioUrl("fmp4-alt-audio"));
      const ctx = await createPlayerHarness({
        fetchRules: [
          createTopLevelFetchRule([
            {
              type: "response",
              status: 200,
              body: `#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="en",NAME="English",LANGUAGE="en",DEFAULT=YES,URI="${baseUrl}audio-en.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="fr",NAME="French",LANGUAGE="fr",DEFAULT=YES,URI="${baseUrl}audio-fr.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1900000,AUDIO="en"
${baseUrl}video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000000,AUDIO="fr"
${baseUrl}video.m3u8
`,
            },
          ]),
        ],
      });

      try {
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"), {
          initialAudioTrack: { language: "en" },
        });
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const initialVariant = ctx.player.getCurrentVariant();
        const locked = waitForPlayerEvent(ctx.player, "variantLockUpdate");
        ctx.player.lockVariant(initialVariant.id);
        await locked;

        const frenchTrack = ctx.player
          .getAudioTrackList()
          .find((track) => track.language === "fr");
        const events = [];
        ctx.player.addEventListener("audioTrackUpdate", () => {
          events.push("audioTrackUpdate");
          expect(ctx.player.getCurrentAudioTrack()?.id).toBe(frenchTrack.id);
          expect(ctx.player.getCurrentVariant()?.id).toBe(initialVariant.id);
        });
        ctx.player.addEventListener("variantUpdate", () => {
          events.push("variantUpdate");
          expect(ctx.player.getCurrentAudioTrack()?.id).toBe(frenchTrack.id);
        });
        ctx.player.addEventListener("variantLockUpdate", () => {
          events.push("variantLockUpdate");
        });

        const unlocked = waitForPlayerEvent(
          ctx.player,
          "variantLockUpdate",
          (variant) => variant === null,
        );
        ctx.player.setAudioTrack(frenchTrack.id);
        await unlocked;
        expect(events).toEqual([
          "audioTrackUpdate",
          "variantUpdate",
          "variantLockUpdate",
        ]);
      } finally {
        ctx.dispose();
      }
    },
  );
});
