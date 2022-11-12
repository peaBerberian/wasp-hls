const titleElt = createElement("h1", {
  className: "title",
  textContent: "wasp-hls player",
});
document.body.appendChild(titleElt);

const playerContainer = createElement("div", {
  className: "player-container",
});
createNewPlayer(playerContainer);
document.body.appendChild(playerContainer);

function createElement(elementName, props) {
  const elt = document.createElement(elementName);
  for (let key in props) {
    elt[key] = props[key];
  }
  return elt;
}

function createNewPlayer(containerElt) {
  const spinnerElt = createElement("div", {
    className: "inline-spinner",
  });
  containerElt.appendChild(spinnerElt);

  const videoElement = createElement("video", {
    className: "video video-small",
    autoplay: true,
    controls: true,
  });

  videoElement.addEventListener("canplay", () => {
    videoElement.play();
  });

  const player = new WaspHlsPlayer(videoElement);
  player.initialize({
    workerUrl: "./worker.js",
    wasmUrl: "./wasp_hls_bg.wasm"
  }).then(() => {
    const inputsBlockElt = createElement("div", {
      className: "inputs-container",
    });
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
        loadUrl(inputEl.value);
      },
    });
    inputsBlockElt.appendChild(labelElt);
    inputsBlockElt.appendChild(createBreakElement());
    inputsBlockElt.appendChild(inputEl);
    inputsBlockElt.appendChild(loadingButton);
    const videoContainerElt = createElement("div", {
      className: "video-container",
    });
    videoContainerElt.appendChild(videoElement);

    containerElt.removeChild(spinnerElt);
    containerElt.appendChild(inputsBlockElt);
    containerElt.appendChild(createBreakElement());
    containerElt.appendChild(videoContainerElt);

    function loadUrl(url) {
      player.loadContent(url);
      // player.loadContent(
      //   "https://cdn.jwplayer.com/manifests/pZxWPRg4.m3u8");
      // player.loadContent(
      //   "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8");
        window.player = player;
    }
  });
}

function createBreakElement() {
  return createElement("br", {});
}

