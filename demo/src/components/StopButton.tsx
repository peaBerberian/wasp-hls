import * as React from "react";

export default function StopButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) : JSX.Element {
  return <button
    disabled={disabled}
    className="video-controls-button stop-button" onClick={onClick}
  >
    <svg viewBox="0 0 20 20" x="0px" y="0px" width="20" height="20">
      <rect width="12" height="12" x="4" y="4"></rect>
    </svg> :
  </button>;
}
