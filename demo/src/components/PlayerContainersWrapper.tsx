import * as React from "react";
import CreateNewPlayerButton from "./CreateNewPlayerButton.js";
import PlayerContainer from "./PlayerContainer.jsx";

/** Unique identifier for each new PlayerContainer */
let currentPlayerId = 1;

export default function PlayerContainersWrapper(): JSX.Element {
  const [players, setPlayers] = React.useState<JSX.Element[]>([]);

  const createPlayerContainer = React.useCallback((id: number) => {
    const newContainer = <PlayerContainer key={id} onClose={onClose} />;

    setPlayers((prevPlayers) => {
      const newPlayers = prevPlayers.slice();
      newPlayers.push(newContainer);
      return newPlayers;
    });

    return onClose;

    function onClose(): void {
      setPlayers((prevPlayers: JSX.Element[]) => {
        const indexOf = prevPlayers.indexOf(newContainer);
        if (indexOf >= 0) {
          const newPlayers = prevPlayers.slice();
          newPlayers.splice(indexOf, 1);
          return newPlayers;
        }
        return prevPlayers;
      });
    }
  }, []);

  const createNewPlayer = React.useCallback(() => {
    createPlayerContainer(currentPlayerId);
    currentPlayerId++;
  }, []);

  // Create initial player
  React.useEffect(() => createPlayerContainer(0), []);

  return (
    <>
      {players}
      <CreateNewPlayerButton onClick={createNewPlayer} />
    </>
  );
}
