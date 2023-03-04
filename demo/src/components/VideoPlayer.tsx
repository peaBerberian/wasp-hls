import React, {
  useEffect,
  useRef,
} from "react";

export default function VideoPlayer(
  {
    videoElt,
  } : {
    videoElt : HTMLVideoElement;
  }
) : JSX.Element {
  // Inserting already-existing DOM into React looks a little far-fetched
  const containerRef = useRef(null);
  useEffect(() => {
    containerRef.current.appendChild(videoElt);
  }, []);
  return <div ref={containerRef} className="video-container"></div>;
}
