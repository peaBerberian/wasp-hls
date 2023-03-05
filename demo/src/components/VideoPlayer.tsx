import * as React from "react";
import WaspHlsPlayer from "../../../src";
import {
  exitFullscreen,
  isFullscreen,
  requestFullscreen,
} from "../utils/fullscreen";
import FullScreenButton from "./FullScreenButton";
import PlayButton from "./PlayButton";
import PositionIndicator from "./PositionIndicator";
import ProgressBar from "./ProgressBar";
import StopButton from "./StopButton";
import VolumeButton from "./VolumeButton";

const TIME_CHECK_INTERVAL = 200;

export default React.memo(function VideoPlayer(
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
  const [bufferGap, setBufferGap] = React.useState(0);
  const [isPaused, setIsPaused] = React.useState(true);
  const [showControlBar, setShowControlBar] = React.useState(true);
  const [disableControls, setDisableControls] = React.useState(true);

  // Inserting already-existing DOM into React looks a little weird
  const videoWrapperRef: React.Ref<HTMLDivElement> = React.useRef(null);

  const playerContainerRef: React.Ref<HTMLDivElement> = React.useRef(null);

  React.useEffect(() => {
    let positionRefreshIntervalId: number|undefined;
    let hideControlBarTimeoutId: number|undefined;
    let isElementHovered = false;

    player.addEventListener("loaded", onLoaded);
    player.addEventListener("stopped", onStopped);
    player.videoElement.addEventListener("pause", onPause);
    player.videoElement.addEventListener("play", onPlay);
    player.videoElement.addEventListener("volumechange", onVolumeChange);
    if (playerContainerRef.current !== null) {
      playerContainerRef.current.addEventListener("mouseover", onMouseOver);
      playerContainerRef.current.addEventListener("mousemove", onMouseMove);
      playerContainerRef.current.addEventListener("mouseout", onMouseOut);
    }

    if (videoWrapperRef.current !== null) {
      videoWrapperRef.current.appendChild(player.videoElement);
    }

    const canControlBarBeHidden = (mouseY: number): boolean => {
      if (playerContainerRef.current !== null) {
        const rect = playerContainerRef.current.getBoundingClientRect();
        if (rect.bottom - mouseY < 50) {
          return false;
        }
      }
      return true;
    };

    return () => {
      resetTimeInfo();
      player.removeEventListener("loaded", onLoaded);
      player.removeEventListener("stopped", onStopped);
      player.videoElement.removeEventListener("pause", onPause);
      player.videoElement.removeEventListener("play", onPlay);
      player.videoElement.removeEventListener("volumechange", onVolumeChange);
      clearPositionUpdateInterval();
      clearHideControlBarTimeout();
      if (playerContainerRef.current !== null) {
        playerContainerRef.current.removeEventListener("mouseover", onMouseOver);
        playerContainerRef.current.removeEventListener("mousemove", onMouseMove);
        playerContainerRef.current.removeEventListener("mouseout", onMouseOut);
      }
    };

    function onLoaded() {
      lockControlBarOn();
      if (!isElementHovered && player.isPlaying()) {
        hideControlBarAfterTimeout();
      }
      resetTimeInfo();
      setDisableControls(false);
      clearPositionUpdateInterval();
      positionRefreshIntervalId = setInterval(() => {
        const pos = player.getPosition();
        setPosition(pos);
        const minPos = player.getMinimumPosition();
        setMinimumPosition(minPos ?? 0);
        const maxPos = player.getMaximumPosition();
        setMaximumPosition(maxPos ?? Infinity);

        let newBufferGap = 0;
        const buffered = player.videoElement.buffered;
        if (buffered.length > 0) {
          for (let i = 0; i < buffered.length; i++) {
            if (pos >= buffered.start(i) && pos < buffered.end(i)) {
              newBufferGap = buffered.end(i) - pos;
            }
          }
        }
        setBufferGap(newBufferGap);
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
      lockControlBarOn();
      resetTimeInfo();
      setDisableControls(true);
      setIsPaused(true);
      clearPositionUpdateInterval();
    }

    function onPause() {
      lockControlBarOn();
      setIsPaused(true);
    }

    function onPlay() {
      if (!isElementHovered) {
        hideControlBarAfterTimeout();
      }
      setIsPaused(false);
    }

    function onVolumeChange() {
      setVolume(player.videoElement.volume);
    }

    function onMouseOver(evt: { clientY: number }): void {
      isElementHovered = true;
      lockControlBarOn();
      if (canControlBarBeHidden(evt.clientY)) {
        hideControlBarAfterTimeout();
      }
    }

    function onMouseMove(evt: { clientY: number }): void {
      if (!isElementHovered) {
        return;
      }
      lockControlBarOn();
      if (canControlBarBeHidden(evt.clientY)) {
        hideControlBarAfterTimeout();
      }
    }

    function onMouseOut() {
      isElementHovered = false;
      if (player.isPlaying()) {
        hideControlBarAfterTimeout();
      }
    }

    function lockControlBarOn() {
      setShowControlBar(true);
      clearHideControlBarTimeout();
    }

    function hideControlBarAfterTimeout() {
      clearHideControlBarTimeout();
      hideControlBarTimeoutId = setTimeout(() => {
        setShowControlBar(false);
      }, 1500);
    }

    function clearHideControlBarTimeout() {
      if (hideControlBarTimeoutId !== undefined) {
        clearTimeout(hideControlBarTimeoutId);
        hideControlBarTimeoutId = undefined;
      }
    }

    function clearPositionUpdateInterval() {
      if (positionRefreshIntervalId !== undefined) {
        clearInterval(positionRefreshIntervalId);
        positionRefreshIntervalId = undefined;
      }
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

  const onVolumeButtonClick = React.useCallback(() => {
    if (volume === 0) {
      player.videoElement.volume = 1;
    } else {
      player.videoElement.volume = 0;
    }
  }, [player, volume]);

  const onVolumeChange = React.useCallback((newVolume: number) => {
    player.videoElement.volume = newVolume;
  }, [player]);

  const onVideoWrapperClick = React.useCallback(() => {
    if (disableControls) {
      return;
    }
    togglePlayPause();
  }, [disableControls, togglePlayPause]);

  const onStopButtonClick = React.useCallback(() => {
    player.stop();
  }, [player]);

  const onProgressBarSeek = React.useCallback((pos: number) => {
    player.seek(pos);
  }, [player]);

  return <div className="video-container" ref={playerContainerRef}>
    <div
      className="video-element-wrapper"
      onClick={onVideoWrapperClick}
      ref={videoWrapperRef}
      style={ disableControls ? {} : { cursor: "pointer" } }
    />
    <div
      className={"control-bar " + (showControlBar ? "visible" : "hidden")}
    >
      {
        disableControls ?
          null :
        <ProgressBar
          seek={onProgressBarSeek}
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
          <StopButton
            disabled={disableControls}
            onClick={onStopButtonClick}
          />
          {
            disableControls ?
              null :
              <PositionIndicator position={position} duration={maximumPosition} />
          }
        </div>
        <div className="video-controls-right">
          <VolumeButton
            volume={volume}
            onClick={onVolumeButtonClick}
            onVolumeChange={onVolumeChange}
          />
          <FullScreenButton
            disabled={disableControls}
            // TODO it works by luck for now, we should probably listen to an
            // enter/exit fullscreen event and add it to the state
            isFullScreen={isFullscreen()}
            onClick={() => {
              if (isFullscreen()) {
                exitFullscreen();
              } else if (playerContainerRef.current !== null) {
                requestFullscreen(playerContainerRef.current);
              }
            }}
          />
        </div>
      </div>
    </div>
  </div>;
});
