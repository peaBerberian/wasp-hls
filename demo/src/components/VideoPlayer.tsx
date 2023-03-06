import * as React from "react";
import WaspHlsPlayer from "../../../src";
import {
  exitFullscreen,
  isFullscreen,
  requestFullscreen,
} from "../utils/fullscreen";
import ControlBar from "./ControlBar";

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
    const enableClickableVideo = () => setIsVideoClickable(true);
    const disableClickableVideo = () => setIsVideoClickable(false);
    player.addEventListener("loaded", enableClickableVideo);
    player.addEventListener("error", disableClickableVideo);
    player.addEventListener("stopped", disableClickableVideo);
    return () => {
      player.removeEventListener("loaded", enableClickableVideo);
      player.removeEventListener("error", disableClickableVideo);
      player.removeEventListener("stopped", disableClickableVideo);
    };
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
