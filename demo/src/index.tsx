import * as React from "react";
import * as ReactDOM from "react-dom/client";
import { logger, LoggerLevel } from "wasp-hls";
import Header from "./components/Header";
import PlayerContainersWrapper from "./components/PlayerContainersWrapper";

logger.setLevel(LoggerLevel.Debug);

/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable @typescript-eslint/no-unsafe-member-access */
(window as any).logger = logger;
(window as any).LoggerLevel = LoggerLevel;
/* eslint-enable @typescript-eslint/no-unsafe-member-access */
/* eslint-enable @typescript-eslint/no-explicit-any */

window.onload = function (): void {
  const rootElt = document.getElementById("main");
  if (rootElt === null) {
    console.error("Error: missing `main` element");
    return;
  }
  const root = ReactDOM.createRoot(rootElt);
  root.render(
    <React.StrictMode>
      <Header />
      <PlayerContainersWrapper />
    </React.StrictMode>
  );
};
