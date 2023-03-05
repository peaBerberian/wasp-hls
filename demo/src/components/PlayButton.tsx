import * as React from "react";

export default function PlayButton({
  disabled,
  isPaused,
  onClick,
}: {
  disabled: boolean;
  isPaused: boolean;
  onClick: () => void;
}) : JSX.Element {
    // Shamefully copied from some other website
  const svg =  isPaused ?
    <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g>
        <path
          d="M5 17.066V2.934a.5.5 0 01.777-.416L17 10 5.777 17.482A.5.5 0 015 17.066z"
        ></path>
      </g>
    </svg> :
    <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g><path d="M8 3H4v14h4V3zM16 3h-4v14h4V3z"></path></g>
    </svg>;
  return <button
    disabled={disabled}
    className="video-controls-button play-button" onClick={onClick}
  >
    {svg}
  </button>;
}
