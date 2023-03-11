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
  const [error, setError] = React.useState<Error|null>(null);

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
    player.addEventListener("loaded", onLoaded);
    player.addEventListener("error", onError);
    player.addEventListener("stopped", onStopped);
    player.addEventListener("loading", onLoading);
    player.addEventListener("stopped", onStopped);
    player.addEventListener("rebufferingStarted", onRebuffering);
    player.addEventListener("rebufferingEnded", disableSpinner);
    return () => {
      player.removeEventListener("loaded", onLoaded);
      player.removeEventListener("error", onError);
      player.removeEventListener("stopped", onStopped);
      player.removeEventListener("loading", onLoading);
      player.removeEventListener("rebufferingStarted", onRebuffering);
      player.removeEventListener("rebufferingEnded", disableSpinner);
    };

    function onLoaded() {
      disableSpinner();
      enableClickableVideo();
      setError(null);
    }

    function onStopped() {
      disableSpinner();
      disableClickableVideo();
      setError(null);
    }

    function onLoading() {
      enableSpinnerAfterTimeout(30);
      setError(null);
    }

    function onRebuffering() {
      enableSpinnerAfterTimeout(500);
    }

    function onError(err: Error) {
      disableSpinner();
      disableClickableVideo();
      setError(err);
    }

    function enableSpinnerAfterTimeout(timeout: number) {
      if (spinnerTimeout !== null) {
        clearTimeout(spinnerTimeout);
      }
      spinnerTimeout = setTimeout(() => {
        setShouldShowSpinner(true);
      }, timeout);
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
      error !== null ?
        <div className="video-element-error">
          <div className="video-element-error-name">
            {error.name}
          </div>
          <div className="video-element-error-message">
            {error.message}
          </div>
        </div> :
        null
    }
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
