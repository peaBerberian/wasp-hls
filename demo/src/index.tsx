import React from "react";
import ReactDOM from "react-dom/client";
import PlayerContainersWrapper from "./components/PlayerContainersWrapper";

window.onload = function() : void {
  const root = ReactDOM.createRoot(document.getElementById("main"));
  root.render(
    <React.StrictMode>
      <PlayerContainersWrapper />
    </React.StrictMode>);
};
