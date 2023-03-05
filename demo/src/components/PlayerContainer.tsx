import * as React from "react";
import WaspHlsPlayer from "../../../src";
import ContentInput from "./ContentInput";
import RemovePlayerButton from "./RemovePlayerButton";
import VideoPlayer from "./VideoPlayer";

export default React.memo(function PlayerContainer(
  {
    onClose,
  } : {
    onClose : () => void;
  }
) {
  const [player, setPlayer] = React.useState<WaspHlsPlayer|null>(null);

  React.useEffect(() => {
    let isRemoved = false;
    const videoElt = document.createElement("video");
    videoElt.className = "video video-small";
    videoElt.autoplay = true;
    const waspHlsPlayer = new WaspHlsPlayer(videoElt);
    /* eslint-disable-next-line */
    (window as any).player = waspHlsPlayer;
    waspHlsPlayer.initialize({
      workerUrl: "./worker.js",
      wasmUrl: "./wasp_hls_bg.wasm",
    }).then(() => {
      if (isRemoved) {
        waspHlsPlayer.dispose();
        return;
      }
      setPlayer(waspHlsPlayer);
    }, (err) => {
      console.error("Could not initialize WaspHlsPlayer:", err);
    });

    return () => {
      isRemoved = true;
      waspHlsPlayer.dispose();
      /* eslint-disable-next-line */
      if ((window as any).player === waspHlsPlayer) {
        /* eslint-disable-next-line */
        (window as any).player = null;
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
                <VideoPlayer player={player} />
                <RemovePlayerButton onClick={onClose} />
              </>
            )
        }
      </div>
    </div>
  );
});
