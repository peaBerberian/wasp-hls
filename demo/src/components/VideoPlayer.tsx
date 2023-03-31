import * as React from "react";
import WaspHlsPlayer, { PlayerState, WaspError } from "../../../src";
import {
  exitFullscreen,
  isFullscreen,
  requestFullscreen,
} from "../utils/fullscreen";
import ControlBar from "./ControlBar";
import Spinner from "./Spinner";

export default React.memo(function VideoPlayer({
  player,
}: {
  player: WaspHlsPlayer;
}): JSX.Element {
  const playerContainerRef = React.useRef<HTMLDivElement | null>(null);
  const [isInFullScreenMode, setIsInFullscreenMode] = React.useState(
    isFullscreen()
  );
  const [isVideoClickable, setIsVideoClickable] = React.useState(false);
  const [shouldShowSpinner, setShouldShowSpinner] = React.useState(
    player.getPlayerState() === PlayerState.Loading || player.isRebuffering()
  );
  const [error, setError] = React.useState<WaspError | null>(null);
  const [wrapperStyle, setWrapperStyle] = React.useState({});

  // Inserting already-existing DOM into React looks a little weird
  const videoWrapperRef: React.Ref<HTMLDivElement> = React.useRef(null);

  React.useEffect(() => {
    if (playerContainerRef.current === null) {
      return;
    }
    const observer = new ResizeObserver(() => {
      if (playerContainerRef.current === null) {
        return;
      }
      const { clientWidth } = playerContainerRef.current;
      const ratio = 16 / 9;
      setWrapperStyle({
        height: `${clientWidth / ratio}px`,
      });
    });
    observer.observe(playerContainerRef.current);
    return () => {
      observer.disconnect();
    };
  }, [player]);
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
    player.addEventListener("playerStateChange", onPlayerStateChange);
    player.addEventListener("rebufferingStarted", onRebuffering);
    player.addEventListener("rebufferingEnded", disableSpinner);
    return () => {
      player.removeEventListener("playerStateChange", onPlayerStateChange);
      player.removeEventListener("rebufferingStarted", onRebuffering);
      player.removeEventListener("rebufferingEnded", disableSpinner);
    };

    function onPlayerStateChange(playerState: PlayerState): void {
      switch (playerState) {
        case PlayerState.Stopped:
          disableSpinner();
          disableClickableVideo();
          setError(null);
          break;
        case PlayerState.Loading:
          enableSpinnerAfterTimeout(30);
          setError(null);
          break;
        case PlayerState.Loaded:
          disableSpinner();
          enableClickableVideo();
          setError(null);
          player.resume();
          break;
        case PlayerState.Error:
          disableSpinner();
          disableClickableVideo();
          setError(player.getError());
          break;
      }
    }

    function onRebuffering() {
      enableSpinnerAfterTimeout(500);
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

  return (
    <div
      className="video-container"
      ref={playerContainerRef}
      style={wrapperStyle}
    >
      <div
        className={
          "video-element-wrapper " + (isVideoClickable ? "clickable" : "")
        }
        onClick={onVideoWrapperClick}
        ref={videoWrapperRef}
      />
      {error !== null ? (
        <div className="video-element-error">
          <div className="video-element-error-name">{error.name}</div>
          <div className="video-element-error-code">
            {"Code: " + error.code}
          </div>
          <div className="video-element-error-message">{error.message}</div>
        </div>
      ) : null}
      {shouldShowSpinner ? <Spinner /> : null}
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
    </div>
  );
});
