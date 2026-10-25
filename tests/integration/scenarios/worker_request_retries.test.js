import { describe, it } from "vitest";
import {
  createMediaPlaylistFetchRule,
  createSegmentFetchRule,
  createTopLevelFetchRule,
  expectFatalRequestError,
  expectLoadAfterSingleRetry,
  expectMixedRetryFailure,
  expectRetryBackoffRangeSequence,
  MEDIA_PLAYLIST_CONFIG,
  MULTIVARIANT_CONFIG,
  SEGMENT_CONFIG,
  TEST_TIMEOUT_MS,
} from "../utils/request_integration_test_tools.js";

describe("Worker request injection - retries", function () {
  describe("successful recovery after a retry", function () {
    it(
      "applies initial request config to retry a transient top-level failure",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectLoadAfterSingleRetry({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: MULTIVARIANT_CONFIG,
          fetchRules: [
            createTopLevelFetchRule([
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "master-playlist",
          expectedWarningCodes: ["MultivariantPlaylistBadHttpStatus"],
        });
      },
    );

    it(
      "applies updateConfig to retry a transient media-playlist failure",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectLoadAfterSingleRetry({
          scenarioId: "fmp4-multivariant-no-codecs",
          fetchRules: [
            createMediaPlaylistFetchRule([
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "media-playlist",
          configUpdate: MEDIA_PLAYLIST_CONFIG,
          expectedWarningCodes: ["MediaPlaylistBadHttpStatus"],
        });
      },
    );

    it(
      "applies updateConfig to retry a transient segment failure",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectLoadAfterSingleRetry({
          scenarioId: "fmp4-direct-media",
          fetchRules: [
            createSegmentFetchRule([
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "segment-request",
          configUpdate: SEGMENT_CONFIG,
          expectedWarningCodes: ["SegmentBadHttpStatus"],
        });
      },
    );
  });

  describe("configured backoff", function () {
    it(
      "uses configured multivariant backoff base and max values",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectRetryBackoffRangeSequence({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: {
            multiVariantPlaylistMaxRetry: 3,
            multiVariantPlaylistBackoffBase: 500,
            multiVariantPlaylistBackoffMax: 1200,
          },
          fetchRules: [
            createTopLevelFetchRule([
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "master-playlist",
          expectedNominalDelaysMs: [500, 1000, 1200],
        });
      },
    );

    it(
      "uses configured media-playlist backoff base and max values from updateConfig",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectRetryBackoffRangeSequence({
          scenarioId: "fmp4-multivariant-no-codecs",
          fetchRules: [
            createMediaPlaylistFetchRule([
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "media-playlist",
          configUpdate: {
            mediaPlaylistMaxRetry: 3,
            mediaPlaylistBackoffBase: 500,
            mediaPlaylistBackoffMax: 1200,
          },
          expectedNominalDelaysMs: [500, 1000, 1200],
        });
      },
    );

    it(
      "uses configured segment backoff base and max values from updateConfig",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectRetryBackoffRangeSequence({
          scenarioId: "fmp4-direct-media",
          fetchRules: [
            createSegmentFetchRule([
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "response", status: 404 },
              { type: "passthrough" },
            ]),
          ],
          ruleId: "segment-request",
          configUpdate: {
            segmentMaxRetry: 3,
            segmentBackoffBase: 500,
            segmentBackoffMax: 1200,
          },
          expectedNominalDelaysMs: [500, 1000, 1200],
        });
      },
    );
  });

  describe("retry conditions", function () {
    it(
      "retries 412 and 500 responses but stops on a later network error",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectMixedRetryFailure({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: {
            multiVariantPlaylistMaxRetry: 2,
            multiVariantPlaylistBackoffBase: 1,
            multiVariantPlaylistBackoffMax: 1,
          },
          fetchRules: [
            createTopLevelFetchRule([
              { type: "response", status: 412 },
              { type: "response", status: 500 },
              { type: "error", message: "Injected top-level failure" },
            ]),
          ],
          ruleId: "master-playlist",
          expectedWarningCodes: [
            "MultivariantPlaylistBadHttpStatus",
            "MultivariantPlaylistBadHttpStatus",
          ],
          expectedFinalErrorCode: "MultivariantPlaylistRequestError",
          expectedAttempts: 3,
        });
      },
    );

    it(
      "does not retry non-retriable 400 responses",
      { timeout: TEST_TIMEOUT_MS },
      async () => {
        await expectFatalRequestError({
          scenarioId: "fmp4-multivariant-no-codecs",
          playerConfig: {
            multiVariantPlaylistMaxRetry: 3,
            multiVariantPlaylistBackoffBase: 1,
            multiVariantPlaylistBackoffMax: 1,
          },
          fetchRules: [
            createTopLevelFetchRule([{ type: "response", status: 400 }]),
          ],
          ruleId: "master-playlist",
          expectedErrorName: "WaspMultivariantPlaylistRequestError",
          expectedErrorCode: "MultivariantPlaylistRequestError",
          expectedAttempts: 1,
          expectedWarnings: 0,
        });
      },
    );
  });
});
