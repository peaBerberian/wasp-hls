import * as React from "react";
import WaspHlsPlayer from "../../../src";

/**
 * Return input elements to load a new content.
 * @param {Object} props
 * @param {Object} props.player - The WaspHlsPlayer instance on which the
 * content will be loaded
 * @returns {Object}
 */
export default React.memo(function ContentInput(
  {
    player,
  } : {
    player : WaspHlsPlayer;
  }
) : JSX.Element {
  // TODO better input management
  const inputElRef = React.useRef<HTMLInputElement>(null);
  const loadContent = React.useCallback(() => {
    if (inputElRef.current === null) {
      return;
    }
    player.loadContent(inputElRef.current.value);
  }, [player]);
  const onKeyDown = React.useCallback((e: { key: string }) => {
    if (e.key === "Enter") {
      loadContent();
    }
  }, [loadContent]);
  return (
    <div
      className = "inputs-container"
    >
      <label htmlFor= "url" >{"URL to HLS MultiVariant (a.k.a. Master) Playlist:"}</label>
      <br />
      <input
        ref={inputElRef}
        onKeyDown={onKeyDown}
        className="input-url"
        type="text"
        name="url"
        /* eslint-disable-next-line max-len */
        defaultValue="https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8"
      />

      <button
        id="loading-button"
        onClick={loadContent}
      >
        Load
      </button>
    </div>
  );
});