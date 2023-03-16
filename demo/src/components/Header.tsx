import * as React from "react";

export default React.memo(function Header() : JSX.Element {
  return <div className="nav-header">
    <div />
    <a style={{ textDecoration: "none", color: "inherit" }} href="./">
      <img className="title" src="./logo-white.png" alt="wasp-hls-player" />
    </a>
    <div />
  </div>;
});
