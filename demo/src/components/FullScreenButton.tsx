import * as React from "react";

export default React.memo(function FullScreenButton({
  disabled,
  isFullScreen,
  onClick,
}: {
  disabled: boolean;
  isFullScreen: boolean;
  onClick: () => void;
}): JSX.Element {
  // Shamefully copied from some other website
  /* eslint-disable max-len */
  const svg = isFullScreen ? (
    <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g>
        <path d="M8 8V3H6v3H2v2h6zM12 8h6V6h-4V3h-2v5zM12 17v-5h6v2h-4v3h-2zM8 12H2v2h4v3h2v-5z"></path>
      </g>
    </svg>
  ) : (
    <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g>
        <path d="M7 3H2v5h2V5h3V3zM18 8V3h-5v2h3v3h2zM13 17v-2h3v-3h2v5h-5zM4 12H2v5h5v-2H4v-3z"></path>
      </g>
    </svg>
  );
  /* eslint-enable max-len */
  return (
    <button
      disabled={disabled}
      className="video-controls-button fullscreen-button"
      onClick={onClick}
    >
      {svg}
    </button>
  );
});
