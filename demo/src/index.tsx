import * as React from "react";
import * as ReactDOM from "react-dom/client";
import PlayerContainersWrapper from "./components/PlayerContainersWrapper";

window.onload = function() : void {
  const rootElt = document.getElementById("main");
  if (rootElt === null) {
    console.error("Error: missing `main` element");
    return;
  }
  const root = ReactDOM.createRoot(rootElt);
  root.render(
    <React.StrictMode>
      <PlayerContainersWrapper />
    </React.StrictMode>);
};
