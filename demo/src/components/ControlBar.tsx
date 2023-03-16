import * as React from "react";
import WaspHlsPlayer, { PlayerState } from "../../../src";
import FullScreenButton from "./FullScreenButton";
import PlayButton from "./PlayButton";
import PositionIndicator from "./PositionIndicator";
import ProgressBar from "./ProgressBar";
import SettingsButton from "./SettingsButton";
import SettingsWindow from "./SettingsWindow";
import StopButton from "./StopButton";
import VolumeButton from "./VolumeButton";

const TIME_CHECK_INTERVAL = 200;

export default React.memo(function ControlBar({
  player,
  isInFullScreenMode,
  toggleFullScreen,
  playerContainerRef,
}: {
  player: WaspHlsPlayer;
  isInFullScreenMode: boolean;
  toggleFullScreen: () => void;

  // TODO it's weird to have a reference to the control bar's parent
  playerContainerRef: React.MutableRefObject<HTMLDivElement | null>;
}): JSX.Element {
  const [volume, setVolume] = React.useState(player.videoElement.volume);
  const [position, setPosition] = React.useState<number | undefined>(undefined);
  const [minimumPosition, setMinimumPosition] = React.useState(0);
  const [maximumPosition, setMaximumPosition] = React.useState(Infinity);
  const [bufferGap, setBufferGap] = React.useState(0);
  const [isPaused, setIsPaused] = React.useState(true);
  const [isControlBarDisplayed, setIsControlBarDisplayed] =
    React.useState(true);
  const [areControlsDisabled, setAreControlsDisabled] = React.useState(true);
  const [isPlayPauseDisabled, setIsPlayPauseDisabled] = React.useState(true);
  const [shouldDisplaySettings, setShouldDisplaySettings] =
    React.useState(false);
  const lastMouseY = React.useRef(0);
  const isPlayerElementHovered = React.useRef(false);
  const hideControlBarTimeoutId = React.useRef<number | undefined>(undefined);

  const clearHideControlBarTimeout = React.useCallback(() => {
    if (hideControlBarTimeoutId.current !== undefined) {
      clearTimeout(hideControlBarTimeoutId.current);
      hideControlBarTimeoutId.current = undefined;
    }
  }, []);

  // Clear Timeout on unmount
  React.useEffect(
    () => clearHideControlBarTimeout,
    [clearHideControlBarTimeout]
  );
  const startControlBarHideTimeout = React.useCallback(() => {
    clearHideControlBarTimeout();
    if (!player.isPlaying()) {
      return;
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
      setIsControlBarDisplayed(false);
    }, 1500);
  }, [player, clearHideControlBarTimeout]);

  const displayControlBar = React.useCallback(
    (keepOpen: boolean) => {
      setIsControlBarDisplayed(true);
      clearHideControlBarTimeout();
      if (!keepOpen) {
        startControlBarHideTimeout();
      }
    },
    [clearHideControlBarTimeout, startControlBarHideTimeout]
  );

  React.useEffect(() => {
    let positionRefreshIntervalId: number | undefined;

    player.addEventListener("playerStateChange", onPlayerStateChange);
    player.addEventListener("paused", onPaused);
    player.addEventListener("playing", onPlaying);
    player.videoElement.addEventListener("volumechange", onVideoVolumeChange);
    if (playerContainerRef.current !== null) {
      playerContainerRef.current.addEventListener("mouseover", onMouseOver);
      playerContainerRef.current.addEventListener("mousemove", onMouseMove);
      playerContainerRef.current.addEventListener("mouseout", onMouseOut);
    }

    return () => {
      resetTimeInfo();
      player.removeEventListener("playerStateChange", onPlayerStateChange);
      player.removeEventListener("paused", onPaused);
      player.removeEventListener("playing", onPlaying);
      player.videoElement.removeEventListener(
        "volumechange",
        onVideoVolumeChange
      );
      clearPositionUpdateInterval();
      if (playerContainerRef.current !== null) {
        playerContainerRef.current.removeEventListener(
          "mouseover",
          onMouseOver
        );
        playerContainerRef.current.removeEventListener(
          "mousemove",
          onMouseMove
        );
        playerContainerRef.current.removeEventListener("mouseout", onMouseOut);
      }
    };

    function onPlayerStateChange(playerState: PlayerState): void {
      console.error("NEW PLAYER State", playerState);
      switch (playerState) {
        case PlayerState.Loading:
          setIsPaused(true);
          setAreControlsDisabled(false);
          setIsPlayPauseDisabled(true);
          resetTimeInfo();
          clearPositionUpdateInterval();
          break;
        case PlayerState.Loaded:
          displayControlBar(false);
          setAreControlsDisabled(false);
          setIsPlayPauseDisabled(false);
          clearPositionUpdateInterval();
          positionRefreshIntervalId = setInterval(
            onPositionUpdateInterval,
            TIME_CHECK_INTERVAL
          );
          break;
        case PlayerState.Stopped:
          displayControlBar(true);
          resetTimeInfo();
          setAreControlsDisabled(true);
          setShouldDisplaySettings(false);
          setIsPlayPauseDisabled(true);
          setIsPaused(true);
          clearPositionUpdateInterval();
          break;
        case PlayerState.Error:
          resetTimeInfo();
          displayControlBar(true);
          setAreControlsDisabled(false);
          setShouldDisplaySettings(false);
          setIsPlayPauseDisabled(true);
          setIsPaused(true);
          clearPositionUpdateInterval();
          break;
      }
    }

    function resetTimeInfo() {
      setPosition(undefined);
      setMinimumPosition(0);
      setMaximumPosition(Infinity);
      setBufferGap(0);
    }

    function onPositionUpdateInterval() {
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
    }

    function onPaused() {
      displayControlBar(true);
      setIsPaused(true);
    }

    function onPlaying() {
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

  const onVolumeChange = React.useCallback(
    (newVolume: number) => {
      player.videoElement.volume = newVolume;
    },
    [player]
  );

  const onStopButtonClick = React.useCallback(() => {
    player.stop();
  }, [player]);

  const onProgressBarSeek = React.useCallback(
    (pos: number) => {
      player.seek(pos);
    },
    [player]
  );

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
              const newPosition = Math.min(
                player.getPosition() + 10,
                maxPosition
              );
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
              const newPosition = Math.max(
                player.getPosition() - 10,
                minPosition
              );
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

  const toggleSettings = React.useCallback(() => {
    setShouldDisplaySettings((s) => !s);
  }, []);

  return (
    <>
      {!areControlsDisabled && shouldDisplaySettings ? (
        <SettingsWindow player={player} />
      ) : null}
      <div
        className={
          "control-bar " + (isControlBarDisplayed ? "visible" : "hidden")
        }
      >
        {areControlsDisabled || position === undefined ? null : (
          <ProgressBar
            seek={onProgressBarSeek}
            position={position}
            bufferGap={bufferGap}
            minimumPosition={minimumPosition}
            maximumPosition={maximumPosition}
          />
        )}
        <div className="video-controls">
          <div className="video-controls-left">
            <PlayButton
              disabled={areControlsDisabled || isPlayPauseDisabled}
              isPaused={isPaused}
              onClick={togglePlayPause}
            />
            <StopButton
              disabled={areControlsDisabled}
              onClick={onStopButtonClick}
            />
            {areControlsDisabled || position === undefined ? null : (
              <PositionIndicator
                position={position}
                duration={maximumPosition}
              />
            )}
          </div>
          <div className="video-controls-right">
            <VolumeButton
              volume={volume}
              onClick={onVolumeButtonClick}
              onVolumeChange={onVolumeChange}
            />
            <SettingsButton
              disabled={areControlsDisabled}
              onClick={toggleSettings}
            />
            <FullScreenButton
              disabled={areControlsDisabled}
              // TODO it works by luck for now, we should probably listen to an
              // enter/exit fullscreen event and add it to the state
              isFullScreen={isInFullScreenMode}
              onClick={toggleFullScreen}
            />
          </div>
        </div>
      </div>
    </>
  );
});
