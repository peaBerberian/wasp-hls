import * as React from "react";
import WaspHlsPlayer, {
  PlayerState,
} from "../../../src";
import {
  exitFullscreen,
  isFullscreen,
  requestFullscreen,
} from "../utils/fullscreen";
import ControlBar from "./ControlBar";
import Spinner from "./Spinner";

export default React.memo(function VideoPlayer(
  {
    player,
  } : {
    player : WaspHlsPlayer;
  }
) : JSX.Element {
  const playerContainerRef = React.useRef<HTMLDivElement|null>(null);
  const [isInFullScreenMode, setIsInFullscreenMode] = React.useState(isFullscreen());
  const [isVideoClickable, setIsVideoClickable] = React.useState(false);
  const [shouldShowSpinner, setShouldShowSpinner] = React.useState(
    player.getPlayerState() === PlayerState.Loading || player.isRebuffering()
  );

  // Inserting already-existing DOM into React looks a little weird
  const videoWrapperRef: React.Ref<HTMLDivElement> = React.useRef(null);

  React.useEffect(() => {
    const onFullScreenChange = () => {
      setIsInFullscreenMode(isFullscreen());
    };
    document.addEventListener("fullscreenchange", onFullScreenChange);
    return () => {
      document.removeEventListener("fullscreenchange", onFullScreenChange);
    };
  }, []);

  React.useEffect(() => {
    let spinnerTimeout: number | null = null;
    const enableClickableVideo = () => setIsVideoClickable(true);
    const disableClickableVideo = () => setIsVideoClickable(false);
    player.addEventListener("loaded", enableClickableVideo);
    player.addEventListener("error", disableClickableVideo);
    player.addEventListener("stopped", disableClickableVideo);
    player.addEventListener("loading", enableSpinnerAfterTimeout);
    player.addEventListener("loaded", disableSpinner);
    player.addEventListener("error", disableSpinner);
    player.addEventListener("stopped", disableSpinner);
    player.addEventListener("rebufferingStarted", enableSpinnerAfterTimeout);
    player.addEventListener("rebufferingEnded", disableSpinner);
    return () => {
      player.removeEventListener("loaded", enableClickableVideo);
      player.removeEventListener("error", disableClickableVideo);
      player.removeEventListener("stopped", disableClickableVideo);
      player.removeEventListener("loading", enableSpinnerAfterTimeout);
      player.removeEventListener("rebufferingStarted", enableSpinnerAfterTimeout);
      player.removeEventListener("rebufferingEnded", disableSpinner);
    };

    function enableSpinnerAfterTimeout() {
      if (spinnerTimeout !== null) {
        clearTimeout(spinnerTimeout);
      }
      spinnerTimeout = setTimeout(() => {
        setShouldShowSpinner(true);
      }, 500);
    }

    function disableSpinner() {
      if (spinnerTimeout !== null) {
        clearTimeout(spinnerTimeout);
        spinnerTimeout = null;
      }
      setShouldShowSpinner(false);
    }
  }, [player]);

  React.useEffect(() => {
    if (videoWrapperRef.current !== null) {
      videoWrapperRef.current.appendChild(player.videoElement);
    }

    return () => {
      if (videoWrapperRef.current?.contains(player.videoElement) === true) {
        videoWrapperRef.current.removeChild(player.videoElement);
      }
    };
  }, []);

  const togglePlayPause = React.useCallback(() => {
    if (player.isPaused()) {
      player.resume();
    } else {
      player.pause();
    }
  }, [player]);

  const onVideoWrapperClick = React.useCallback(() => {
    if (isVideoClickable) {
      togglePlayPause();
    }
  }, [togglePlayPause, isVideoClickable]);

  return <div
    className="video-container"
    ref={playerContainerRef}
  >
    <div
      className={"video-element-wrapper " + (isVideoClickable ? "clickable" : "")}
      onClick={onVideoWrapperClick}
      ref={videoWrapperRef}
    />
    {
      shouldShowSpinner ?
        <Spinner /> :
        null
    }
    <ControlBar
      player={player}
      playerContainerRef={playerContainerRef}
      isInFullScreenMode={isInFullScreenMode}
      toggleFullScreen={() => {
        if (isFullscreen()) {
          exitFullscreen();
        } else if (playerContainerRef.current !== null) {
          requestFullscreen(playerContainerRef.current);
        }
      }}
    />
  </div>;
});
