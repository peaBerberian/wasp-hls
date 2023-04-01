import * as React from "react";
import WaspHlsPlayer, { PlayerState } from "../../../src";
import BufferSizeChart from "./BufferSizeChart";
import ContentInput from "./ContentInput";
import BufferContentGraph from "./MediaBufferContentGraph";
import RemovePlayerButton from "./RemovePlayerButton";
import VideoPlayer from "./VideoPlayer";

export default React.memo(function PlayerContainer({
  onClose,
}: {
  onClose: () => void;
}) {
  const checkBoxesId = React.useId();
  const [player, setPlayer] = React.useState<WaspHlsPlayer | null>(null);
  const [shouldShowBufferGaps, setShouldShowBufferGaps] = React.useState(false);
  const [shouldShowBufferContent, setShouldShowBufferContent] =
    React.useState(true);
  const [bufferGaps, setBufferGaps] = React.useState<
    Array<{
      date: number;
      value: number;
    }>
  >([]);

  React.useEffect(() => {
    let isRemoved = false;
    const videoElt = document.createElement("video");
    videoElt.className = "video video-small";
    const waspHlsPlayer = new WaspHlsPlayer(videoElt);

    /* eslint-disable-next-line */
    (window as any).player = waspHlsPlayer;
    waspHlsPlayer
      .initialize({
        workerUrl: "./worker.js",
        wasmUrl: "./wasp_hls_bg.wasm",
      })
      .then(
        () => {
          if (isRemoved) {
            waspHlsPlayer.dispose();
            return;
          }
          setPlayer(waspHlsPlayer);
        },
        (err) => {
          console.error("Could not initialize WaspHlsPlayer:", err);
        }
      );

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

  React.useEffect(() => {
    if (player === null || !shouldShowBufferGaps) {
      return;
    }
    let bufferGapIntervalId: number | undefined;

    if (
      player.getPlayerState() === PlayerState.Loaded ||
      player.getPlayerState() === PlayerState.Loading
    ) {
      startBufferGapInterval();
    }
    player.addEventListener("playerStateChange", onPlayerStateChange);

    return () => {
      stopBufferGapInterval();
      player.removeEventListener("playerStateChange", onPlayerStateChange);
    };

    function startBufferGapInterval() {
      if (bufferGapIntervalId !== undefined) {
        return;
      }
      bufferGapIntervalId = setInterval(() => {
        if (player === null) {
          stopBufferGapInterval();
          return;
        }
        const buffered = player.videoElement.buffered;
        const currentTime = player.videoElement.currentTime;
        let bufferGap = 0;
        for (let i = 0; i < buffered.length; i++) {
          if (
            buffered.start(i) <= currentTime &&
            buffered.end(i) > currentTime
          ) {
            bufferGap = buffered.end(i) - currentTime;
            break;
          }
        }
        setBufferGaps((oldBufferGaps) => {
          const newVal = { date: performance.now(), value: bufferGap };
          if (oldBufferGaps.length >= 500) {
            return [...oldBufferGaps.slice(1), newVal];
          }
          return [...oldBufferGaps, newVal];
        });
      }, 150);
    }
    function stopBufferGapInterval() {
      if (bufferGapIntervalId !== undefined) {
        clearInterval(bufferGapIntervalId);
        bufferGapIntervalId = undefined;
        setBufferGaps([]);
      }
    }
    function onPlayerStateChange(playerState: PlayerState): void {
      if (
        playerState === PlayerState.Stopped ||
        playerState === PlayerState.Error
      ) {
        stopBufferGapInterval();
      } else {
        startBufferGapInterval();
      }
    }
  }, [player, shouldShowBufferGaps]);

  const onBufferSizeCheckBoxChange = React.useCallback(
    (evt: React.ChangeEvent<HTMLInputElement>) => {
      setShouldShowBufferGaps(evt.target.checked);
    },
    []
  );

  const onBufferContentCheckBoxChange = React.useCallback(
    (evt: React.ChangeEvent<HTMLInputElement>) => {
      setShouldShowBufferContent(evt.target.checked);
    },
    []
  );

  return (
    <div className="player-container">
      <div className="player-parent">
        {player === null ? (
          <>
            <RemovePlayerButton onClick={onClose} />
            <div className="inline-spinner" />
          </>
        ) : (
          <>
            <RemovePlayerButton onClick={onClose} />
            <ContentInput player={player} />
            <VideoPlayer player={player} />
            <div className="chart">
              <input
                type="checkbox"
                className="buffer-content"
                name={`buffer-content${checkBoxesId}`}
                id={`buffer-content${checkBoxesId}`}
                checked={shouldShowBufferContent}
                onChange={onBufferContentCheckBoxChange}
              />
              <label htmlFor={`buffer-content${checkBoxesId}`}>
                Enable Buffer Content Chart (below when available)
              </label>
              {shouldShowBufferContent && player !== null ? (
                <BufferContentGraph
                  videoElement={player.videoElement}
                  player={player}
                />
              ) : null}
            </div>
            <div className="chart">
              <input
                type="checkbox"
                className="buffer-size"
                name={`buffer-size${checkBoxesId}`}
                id={`buffer-size${checkBoxesId}`}
                checked={shouldShowBufferGaps}
                onChange={onBufferSizeCheckBoxChange}
              />
              <label htmlFor={`buffer-size${checkBoxesId}`}>
                Enable Buffer Size Chart (below when available)
              </label>
              {shouldShowBufferGaps && bufferGaps.length > 0 ? (
                <BufferSizeChart data={bufferGaps} />
              ) : null}
            </div>
            <br />
          </>
        )}
      </div>
    </div>
  );
});
