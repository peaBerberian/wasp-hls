import React, { useEffect, useState } from "react";
import ContentInput from "./ContentInput";
import RemovePlayerButton from "./RemovePlayerButton";
import VideoPlayer from "./VideoPlayer";

const win = window as any;

export default function PlayerContainer(
  {
    onClose,
  } : {
    onClose : () => void;
  }
) {
  const [player, setPlayer] = useState(null);

  useEffect(() => {
    let isRemoved = false;
    const videoElt = document.createElement("video");
    videoElt.className = "video video-small";
    videoElt.autoplay = true;
    videoElt.controls = true;
    const player = new win.WaspHlsPlayer(videoElt);
    win.player = player;
    player.initialize({
      workerUrl: "./worker.js",
      wasmUrl: "./wasp_hls_bg.wasm"
    }).then(() => {
      if (isRemoved) {
        player.dispose();
        return;
      }
     setPlayer(player);
    });

    return () => {
      isRemoved = true;
      player.dispose();
      if (win.player === player) {
        win.player = null;
      }
    };
  }, []);

  return (
    <div className="player-container">
      <div className="player-parent">
        {
          player === null ?
            (
              <>
                <RemovePlayerButton onClick={onClose} />
                <div className="inline-spinner" />
              </>
            ) :
            (
              <>
                <ContentInput player={player} />
                <br />
                <VideoPlayer videoElt={player.videoElement} />
                <RemovePlayerButton onClick={onClose} />
              </>
            )
        }
      </div>
    </div>
  );
}
