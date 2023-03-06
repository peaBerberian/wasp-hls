import * as React from "react";
import WaspHlsPlayer, {
  PlayerState,
} from "../../../src";
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
  const [isControlBarEnabled, setIsControlBarEnabled] = React.useState(true);
  const [areControlsDisabled, setAreControlsDisabled] = React.useState(true);
  const [isInFullScreenMode, setIsInFullscreenMode] = React.useState(isFullscreen());
  const lastMouseY = React.useRef(0);
  const isPlayerElementHovered = React.useRef(false);
  const hideControlBarTimeoutId = React.useRef<number|undefined>(undefined);
  const playerContainerRef: React.Ref<HTMLDivElement> = React.useRef(null);

  // Inserting already-existing DOM into React looks a little weird
  const videoWrapperRef: React.Ref<HTMLDivElement> = React.useRef(null);

  const clearHideControlBarTimeout = React.useCallback(() => {
    if (hideControlBarTimeoutId.current !== undefined) {
      clearTimeout(hideControlBarTimeoutId.current);
      hideControlBarTimeoutId.current = undefined;
    }
  }, []);

  // Clear Timeout on unmount
  React.useEffect(() => clearHideControlBarTimeout, [clearHideControlBarTimeout]);

  React.useEffect(() => {
    const onFullScreenChange = () => {
      setIsInFullscreenMode(isFullscreen());
    };
    document.addEventListener("fullscreenchange", onFullScreenChange);
    return () => {
      document.removeEventListener("fullscreenchange", onFullScreenChange);
    };
  }, []);
  const startControlBarHideTimeout = React.useCallback(() => {
    clearHideControlBarTimeout();
    if (!player.isPlaying()) {
      return ;
    }
    if (isPlayerElementHovered.current) {
      if (playerContainerRef.current !== null) {
        const rect = playerContainerRef.current.getBoundingClientRect();
        if (rect.bottom - lastMouseY.current < 50) {
          return;
        }
      }
    }
    hideControlBarTimeoutId.current = setTimeout(() => {
      setIsControlBarEnabled(false);
    }, 1500);
  }, [player, clearHideControlBarTimeout]);

  const displayControlBar =  React.useCallback((keepOpen: boolean) => {
    setIsControlBarEnabled(true);
    clearHideControlBarTimeout();
    if (!keepOpen) {
      startControlBarHideTimeout();
    }
  }, [clearHideControlBarTimeout, startControlBarHideTimeout]);

  React.useEffect(() => {
    let positionRefreshIntervalId: number|undefined;

    player.addEventListener("loaded", onLoaded);
    player.addEventListener("stopped", onStopped);
    player.addEventListener("pause", onPause);
    player.addEventListener("play", onPlay);
    player.videoElement.addEventListener("volumechange", onVideoVolumeChange);
    if (playerContainerRef.current !== null) {
      playerContainerRef.current.addEventListener("mouseover", onMouseOver);
      playerContainerRef.current.addEventListener("mousemove", onMouseMove);
      playerContainerRef.current.addEventListener("mouseout", onMouseOut);
    }

    if (videoWrapperRef.current !== null) {
      videoWrapperRef.current.appendChild(player.videoElement);
    }

    return () => {
      resetTimeInfo();
      player.removeEventListener("loaded", onLoaded);
      player.removeEventListener("stopped", onStopped);
      player.removeEventListener("pause", onPause);
      player.removeEventListener("play", onPlay);
      player.videoElement.removeEventListener("volumechange", onVideoVolumeChange);
      clearPositionUpdateInterval();
      if (playerContainerRef.current !== null) {
        playerContainerRef.current.removeEventListener("mouseover", onMouseOver);
        playerContainerRef.current.removeEventListener("mousemove", onMouseMove);
        playerContainerRef.current.removeEventListener("mouseout", onMouseOut);
      }
    };

    function onLoaded() {
      displayControlBar(false);
      resetTimeInfo();
      setAreControlsDisabled(false);
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
      displayControlBar(true);
      resetTimeInfo();
      setAreControlsDisabled(true);
      setIsPaused(true);
      clearPositionUpdateInterval();
    }

    function onPause() {
      displayControlBar(true);
      setIsPaused(true);
    }

    function onPlay() {
      startControlBarHideTimeout();
      setIsPaused(false);
    }

    function onVideoVolumeChange() {
      setVolume(player.videoElement.volume);
    }

    function onMouseOver(evt: { clientY: number }): void {
      lastMouseY.current = evt.clientY;
      isPlayerElementHovered.current = true;
      displayControlBar(false);
    }

    function onMouseMove(evt: { clientY: number }): void {
      lastMouseY.current = evt.clientY;
      if (!isPlayerElementHovered.current) {
        return;
      }
      displayControlBar(false);
    }

    function onMouseOut(evt: { clientY: number }): void {
      lastMouseY.current = evt.clientY;
      isPlayerElementHovered.current = false;
      startControlBarHideTimeout();
    }

    function clearPositionUpdateInterval() {
      if (positionRefreshIntervalId !== undefined) {
        clearInterval(positionRefreshIntervalId);
        positionRefreshIntervalId = undefined;
      }
    }
  }, [player, displayControlBar, hideControlBarTimeoutId]);

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
    if (areControlsDisabled) {
      return;
    }
    togglePlayPause();
  }, [areControlsDisabled, togglePlayPause]);

  const onStopButtonClick = React.useCallback(() => {
    player.stop();
  }, [player]);

  const onProgressBarSeek = React.useCallback((pos: number) => {
    player.seek(pos);
  }, [player]);

  // Handle controls on keypresses
  React.useEffect(() => {
    if (areControlsDisabled) {
      return;
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
    function onKeyDown(evt: { preventDefault: () => void; key: string }) {
      switch (evt.key) {
        case " ":
          evt.preventDefault();
          displayControlBar(false);
          togglePlayPause();
          break;
        case "ArrowRight":
          if (player.getPlayerState() === PlayerState.Loaded) {
            const maxPosition = player.getMaximumPosition();
            if (maxPosition !== undefined) {
              evt.preventDefault();
              const newPosition = Math.min(player.getPosition() + 10, maxPosition);
              displayControlBar(false);
              player.seek(newPosition);
            }
          }
          break;
        case "ArrowLeft":
          if (player.getPlayerState() === PlayerState.Loaded) {
            const minPosition = player.getMinimumPosition();
            if (minPosition !== undefined) {
              evt.preventDefault();
              const newPosition = Math.max(player.getPosition() - 10, minPosition);
              displayControlBar(false);
              player.seek(newPosition);
            }
          }
          break;
      }
    }
  }, [
    areControlsDisabled,
    togglePlayPause,
    player,
    displayControlBar,
    hideControlBarTimeoutId,
  ]);

  return <div
    className="video-container"
    ref={playerContainerRef}
  >
    <div
      className="video-element-wrapper"
      onClick={onVideoWrapperClick}
      ref={videoWrapperRef}
      style={ areControlsDisabled ? {} : { cursor: "pointer" } }
    />
    <div
      className={"control-bar " + (isControlBarEnabled ? "visible" : "hidden")}
    >
      {
        areControlsDisabled ?
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
            disabled={areControlsDisabled}
            isPaused={isPaused}
            onClick={togglePlayPause}
          />
          <StopButton
            disabled={areControlsDisabled}
            onClick={onStopButtonClick}
          />
          {
            areControlsDisabled ?
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
            disabled={areControlsDisabled}
            // TODO it works by luck for now, we should probably listen to an
            // enter/exit fullscreen event and add it to the state
            isFullScreen={isInFullScreenMode}
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
