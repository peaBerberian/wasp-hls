import * as React from "react";

export default function CreateNewPlayerButton(
  {
    onClick,
  } : {
    onClick : () => void;
  }
) : JSX.Element {
  return (
    <button
      className="create-new-player"
      onClick={onClick}
    >
      {"Create new Player"}
    </button>
  );
}
