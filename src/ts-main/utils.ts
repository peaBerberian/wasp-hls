import logger from "../ts-common/logger";
import { MainMessageType } from "../ts-common/types";
import postMessageToWorker from "./postMessageToWorker";
import { ContentMetadata } from "./types";

const DEFAULT_MPEG2_TS_TYPE = 'video/mp2t;codecs="avc1.4D401F"';

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

export function requestStopForContent(
  metadata: ContentMetadata,
  worker: Worker | null
): void {
  // Preventively free some resource that should not impact the Worker much.
  metadata.stopPlaybackObservations?.();
  metadata.loadingAborter?.abort();

  if (worker !== null) {
    postMessageToWorker(worker, {
      type: MainMessageType.StopContent,
      value: { contentId: metadata.contentId },
    });
  }
}

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
