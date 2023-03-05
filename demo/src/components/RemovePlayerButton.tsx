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
      className="remove-player"
      onClick={onClick}
    >
      {"Remove Player"}
    </button>
  );
});
