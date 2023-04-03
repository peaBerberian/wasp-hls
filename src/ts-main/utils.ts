import logger from "../ts-common/logger";
import { MainMessageType } from "../ts-common/types";
import postMessageToWorker from "./postMessageToWorker";
import { ContentMetadata } from "./types";

/**
 * Default mime-type used for checking support of mpeg2-ts media.
 */
const DEFAULT_MPEG2_TS_TYPE = 'video/mp2t;codecs="avc1.4D401F"';

/**
 * If `true`, we're on the Firefox web browser.
 */
const isFirefox = navigator.userAgent.toLowerCase().indexOf("firefox") !== -1;

/**
 * Returns `true` if the current environment can support mpeg2-ts contents.
 * @returns {boolean}
 */
export function canDemuxMpeg2Ts(): boolean {
  if (isFirefox) {
    // Very sadly, Firefox seems to pollute te console with a warning when
    // testing unsupported mime-types this way.
    //
    // As a library, we don't like having side-effect in the console because it
    // may annoy applications' developers.
    // Because of this, it is penalized, no MPEG-TS for you, nah! This should
    // be the case in most (all?) cases anyways.
    return false;
  }
  return (
    typeof MediaSource === "function" &&
    MediaSource.isTypeSupported(DEFAULT_MPEG2_TS_TYPE)
  );
}

/**
 * Transforms received Error, in an unknown format, into an object with an
 * optional `name` string (if found on the Error) and a `message` string who is
 * either set to the Error's message or to `defaultMsg` if no such message was
 * found.
 * @param {*} err
 * @param {string} defaultMsg
 * @returns {Object}
 */
export function getErrorInformation(
  err: unknown,
  defaultMsg: string
): {
  name: string | undefined;
  message: string;
} {
  if (err instanceof Error) {
    return { message: err.message, name: err.name };
  } else {
    return { message: defaultMsg, name: undefined };
  }
}

/**
 * Stop content playback thanks to the given `ContentMetadata` object and
 * indicate to the WebWorker that it should also stop playing the content
 * on its side.
 * @param {Object} metadata
 * @param {Worker|null} worker
 */
export function requestStopForContent(
  metadata: ContentMetadata,
  worker: Worker | null
): void {
  // Preventively free some resource that should not impact the Worker much.
  if (metadata.playbackObserver !== null) {
    metadata.playbackObserver.stop();
    metadata.playbackObserver = null;
  }
  if (metadata.loadingAborter !== undefined) {
    metadata.loadingAborter.abort();
    metadata.loadingAborter = undefined;
  }

  if (worker !== null) {
    postMessageToWorker(worker, {
      type: MainMessageType.StopContent,
      value: { contentId: metadata.contentId },
    });
  }
}

/**
 * Observe the given `HTMLMediaElement` and returns a Promise which:
 *   - resolves when the `HTMLMediaElement`'s `"canplay"` event is sent.
 *   - reject if the given `AbortSignal` emits before the `"canplay"` event
 *     has been received.
 * @param {HTMLMediaElement} videoElement
 * @param {AbortSignal} abortSignal
 * @returns {Promise}
 */
export function waitForLoad(
  videoElement: HTMLMediaElement,
  abortSignal: AbortSignal
): Promise<void> {
  return new Promise<void>((res, rej) => {
    if (videoElement.readyState >= HTMLMediaElement.HAVE_ENOUGH_DATA) {
      res();
    }
    abortSignal.addEventListener("abort", onAbort);
    videoElement.addEventListener("canplay", onCanPlay);
    function onCanPlay() {
      videoElement.removeEventListener("canplay", onCanPlay);
      abortSignal.removeEventListener("abort", onAbort);
      res();
    }
    function onAbort() {
      videoElement.removeEventListener("canplay", onCanPlay);
      abortSignal.removeEventListener("abort", onAbort);

      // Typing needed because of a weird TypeScript issue
      if ((abortSignal as unknown as { reason: unknown }).reason !== null) {
        rej((abortSignal as unknown as { reason: unknown }).reason);
      } else {
        rej(new Error("The loading operation was aborted"));
      }
    }
  });
}
/**
 * Clear element's src attribute.
 * @param {HTMLMediaElement} element
 */
export function clearElementSrc(element: HTMLMediaElement): void {
  // On some browsers, we first have to make sure the textTracks elements are
  // both disabled and removed from the DOM.
  // If we do not do that, we may be left with displayed text tracks on the
  // screen, even if the track elements are properly removed, due to browser
  // issues.
  // Bug seen on Firefox (I forgot which version) and Chrome 96.
  const { textTracks } = element;
  if (textTracks != null) {
    for (let i = 0; i < textTracks.length; i++) {
      textTracks[i].mode = "disabled";
    }
    if (element.hasChildNodes()) {
      const { childNodes } = element;
      for (let j = childNodes.length - 1; j >= 0; j--) {
        if (childNodes[j].nodeName === "track") {
          try {
            element.removeChild(childNodes[j]);
          } catch (err) {
            const error =
              err instanceof Error ? err.toString() : "Unknown Error";
            logger.warn(
              "Unable to remove track element from media element",
              error
            );
          }
        }
      }
    }
  }
  element.src = "";

  // On IE11, element.src = "" is not sufficient as it
  // does not clear properly the current MediaKey Session.
  // Microsoft recommended to use element.removeAttr("src").
  element.removeAttribute("src");
}

/**
 * Transform a URL which is potentially relative to the current script to
 * an absolute URL.
 * @param {string} url
 * @returns {string}
 */
export function potentiallyRelativeUrlToAbsoluteUrl(url: string): string {
  return new URL(url, location.href).toString();
}
