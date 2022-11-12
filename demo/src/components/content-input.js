import {
  createBreakElement,
  createContainerElt,
  createElement,
} from "../dom-utils";

/**
 * Return input elements to load a new content.
 * @param {Object} player - The WaspHlsPlayer instance on which the content will
 * be loaded
 * @returns {HTMLElement} - The HTMLElement, allowing to display the load input,
 * to add to the DOM.
 */
export default function ContentInputComponent(player) {
  const labelElt = createElement("label", {
      for: "url",
    textContent: "URL to HLS MultiVariant (a.k.a. Master) Playlist:",
  });
  const inputEl = createElement("input", {
    className: "input-url",
    type: "text",
    name: "url",
    value: "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8",
  });
  const loadingButton = createElement("button", {
    id: "loading-button",
    textContent: "Load",
    onclick() {
      player.loadContent(inputEl.value);
    },
  });

  return createContainerElt("div", {
    className: "inputs-container",
  }, [
    labelElt,
    createBreakElement(),
    inputEl,
    loadingButton,
  ]);
}
