import * as React from "react";
import * as ReactDOM from "react-dom/client";
import {
  logger,
  LoggerLevels,
} from "../../src";
import PlayerContainersWrapper from "./components/PlayerContainersWrapper";

logger.setLevel(LoggerLevels.Debug);

/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
(window as any).logger = logger;
(window as any).LoggerLevels = LoggerLevels;
/* eslint-enable @typescript-eslint/no-unsafe-member-access */
/* eslint-enable @typescript-eslint/no-explicit-any */

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
