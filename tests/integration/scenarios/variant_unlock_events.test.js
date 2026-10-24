import { describe, expect, it } from "vitest";
import {
  eventListener,
  waitForLoadedState,
  waitForPlayerEvent,
} from "../../utils/player_test_tools.js";
import { getVodScenarioUrl } from "../../utils/vod_scenarios.js";
import {
  createPlayerHarness,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Explicit variant unlocking", function () {
  it(
    "acknowledges unlocking even when the current variant stays unchanged",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      // This is a multivariant playlist with one variant: unlocking must pass
      // through the worker, but cannot cause an adaptive variant change.
      const ctx = await createPlayerHarness({ fetchRules: [] });
      try {
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"));
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const variant = ctx.player.getCurrentVariant();
        expect(variant).toBeDefined();
        expect(ctx.player.getVariantList()).toHaveLength(1);
        const lockUpdates = eventListener(ctx.player, "variantLockUpdate");
        const variantUpdates = eventListener(ctx.player, "variantUpdate");
        ctx.player.addEventListener("variantLockUpdate", (lockedVariant) => {
          expect(ctx.player.getLockedVariant()).toEqual(lockedVariant);
        });

        for (let cycle = 0; cycle < 2; cycle++) {
          const locked = waitForPlayerEvent(
            ctx.player,
            "variantLockUpdate",
            (value) => value?.id === variant.id,
          );
          ctx.player.lockVariant(variant.id);
          expect(await locked).toEqual(variant);
          expect(ctx.player.getLockedVariant()).toEqual(variant);

          const unlocked = waitForPlayerEvent(
            ctx.player,
            "variantLockUpdate",
            (value) => value === null,
          );
          ctx.player.unlockVariant();
          expect(await unlocked).toBeNull();
          expect(ctx.player.getLockedVariant()).toBeNull();
          expect(ctx.player.getCurrentVariant()).toEqual(variant);

          // The next lock acknowledgment also confirms this redundant unlock
          // was processed, without relying on a sleep to check event counts.
          ctx.player.unlockVariant();
        }

        const relocked = waitForPlayerEvent(
          ctx.player,
          "variantLockUpdate",
          (value) => value?.id === variant.id,
        );
        ctx.player.lockVariant(variant.id);
        await relocked;
        expect(lockUpdates.getCurrentCount()).toBe(5);
        for (let index = 0; index < 5; index++) {
          expect(lockUpdates.getPayloadFor(index)).toEqual(
            index % 2 === 0 ? variant : null,
          );
        }
        expect(variantUpdates.getCurrentCount()).toBe(0);
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );

  it(
    "preserves an existing lock after rejecting an unknown variant",
    { timeout: TEST_TIMEOUT_MS },
    async () => {
      const ctx = await createPlayerHarness({ fetchRules: [] });
      try {
        ctx.player.load(getVodScenarioUrl("fmp4-multivariant-no-codecs"));
        await waitForLoadedState(
          ctx.player,
          ctx.videoElement,
          ctx.getLastPlayerError,
        );
        const variant = ctx.player.getCurrentVariant();
        const lockUpdates = eventListener(ctx.player, "variantLockUpdate");
        const locked = waitForPlayerEvent(ctx.player, "variantLockUpdate");
        ctx.player.lockVariant(variant.id);
        await locked;
        const warning = waitForPlayerEvent(ctx.player, "warning");
        ctx.player.lockVariant(0xfffffffe);
        expect((await warning).code).toBe("UnfoundLockedVariant");
        expect(ctx.player.getLockedVariant()).toEqual(variant);
        expect(lockUpdates.getCurrentCount()).toBe(1);

        // The acknowledgment proves that the worker kept its lock, not just the public metadata.
        const unlocked = waitForPlayerEvent(
          ctx.player,
          "variantLockUpdate",
          (value) => value === null,
        );
        ctx.player.unlockVariant();
        await unlocked;
        expect(lockUpdates.getCurrentCount()).toBe(2);
        expect(ctx.player.getLockedVariant()).toBeNull();
        expect(ctx.getLastPlayerError()).toBeNull();
      } finally {
        ctx.dispose();
      }
    },
  );
});
