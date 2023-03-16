import * as React from "react";

export default React.memo(function Title() : JSX.Element {
  return <a style={{ textDecoration: "none", color: "inherit" }} href="./">
    <h1 className="title">{"wasp-hls player"}</h1>
  </a>;
});
