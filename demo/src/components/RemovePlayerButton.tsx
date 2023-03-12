import * as React from "react";

export default React.memo(function RemovePlayerButton(
  {
    onClick,
  } : {
    onClick : () => void;
  }
) : JSX.Element {
  return (
    <button
      className="remove-player white-button"
      onClick={onClick}
    >
      {"Close X"}
    </button>
  );
});
