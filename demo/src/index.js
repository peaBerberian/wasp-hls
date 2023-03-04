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
const createNewPlayerButtonElt = createElement("button", {
  className: "create-new-player",
  textContent: "Create new Player",
  onclick() {
    createNewPlayer(playerContainer);
  },
});
addInnerElements(document.body, [
  titleElt,
  playerContainer,
  createNewPlayerButtonElt,
]);
createNewPlayer(playerContainer);

function createNewPlayer(containerElt) {
  let isRemoved = false;
  const spinnerElt = createElement("div", {
    className: "inline-spinner",
  });

  const videoElt = createElement("video", {
    className: "video video-small",
    autoplay: true,
    controls: true,
  });
  const playerParentElt = createElement("div", {
    className: "player-parent",
  });

  const removePlayerButtonElt = createElement("button", {
    className: "remove-player",
    textContent: "Remove Player",
    onclick() {
      isRemoved = true;
      if (containerElt.contains(spinnerElt)) {
        containerElt.removeChild(spinnerElt);
      }
      if (containerElt.contains(playerParentElt)) {
        containerElt.removeChild(playerParentElt);
      }
      if (containerElt.contains(removePlayerButtonElt)) {
        containerElt.removeChild(removePlayerButtonElt);
      }
    },
  });

  addInnerElements(containerElt, [removePlayerButtonElt, spinnerElt]);
  const player = new WaspHlsPlayer(videoElt);
  window.player = player;
  player.initialize({
    workerUrl: "./worker.js",
    wasmUrl: "./wasp_hls_bg.wasm"
  }).then(() => {
    if (isRemoved) {
      return;
    }
    containerElt.removeChild(spinnerElt);
    containerElt.removeChild(removePlayerButtonElt);
    addInnerElements(playerParentElt, [
      ContentInputComponent(player),
      createBreakElement(),
      VideoPlayerComponent(videoElt),
      removePlayerButtonElt,
    ]);
    addInnerElements(playerContainer, [playerParentElt]);
  });
}
