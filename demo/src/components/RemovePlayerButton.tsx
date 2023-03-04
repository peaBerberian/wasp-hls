import React from "react";

export default function RemovePlayerButton(
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
}

