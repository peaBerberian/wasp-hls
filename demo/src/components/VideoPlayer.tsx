import React, {
  useEffect,
  useRef,
} from "react";

const TIME_BOUNDS_CHECK_INTERVAL = 1000;

export default function VideoPlayer(
  {
    player,
  } : {
    player : any;
  }
) : JSX.Element {
  // Inserting already-existing DOM into React looks a little far-fetched
  const containerRef = useRef(null);
  useEffect(() => {
    const intervalId = setInterval(() => {
      const pos = player.getPosition();
      const minPos = player.getMinimumPosition();
      if (minPos !== undefined && minPos > pos + 2) {
        console.warn("Behind minimum position, seeking...")
        player.seek(minPos + 2);
      }
    }, TIME_BOUNDS_CHECK_INTERVAL);
    player.addEventListener("warning", onWarning);
    containerRef.current.appendChild(player.videoElement);

    return () => {
      clearInterval(intervalId);
      player.removeEventListener("warning", onWarning);
      if (containerRef.current !== null) {
        containerRef.current.removeChild(player.videoElement);
      }
    };

    function onWarning(payload : any) {
      console.warn("RECEIVED WARNING!!!!", payload);
    }
  }, []);
  return <div ref={containerRef} className="video-container"></div>;
}
