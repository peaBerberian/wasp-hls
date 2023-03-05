import * as React from "react";
import VolumeBar from "./VolumeBar";

export default React.memo(function VolumeButton({
  onClick,
  volume,
  onVolumeChange,
} : {
  onClick: () => void;
  volume: number;
  onVolumeChange: (volume: number) => void;
}) : JSX.Element {
  let volumeSvg;

  /* eslint-disable max-len */
  if (volume === 0) {
    // No Volume
    volumeSvg = <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <path d="M5 7l4.146-4.146a.5.5 0 01.854.353v13.586a.5.5 0 01-.854.353L5 13H4a2 2 0 01-2-2V9a2 2 0 012-2h1zM12 8.414L13.414 7l1.623 1.623L16.66 7l1.414 1.414-1.623 1.623 1.623 1.623-1.414 1.414-1.623-1.623-1.623 1.623L12 11.66l1.623-1.623L12 8.414z"></path>
    </svg>;
  } else if (volume < 0.7) {
    // Mid Volume
    volumeSvg = <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g>
        <path d="M5 7l4.146-4.146a.5.5 0 01.854.353v13.586a.5.5 0 01-.854.353L5 13H4a2 2 0 01-2-2V9a2 2 0 012-2h1zM14 10a2 2 0 00-2-2v4a2 2 0 002-2z"></path>
      </g>
    </svg>;
  } else {
    // Full volume
    volumeSvg = <svg width="100%" height="100%" viewBox="0 0 20 20" x="0px" y="0px">
      <g>
        <path d="M9.146 2.853L5 7H4a2 2 0 00-2 2v2a2 2 0 002 2h1l4.146 4.146a.5.5 0 00.854-.353V3.207a.5.5 0 00-.854-.353zM12 8a2 2 0 110 4V8z"></path>
        <path d="M12 6a4 4 0 010 8v2a6 6 0 000-12v2z"></path>
      </g>
    </svg>;
  }
  /* eslint-enable max-len */

  return <div className="volume-container">
    <button className="video-controls-button volume-button" onClick={onClick}>
      {volumeSvg}
    </button>
    <VolumeBar volume={volume} onVolumeChange={onVolumeChange} />
  </div>;
});
