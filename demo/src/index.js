import ContentInputComponent from "./components/content-input";
import VideoPlayerComponent from "./components/video-player";
import {
  createBreakElement,
  createElement,
  addInnerElements,
} from "./dom-utils";

const titleElt = createElement("h1", {
  className: "title",
  textContent: "wasp-hls player",
});
const playerContainer = createElement("div", {
  className: "player-container",
});
addInnerElements(document.body, [titleElt, playerContainer]);
createNewPlayer(playerContainer);

function createNewPlayer(containerElt) {
  const spinnerElt = createElement("div", {
    className: "inline-spinner",
  });
  containerElt.appendChild(spinnerElt);

  const videoElt = createElement("video", {
    className: "video video-small",
    autoplay: true,
    controls: true,
  });

  const player = new WaspHlsPlayer(videoElt);
  window.player = player;
  player.initialize({
    workerUrl: "./worker.js",
    wasmUrl: "./wasp_hls_bg.wasm"
  }).then(() => {
    containerElt.removeChild(spinnerElt);
    addInnerElements(containerElt, [
      ContentInputComponent(player),
      createBreakElement(),
      VideoPlayerComponent(videoElt),
    ]);
  });
}
