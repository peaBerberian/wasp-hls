import React, {
  useCallback,
  useState,
  useEffect,
} from "react";
import CreateNewPlayerButton from "./CreateNewPlayerButton.js";
import PlayerContainer from "./PlayerContainer.jsx";

/** Unique identifier for each new PlayerContainer */
let currentPlayerId = 1;

export default function PlayerContainersWrapper() : JSX.Element {
  const [players, setPlayers] = useState([]);

  const createPlayerContainer = useCallback((id : number) => {
    const newContainer = <PlayerContainer key={id} onClose={onClose} />;

    setPlayers((prevPlayers : JSX.Element[]) => {
      const newPlayers = prevPlayers.slice();
      newPlayers.push(newContainer);
      return newPlayers;
    });

    return onClose;

    function onClose() : void {
      setPlayers((prevPlayers : JSX.Element[]) => {
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

  const createNewPlayer = useCallback(() => {
    createPlayerContainer(currentPlayerId);
    currentPlayerId++;
  }, []);

  // Create initial player
  useEffect(() => createPlayerContainer(0), []);

  return <>
      {players}
      <CreateNewPlayerButton onClick={createNewPlayer} />
    </>;
}
