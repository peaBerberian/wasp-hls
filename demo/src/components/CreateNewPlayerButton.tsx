import * as React from "react";

export default React.memo(function CreateNewPlayerButton({
  onClick,
}: {
  onClick: () => void;
}): JSX.Element {
  return (
    <button className="create-new-player white-button" onClick={onClick}>
      {"Create new Player (above)"}
    </button>
  );
});
