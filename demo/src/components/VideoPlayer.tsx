import * as React from "react";
import WaspHlsPlayer from "../../../src";
import PlayButton from "./PlayButton";
import ProgressBar from "./ProgressBar";
import StopButton from "./StopButton";
import VolumeButton from "./VolumeButton";

const TIME_CHECK_INTERVAL = 200;

export default function VideoPlayer(
  {
    player,
  } : {
    player : WaspHlsPlayer;
  }
) : JSX.Element {
  const [volume, setVolume] = React.useState(player.videoElement.volume);
  const [position, setPosition] = React.useState(0);
  const [minimumPosition, setMinimumPosition] = React.useState(0);
  const [maximumPosition, setMaximumPosition] = React.useState(Infinity);
  const [bufferGap, setBufferGap] = React.useState(Infinity);
  const [isPaused, setIsPaused] = React.useState(true);
  const [disableControls, setDisableControls] = React.useState(true);

  // Inserting already-existing DOM into React looks a little far-fetched
  const containerRef : React.Ref<HTMLDivElement> = React.useRef(null);
  React.useEffect(() => {
    let positionRefreshIntervalId: number|undefined;
    player.addEventListener("loaded", onLoaded);
    player.addEventListener("stopped", onStopped);
    player.videoElement.addEventListener("pause", onPause);
    player.videoElement.addEventListener("play", onPlay);
    player.videoElement.addEventListener("volumechange", onVolumeChange);
    if (containerRef.current !== null) {
      containerRef.current.appendChild(player.videoElement);
    }

    return () => {
      resetTimeInfo();
      player.removeEventListener("loaded", onLoaded);
      player.removeEventListener("stopped", onStopped);
      player.videoElement.removeEventListener("pause", onPause);
      player.videoElement.removeEventListener("play", onPlay);
      player.videoElement.removeEventListener("volumechange", onVolumeChange);
      clearInterval(positionRefreshIntervalId);
      if (containerRef.current !== null) {
        containerRef.current.removeChild(player.videoElement);
      }
    };

    function onLoaded() {
      resetTimeInfo();
      setDisableControls(false);
      if (positionRefreshIntervalId !== undefined) {
        clearInterval(positionRefreshIntervalId);
      }
      positionRefreshIntervalId = setInterval(() => {
        const pos = player.getPosition();
        setPosition(pos);
        const minPos = player.getMinimumPosition();
        setMinimumPosition(minPos ?? 0);
        const maxPos = player.getMaximumPosition();
        setMaximumPosition(maxPos ?? Infinity);
        if (!player.isPaused()) {
          if (minPos !== undefined && minPos > pos + 2) {
            console.warn("Behind minimum position, seeking...");
            player.seek(minPos + 2);
          }
        }
      }, TIME_CHECK_INTERVAL);
    }

    function resetTimeInfo() {
      setPosition(0);
      setMinimumPosition(0);
      setMaximumPosition(Infinity);
      setBufferGap(0);
    }

    function onStopped() {
      resetTimeInfo();
      setDisableControls(true);
      setIsPaused(true);
      if (positionRefreshIntervalId !== undefined) {
        clearInterval(positionRefreshIntervalId);
        positionRefreshIntervalId = undefined;
      }
    }

    function onPause() {
      setIsPaused(true);
    }

    function onPlay() {
      setIsPaused(false);
    }

    function onVolumeChange() {
      setVolume(player.videoElement.volume);
    }
  }, [player]);

  const togglePlayPause = React.useCallback(() => {
    if (isPaused) {
      player.resume();
      setIsPaused(false);
    } else {
      player.pause();
      setIsPaused(true);
    }
  }, [player, isPaused]);
  return <div className="video-container">
    <div className="video-element-wrapper" onClick={() => {
      if (disableControls) {
        return;
      }
      togglePlayPause();
    }} ref={containerRef} style={ disableControls ? {} : { cursor: "pointer" } } />
    {
      disableControls ?
        null :
      <ProgressBar
        seek={(pos: number) => { player.seek(pos); }}
        position={position}
        bufferGap={bufferGap}
        minimumPosition={minimumPosition}
        maximumPosition={maximumPosition}
      />
    }
    <div className="video-controls">
      <div className="video-controls-left">
        <PlayButton
          disabled={disableControls}
          isPaused={isPaused}
          onClick={togglePlayPause}
        />
        <StopButton disabled={disableControls} onClick={() => {
          player.stop();
        }} />
      </div>
      <div className="video-controls-right">
        <VolumeButton volume={volume} onClick={() => {
          if (volume === 0) {
            player.videoElement.volume = 1;
          } else {
            player.videoElement.volume = 0;
          }
        }} onVolumeChange={(newVolume: number) => {
          player.videoElement.volume = newVolume;
        }} />
      </div>
    </div>
  </div>;
}
